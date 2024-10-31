use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_pixel_buffer::prelude::*;
use core::f32;
use image;
use image::Pixel;
use rand::prelude::*;
use rand::seq::SliceRandom;
use std::collections::HashMap;
use std::path::PathBuf;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

const GRID_WIDTH: usize = 320;
const GRID_HEIGHT: usize = 150;
const PIXEL_SIZE: usize = 4;

// Order is important - lighter at the top
#[derive(Debug, Clone, Copy, PartialEq, EnumIter)]
enum Material {
    Fire,
    Gas,
    Air,
    Oil,
    Water,
    Sand,
    Rock,
}

enum InsertMode {
    Material,
    Source,
}

fn choose_random_material(rng: &mut ThreadRng) -> Material {
    match rng.gen_range(0..6) {
        0 => Material::Gas,
        1 => Material::Air,
        2 => Material::Oil,
        3 => Material::Water,
        4 => Material::Sand,
        _ => Material::Rock,
    }
}

struct Particle {
    material: Material,
    alpha: f32,
    energy: usize,
    density: f32,
    viscosity: usize,
    color: Color,
}

impl Default for Particle {
    fn default() -> Self {
        Particle {
            material: Material::Air,
            alpha: 0.0,
            energy: 0,
            density: 1.0,
            viscosity: 1,
            color: Color::srgba(0.0, 0.0, 0.0, 0.0),
        }
    }
}

impl Particle {
    fn new(material: Material, alpha: f32) -> Self {
        let mut particle = Particle::default();
        particle.alpha = alpha;
        particle.set_material(material);
        particle.color = get_material_color(material, alpha);
        particle
    }

    fn set_material(&mut self, material: Material) {
        self.material = material;
        self.energy = match material {
            Material::Gas => 10,
            Material::Oil => 50,
            _ => 0,
        };
        self.density = match material {
            Material::Fire => 0.1,
            Material::Gas => 0.1,
            Material::Air => 0.3,
            Material::Oil => 0.9,
            Material::Water => 1.0,
            Material::Sand => 1.5,
            Material::Rock => 2.0,
        };
        self.viscosity = match material {
            Material::Fire => 10,
            Material::Gas => 6,
            Material::Air => 5,
            Material::Water => 4,
            Material::Oil => 4,
            Material::Sand => 1,
            Material::Rock => 0,
        };
    }
}

fn choose_alpha(rng: &mut ThreadRng) -> f32 {
    rng.gen_range(0..=100) as f32 / 100.0
}

struct Source {
    material: Material,
    rate: usize,
    last_inserted: usize,
}

#[derive(Resource)]
struct Simulation {
    width: usize,
    height: usize,
    grid: Vec<Particle>,
    order: Vec<usize>,
    sources: HashMap<usize, Source>,
    material: Material,
    insert_mode: InsertMode,
    insert_rate: usize,
    paused: bool,
    show_materials: bool,
}

impl Simulation {
    fn new(width: usize, height: usize) -> Self {
        let mut rng = rand::thread_rng();
        let mut rng_alpha = rand::thread_rng();
        let grid = (0..width * height)
            .map(|_| choose_random_material(&mut rng))
            .map(|m| Particle::new(m, choose_alpha(&mut rng_alpha)))
            .collect();
        let mut order: Vec<usize> = (0..width * height).map(|v| v).collect();
        order.shuffle(&mut rng);

        Self {
            width,
            height,
            grid,
            order,
            sources: HashMap::new(),
            material: Material::Rock,
            insert_mode: InsertMode::Material,
            insert_rate: 5,
            paused: false,
            show_materials: true,
        }
    }

    fn set_all(&mut self) {
        for idx in 0..self.width * self.height {
            self.grid[idx].set_material(self.material);
        }
    }

    fn reset_random(&mut self) {
        let mut rng = rand::thread_rng();
        for idx in 0..self.width * self.height {
            self.grid[idx].set_material(choose_random_material(&mut rng));
        }
    }

    fn clear_sources(&mut self) {
        self.sources.clear();
    }

    fn toggle_paused(&mut self) {
        self.paused = !self.paused;
    }

    fn toggle_show_materials(&mut self) {
        self.show_materials = !self.show_materials;
    }

    fn set_material(&mut self, material: Material, shift: bool) {
        self.material = material;
        match shift {
            false => self.insert_mode = InsertMode::Material,
            true => self.insert_mode = InsertMode::Source,
        }
    }

    fn set_insert_rate(&mut self, rate: usize) {
        self.insert_rate = 10 - rate;
    }

