use bevy::prelude::*;
use bevy_pixel_buffer::prelude::*;
use bevy::window::PrimaryWindow;
use rand::prelude::*;
use rand::seq::SliceRandom;
use std::collections::HashMap;

const GRID_WIDTH: usize = 320;
const GRID_HEIGHT: usize = 150;
const PIXEL_SIZE: usize = 4;

// Order is important - lighter at the top
#[derive(Debug, Clone, Copy, PartialEq)]
enum Material {
    Gas,
    Air,
    Water,
    Sand,
    Rock,
}

enum InsertMode {
    Material,
    Source,
}


fn choose_random_material(rng: &mut ThreadRng) -> Material {
    match rng.gen_range(0..5) {
            0 => Material::Gas,
            1 => Material::Air,
            2 => Material::Water,
            3 => Material::Sand,
            _ => Material::Rock,
        }
}

struct Particle {
    material: Material,
    alpha: f32
}

fn choose_alpha(rng: &mut ThreadRng) -> f32 {
    rng.gen_range(0..50) as f32 / 100.0
}

fn density(material: Material) -> f32 {
    match material {
        Material::Gas => 0.1,
        Material::Air => 0.3,
        Material::Water => 1.0,
        Material::Sand => 1.5,
        Material::Rock => 2.0
    }
}

fn viscosity(material: Material) -> usize {
    match material {
        Material::Gas => 6,
        Material::Air => 5,
        Material::Water => 4,
        Material::Sand => 1,
        Material::Rock => 0
    }
}

fn heavier(a: Material, b: Material) -> bool {
    density(a) > density(b)
}

struct Source {
    material: Material,
    location: usize
}

#[derive(Resource)]
struct Simulation {
    width: usize,
    height: usize,
    grid: Vec<Particle>,
    order: Vec<usize>,
    sources: HashMap<usize, Source>,
    material: Material,
    insert_mode: InsertMode
}

impl Simulation {
    fn new(width: usize, height: usize) -> Self {
        let mut rng = rand::thread_rng();
        let mut rng_alpha = rand::thread_rng();
        let grid = (0..width * height)
            .map(|_| match rng.gen_range(0..5) {
                0 => Material::Gas,
                1 => Material::Air,
                2 => Material::Water,
                3 => Material::Sand,
                _ => Material::Rock,
            })
            .map(|m| Particle {material: m, alpha: choose_alpha(&mut rng_alpha)})
            .collect();
        let mut order: Vec<usize> = (0..width * height).map(|v| v).collect();
        order.shuffle(&mut rng);

        let sources = HashMap::new();
        let material = Material::Rock;
        let insert_mode = InsertMode::Material;

        Self { width, height, grid, order, sources, material, insert_mode }
    }

    fn reset_random(&mut self) {
        let mut rng = rand::thread_rng();
        let material = choose_random_material(&mut rng);
        for idx in 0..self.width*self.height {
            self.grid[idx].material = material;
        }
    }

    fn clear_sources(&mut self) {
        self.sources.clear();
    }

    fn set_material(&mut self, material: Material, shift: bool) {
        self.material = material;
        match shift {
            false => self.insert_mode = InsertMode::Material,
            true => self.insert_mode = InsertMode::Source
        }
    }

    fn insert(&mut self, x: usize, y: usize) {
        if x < self.width && y < self.height {
            let idx = y * self.width + x;
            match self.insert_mode {
                InsertMode::Material => {
                    self.grid[idx].material = self.material;
                    if self.sources.contains_key(&idx) {
                        self.sources.remove(&idx);
                    }
                }
                InsertMode::Source => {
                    self.sources.insert(idx, Source {location: idx, material: self.material});
                }
            }
        }
    }

    fn update(&mut self) {
        let mut rng = rand::thread_rng();
        let mut moved = HashMap::new();
        for order_idx in 0..self.order.len() {
            self.update_tile(order_idx, &mut rng, &mut moved);
        }
        for (idx, source) in &self.sources {
            self.grid[*idx].material = source.material;
        }
    }