    fn set_picture(&mut self, path: &PathBuf) {
        let img = image::ImageReader::open(path).unwrap().decode().unwrap();
        let buffer: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> = image::imageops::resize(
            &img,
            self.width.try_into().unwrap(),
            self.height.try_into().unwrap(),
            image::imageops::FilterType::Lanczos3,
        );

        for (idx, pixel) in buffer.pixels().into_iter().enumerate() {
            self.grid[idx].set_material(choose_closest_material(pixel));

            // Keep the original image color
            self.grid[idx].color = pixel_to_color(&pixel);
        }
        self.show_materials = false;
    }

    fn insert(&mut self, x: usize, y: usize) {
        if x < self.width && y < self.height {
            let idx = y * self.width + x;
            match self.insert_mode {
                InsertMode::Material => {
                    self.grid[idx].set_material(self.material);
                    if self.sources.contains_key(&idx) {
                        self.sources.remove(&idx);
                    }
                }
                InsertMode::Source => {
                    self.sources.insert(
                        idx,
                        Source {
                            material: self.material,
                            rate: self.insert_rate,
                            last_inserted: 0,
                        },
                    );
                }
            }
        }
    }

    fn update(&mut self) {
        if self.paused {
            return;
        }

        let mut rng = rand::thread_rng();
        let mut moved = HashMap::new();
        for order_idx in 0..self.order.len() {
            self.update_tile(order_idx, &mut rng, &mut moved);
        }
        for (idx, source) in &mut self.sources {
            if source.last_inserted <= 1 {
                source.last_inserted = source.rate;
                self.grid[*idx].set_material(source.material);
            } else {
                source.last_inserted -= 1;
            }
        }
    }

    fn particle_at(&self, x: i32, y: i32) -> Option<&Particle> {
        if x >= 0 && x < (self.width as i32) && y >= 0 && y < (self.height as i32) {
            let new_idx = (y as usize * self.width) + x as usize;
            Some(&self.grid[new_idx])
        } else {
            None
        }
    }

    fn density_at(&self, x: i32, y: i32) -> Option<f32> {
        if let Some(particle) = self.particle_at(x, y) {
            Some(particle.density)
        } else {
            None
        }
    }

    fn material_at(&self, x: i32, y: i32) -> Option<Material> {
        if let Some(particle) = self.particle_at(x, y) {
            Some(particle.material)
        } else {
            None
        }
    }

    fn energy_at(&self, x: i32, y: i32) -> Option<usize> {
        if let Some(particle) = self.particle_at(x, y) {
            Some(particle.energy)
        } else {
            None
        }
    }

    fn neighbour_on_fire(&mut self, x: i32, y: i32) -> bool {
        if let Some(m) = self.material_at(x, y - 1) {
            if m == Material::Fire {
                return true;
            }
        }
        if let Some(m) = self.material_at(x, y + 1) {
            if m == Material::Fire {
                return true;
            }
        }
        if let Some(m) = self.material_at(x - 1, y) {
            if m == Material::Fire {
                return true;
            }
        }
        if let Some(m) = self.material_at(x + 1, y) {
            if m == Material::Fire {
                return true;
            }
        }
        false
    }

    fn set_on_fire(&mut self, x: i32, y: i32) {
        // Keep other particle properties - just change the material and color
        let idx = y as usize * self.width + x as usize;
        self.grid[idx].material = Material::Fire;
    }

    fn try_set_on_fire(&mut self, x: i32, y: i32) {
        if let Some(e) = self.energy_at(x, y) {
            if e > 0 {
                self.set_on_fire(x, y);
            }
        }
    }

    fn update_tile(
        &mut self,
        order_idx: usize,
        rng: &mut ThreadRng,
        moved: &mut HashMap<usize, usize>,
    ) {
        let idx = self.order[order_idx];

        // 0,0 is top left
        let x = (idx % self.width) as i32;
        let y = (idx / self.width) as i32;

        let density = self.density_at(x, y).unwrap();
        let energy = self.energy_at(x, y).unwrap();
        let choice = rng.gen_ratio(1, 2);

        if self.grid[idx].material == Material::Fire {
            self.try_set_on_fire(x, y - 1);
            self.try_set_on_fire(x, y + 1);
            self.try_set_on_fire(x - 1, y);
            self.try_set_on_fire(x + 1, y);

            if energy > 0 {
                self.grid[idx].energy -= 1;
            }
            if energy == 0 {
                self.grid[idx].set_material(Material::Air);
            }
            return;
        } else if energy > 0 && self.neighbour_on_fire(x, y) {
            self.set_on_fire(x, y);
            return;
        }

        let material = self.material_at(x, y).unwrap();
        let this_viscosity = self.grid[idx].viscosity;

        if let Some(density_below) = self.density_at(x, y + 1) {
            if density > density_below {
                if self.try_swap(idx, x, y + 1, moved, 1) {
                    return;
                }
            }
        }

        if let Some(density_below) = self.density_at(x, y - 1) {
            if density_below > density {
                if self.try_swap(idx, x, y - 1, moved, 1) {
                    return;
                }
            }
        }

        let delta_x = if choice { -1 } else { 1 };
        if this_viscosity > 2 {
            for i in 0..this_viscosity {
                if let Some(particle_left) = self.particle_at(x + delta_x, y) {
                    if material != particle_left.material {
                        if particle_left.viscosity > 4 {
                            if self.try_swap(idx, x + delta_x, y, moved, i) {
                                return;
                            }
                        }
                        break;
                    }
                }
            }
        }

        if this_viscosity > 1 {
            if let Some(particle_left) = self.particle_at(x + delta_x, y) {
                if particle_left.viscosity > 1 && material != particle_left.material {
                    if self.try_swap(idx, x + delta_x, y, moved, 1) {
                        return;
                    }
                }
            }
        }

        if let Some(density_left) = self.density_at(x + delta_x, y) {
            if let Some(density_below_left) = self.density_at(x + delta_x, y + 1) {
                if density > density_left && density > density_below_left {
                    if self.try_swap(idx, x + delta_x, y, moved, 1) {
                        return;
                    }
                }
            }
        }

        if let Some(density_above) = self.density_at(x, y - 1) {
            if let Some(density_left) = self.density_at(x + delta_x, y) {
                if density > density_left && density_above > density {
                    if self.try_swap(idx, x + delta_x, y, moved, 1) {
                        return;
                    }
                }
            }
        }
    }

    fn try_swap(
        &mut self,
        from_idx: usize,
        to_x: i32,
        to_y: i32,
        moved: &mut HashMap<usize, usize>,
        distance: usize,
    ) -> bool {
        if self.grid[from_idx].material == Material::Rock {
            return false;
        }
        if let Some(material) = self.material_at(to_x, to_y) {
            if material == Material::Rock {
                return false;
            }
        }

        let to_idx = to_y as usize * self.width + to_x as usize;

        let from_moved = match moved.get(&from_idx) {
            Some(c) => *c,
            None => 0,
        };
        let to_moved = match moved.get(&to_idx) {
            Some(c) => *c,
            None => 0,
        };
        if from_moved < self.grid[from_idx].viscosity && to_moved < self.grid[to_idx].viscosity {
            self.grid.swap(from_idx, to_idx);
            moved.insert(from_idx, from_moved + distance);
            moved.insert(to_idx, to_moved + distance);
            true
        } else {
            false
        }
    }

    fn get_color(&self, pos: UVec2) -> Color {
        let y: usize = pos.y.try_into().unwrap();
        let x: usize = pos.x.try_into().unwrap();
        let idx: usize = y * self.width + x;
        if self.show_materials {
            get_material_color(self.grid[idx].material, self.grid[idx].alpha)
        } else {
            self.grid[idx].color
        }
    }
}

fn get_material_color(material: Material, alpha: f32) -> Color {
    match material {
        Material::Fire => Color::srgba(1.0, 0.0, 0.0, 0.5 + alpha * 0.5),
        Material::Gas => Color::srgba(0.2, 0.8, 0.1, 0.5 + alpha * 0.5),
        Material::Air => Color::srgba(0.0, 0.0, 0.0, alpha * 0.5),
        Material::Oil => Color::srgba(0.5, 0.5, 0.5, 0.7 + alpha * 0.3),
        Material::Water => Color::srgba(0.0, 0.0, 1.0, 0.5 + alpha * 0.5),
        Material::Sand => Color::srgba(1.0, 1.0, 0.1, 0.5 + alpha * 0.5),
        Material::Rock => Color::srgba(1.0, 1.0, 1.0, 0.3 + alpha * 0.5),
    }
}

fn choose_closest_material(pixel: &image::Rgba<u8>) -> Material {
    let mut closest_material = Material::Air;
    let mut min = f32::MAX;
    for material in Material::iter() {
        let diff = color_diff(get_material_color(material, 0.5), pixel);
        if diff < min {
            min = diff;
            closest_material = material;
        }
    }

    // let rgba = pixel.to_rgba();
    // println!("{}, {}, {}, {} -> {:?}", rgba[0], rgba[1], rgba[2], rgba[3], closest_material);
    closest_material
}

fn pixel_to_color(pixel: &image::Rgba<u8>) -> Color {
    let rgba = pixel.to_rgba();
    Color::srgba(
        rgba[0] as f32 / 255.0,
        rgba[1] as f32 / 255.0,
        rgba[2] as f32 / 255.0,
        rgba[3] as f32 / 255.0,
    )
}