    fn update_tile(&mut self, order_idx: usize, rng: &mut ThreadRng, moved: &mut HashMap<usize, usize>) {
        // Re-compute the x/y of the chosen location
        let idx = self.order[order_idx];
        let x = idx % self.width;
        let y = idx / self.width;

        let idx_above = if idx > self.width { idx - self.width } else { 0 };
        let idx_below = idx + self.width;
        let idx_left = if idx > 0 { idx - 1 } else { 0 };
        let idx_right = idx + 1;

        let choice = rng.gen_ratio(1, 2);

        // 0,0 is top left
        let particle = &self.grid[idx];
        let material = particle.material;
        let this_viscocity = viscosity(material);
        let mut particle_moved = false;

        if y < self.height - 1 && heavier(material, self.grid[idx_below].material) {
            particle_moved = self.try_swap(idx, idx_below, moved, 1);
        }

        if !particle_moved && y > 0 && heavier( self.grid[idx_above].material, material) {
            particle_moved = self.try_swap(idx_above, idx, moved, 1);

        }
        
        if !particle_moved && this_viscocity > 2 {
            for i in 0..this_viscocity {
                if choice && x > i {
                    let material_left = self.grid[idx - i].material;
                    if viscosity(material_left) > 4 && material != material_left {
                        particle_moved = self.try_swap(idx, idx - i, moved, i);
                        break;
                    }
                } else if !choice && x < (self.width - i - 1) {
                    let material_right = self.grid[idx + i].material;
                    if viscosity(material_right) > 4 && material != material_right {
                        particle_moved =self.try_swap(idx, idx + i, moved, i);
                        break;
                    }
                }
            }
        }

        if !particle_moved && this_viscocity > 1 {
            if choice && x > 0 {
                let material_left = self.grid[idx_left].material;
                if viscosity(material_left) > 1 && material != material_left {
                    particle_moved = self.try_swap(idx, idx_left, moved, 1);
                }
            } else if !choice && x < (self.width - 1) {
                let material_right = self.grid[idx_right].material;
                if viscosity(material_right) > 1 && material != material_right {
                    particle_moved = self.try_swap(idx, idx_right, moved, 1);
                }
            }
        }

        if !particle_moved && y < (self.height - 1) {
            if choice && x > 0 {
                let material_left = self.grid[idx_left].material;
                let idx_below_left = idx_below - 1;
                let material_below_left = self.grid[idx_below_left].material;
                if heavier(material, material_left) && heavier(material, material_below_left) {
                    particle_moved = self.try_swap(idx, idx_left, moved, 1);
                }
            } else if !choice && x < (self.width - 1) {
                let material_right = self.grid[idx_right].material;
                let idx_below_right = idx_below + 1;
                let material_below_right = self.grid[idx_below_right].material;
                if heavier(material, material_right) && heavier(material, material_below_right) {
                    particle_moved = self.try_swap(idx, idx_right, moved, 1);
                }
            }
        }

        if !particle_moved && y > 0 {
            let idx_above = idx - self.width;
            let material_above = self.grid[idx_above].material;
            if choice && x > 0 {
                let material_left = self.grid[idx_left].material;
                if heavier(material, material_left) && heavier(material_above, material_left) {
                    self.try_swap(idx, idx_left, moved, 1);
                }
            } else if !choice && x < (self.width - 1) {
                let material_right = self.grid[idx_right].material;
                if heavier(material, material_right) && heavier(material_above, material_right) {
                    self.try_swap(idx, idx_right, moved, 1);
                }
            }
        }
    }

    fn try_swap(&mut self, idx_a: usize, idx_b: usize, moved: &mut HashMap<usize, usize>, distance: usize) -> bool {
        if self.grid[idx_a].material == Material::Rock || self.grid[idx_b].material == Material::Rock {
            return false;
        }
        let moved_a = match moved.get(&idx_a) { Some(c) => *c, None => 0 };
        let moved_b = match moved.get(&idx_b) { Some(c) => *c, None => 0 };
        let viscocity_a = viscosity(self.grid[idx_a].material);
        let viscocity_b = viscosity(self.grid[idx_a].material);
        if moved_a < viscocity_a && moved_b < viscocity_b {
            self.grid.swap(idx_a, idx_b);
            moved.insert(idx_a, moved_a + distance);
            moved.insert(idx_b, moved_b + distance);
            true
        } else {
            false
        }
    }

    fn get_color(&self, pos: UVec2) -> Color {
        let y: usize = pos.y.try_into().unwrap();
        let x: usize = pos.x.try_into().unwrap();
        let idx: usize = y * self.width + x;
        self.get_tile_color(&self.grid[idx])
    }

    fn get_tile_color(&self, particle: &Particle) -> Color {
        match particle.material {
            Material::Gas => Color::srgba(0.2, 0.8, 0.1, 0.5 + particle.alpha),
            Material::Air => Color::srgba(0.0, 0.0, 0.0, particle.alpha),
            Material::Water => Color::srgba(0.0, 0.0, 1.0, 0.5 + particle.alpha),
            Material::Sand => Color::srgba(1.0, 1.0, 0.1, 0.5 + particle.alpha),
            Material::Rock => Color::srgba(1.0, 1.0, 1.0, 0.3 + particle.alpha),
        }
    }
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
                        .. default()
                    }),
                    .. default()
                })
                .build(),
            PixelBufferPlugin))
        .add_systems(Startup, (setup, pixel_buffer_setup(size)))
        // .insert_resource(Time::<Fixed>::from_seconds(0.01))
        // .add_systems(FixedUpdate, update)
        .add_systems(Update, update)
        .add_systems(Update, keyboard_input)
        .add_systems(Update, mouse_button_input)
        .run();
}

fn update(mut pb: QueryPixelBuffer, mut simulation: ResMut<Simulation>) {
    simulation.update();

    pb.frame().per_pixel(|pos, _| simulation.get_color(pos));
}

fn keyboard_input(
    mut simulation: ResMut<Simulation>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if keys.just_pressed(KeyCode::Space) {
        simulation.reset_random();
    }

    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    if keys.just_pressed(KeyCode::KeyA) {
        simulation.set_material(Material::Air, shift);
    }
    if keys.just_pressed(KeyCode::KeyG) {
        simulation.set_material(Material::Gas, shift);
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