fn color_diff(color: Color, pixel: &image::Rgba<u8>) -> f32 {
    let a = color.to_srgba();
    let b = pixel_to_color(pixel).to_srgba();
    let rd = a.red - b.red;
    let gd = a.green - b.green;
    let bd = a.blue - b.blue;
    let ad = a.alpha - b.alpha;
    (rd * rd) + (gd * gd) + (bd * bd) + (ad * ad)
}

fn setup(mut commands: Commands) {
    let width = GRID_WIDTH;
    let height = GRID_HEIGHT;

    let simulation = Simulation::new(width, height);
    commands.insert_resource(simulation);
}

fn main() {
    let x = GRID_WIDTH.try_into().unwrap();
    let y = GRID_HEIGHT.try_into().unwrap();
    let pixel_size: u32 = PIXEL_SIZE.try_into().unwrap();
    let size = PixelBufferSize {
        size: UVec2::new(x, y),
        pixel_size: UVec2::new(pixel_size, pixel_size),
    };

    let x_f = (x * pixel_size) as f32;
    let y_f = (y * pixel_size) as f32;
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Particles".into(),
                        resolution: (x_f, y_f).into(),
                        resizable: false,
                        ..default()
                    }),
                    ..default()
                })
                .build(),
            PixelBufferPlugin,
        ))
        .add_systems(Startup, (setup, pixel_buffer_setup(size)))
        .add_systems(
            Update,
            (update, keyboard_input, mouse_button_input, file_drop),
        )
        .run();
}

fn update(mut pb: QueryPixelBuffer, mut simulation: ResMut<Simulation>) {
    simulation.update();
    pb.frame().per_pixel(|pos, _| simulation.get_color(pos));
}

fn file_drop(mut evr_dnd: EventReader<FileDragAndDrop>, mut simulation: ResMut<Simulation>) {
    for ev in evr_dnd.read() {
        if let FileDragAndDrop::DroppedFile {
            window: _,
            path_buf,
        } = ev
        {
            simulation.set_picture(path_buf);
        }
    }
}

fn keyboard_input(mut simulation: ResMut<Simulation>, keys: Res<ButtonInput<KeyCode>>) {
    if keys.just_pressed(KeyCode::Space) {
        simulation.set_all();
    }

    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    if keys.just_pressed(KeyCode::KeyA) {
        simulation.set_material(Material::Air, shift);
    }
    if keys.just_pressed(KeyCode::KeyF) {
        simulation.set_material(Material::Fire, shift);
    }
    if keys.just_pressed(KeyCode::KeyG) {
        simulation.set_material(Material::Gas, shift);
    }
    if keys.just_pressed(KeyCode::KeyO) {
        simulation.set_material(Material::Oil, shift);
    }
    if keys.just_pressed(KeyCode::KeyR) {
        simulation.set_material(Material::Rock, shift);
    }
    if keys.just_pressed(KeyCode::KeyS) {
        simulation.set_material(Material::Sand, shift);
    }
    if keys.just_pressed(KeyCode::KeyW) {
        simulation.set_material(Material::Water, shift);
    }
    if keys.just_pressed(KeyCode::KeyC) {
        simulation.clear_sources();
    }
    if keys.just_pressed(KeyCode::KeyP) {
        simulation.toggle_paused();
    }
    if keys.just_pressed(KeyCode::KeyM) {
        simulation.toggle_show_materials();
    }
    if keys.just_pressed(KeyCode::Enter) {
        simulation.reset_random();
    }

    if keys.just_pressed(KeyCode::Digit1) {
        simulation.set_insert_rate(1);
    }
    if keys.just_pressed(KeyCode::Digit2) {
        simulation.set_insert_rate(2);
    }
    if keys.just_pressed(KeyCode::Digit3) {
        simulation.set_insert_rate(3);
    }
    if keys.just_pressed(KeyCode::Digit4) {
        simulation.set_insert_rate(4);
    }
    if keys.just_pressed(KeyCode::Digit5) {
        simulation.set_insert_rate(5);
    }
    if keys.just_pressed(KeyCode::Digit6) {
        simulation.set_insert_rate(6);
    }
    if keys.just_pressed(KeyCode::Digit7) {
        simulation.set_insert_rate(7);
    }
    if keys.just_pressed(KeyCode::Digit8) {
        simulation.set_insert_rate(8);
    }
    if keys.just_pressed(KeyCode::Digit9) {
        simulation.set_insert_rate(9);
    }
}

fn mouse_button_input(
    mut simulation: ResMut<Simulation>,
    q_windows: Query<&Window, With<PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
) {
    if buttons.pressed(MouseButton::Left) {
        if let Some(position) = q_windows.single().cursor_position() {
            let x = position.x as usize / PIXEL_SIZE;
            let y = position.y as usize / PIXEL_SIZE;
            simulation.insert(x, y);
        }
    }
}
