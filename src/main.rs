use bevy::prelude::*;
use bevy_pixel_buffer::prelude::*;
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

struct Particle {
    material: Material
}

fn viscosity(material: Material) -> usize {
    match material {
        Material::Gas => 4,
        Material::Air => 3,
        Material::Water => 2,
        Material::Sand => 1,
        Material::Rock => 0
    }
}

fn heavier(a: Material, b: Material) -> bool {
    a as usize > b as usize
}

#[derive(Resource)]
struct Simulation {
    width: usize,
    height: usize,
    grid: Vec<Particle>,
    order: Vec<usize>,
}

impl Simulation {
    fn new(width: usize, height: usize) -> Self {
        let mut rng = rand::thread_rng();
        let grid = (0..width * height)
            .map(|_| match rng.gen_range(0..5) {
                0 => Material::Gas,
                1 => Material::Air,
                2 => Material::Water,
                3 => Material::Sand,
                _ => Material::Rock,
            })
            .map(|m| Particle {material: m})
            .collect();
        let mut order: Vec<usize> = (0..width * height).map(|v| v).collect();
        order.shuffle(&mut rng);
        Self { width, height, grid, order }
    }

    fn update(&mut self) {
        let mut rng = rand::thread_rng();
        let mut moved = HashMap::new();
        for order_idx in 0..self.order.len() {
            self.update_tile(order_idx, &mut rng, &mut moved);
        }
    }

    fn update_tile(&mut self, order_idx: usize, rng: &mut ThreadRng, moved: &mut HashMap<usize, usize>) {
        // Re-compute the x/y of the chosen location
        let idx = self.order[order_idx];
        let x = idx % self.width;
        let y = idx / self.width;

        let idx_below = idx + self.width;
        let idx_left = if idx > 0 { idx - 1 } else { 0 };
        let idx_right = idx + 1;

        let choice = rng.gen_ratio(1, 2);

        // 0,0 is top left
        let particle = &self.grid[idx];
        let material = particle.material;
        let mut particle_moved = false;
        if y < self.height - 1 && heavier(material, self.grid[idx_below].material) {
            particle_moved = self.try_swap(idx, idx_below, moved);
        }
        
        if !particle_moved && viscosity(material) > 1 {
            if choice && x > 0 {
                let material_left = self.grid[idx_left].material;
                if viscosity(material_left) > 1 && heavier(material_left, material) {
                    particle_moved =self.try_swap(idx, idx_left, moved);
                }
            } else if !choice && x < (self.width - 1) {
                let material_right = self.grid[idx_right].material;
                if viscosity(material_right) > 1 && heavier(material_right, material) {
                    particle_moved =self.try_swap(idx, idx_right, moved);
                }
            }
        }

        if !particle_moved && y < (self.height - 1) {
            if choice && x > 0 {
                let material_left = self.grid[idx_left].material;
                let idx_below_left = idx_below - 1;
                let material_below_left = self.grid[idx_below_left].material;
                if heavier(material, material_left) && heavier(material, material_below_left) {
                    particle_moved = self.try_swap(idx, idx_left, moved);
                }
            } else if !choice && x < (self.width - 1) {
                let material_right = self.grid[idx_right].material;
                let idx_below_right = idx_below + 1;
                let material_below_right = self.grid[idx_below_right].material;
                if heavier(material, material_right) && heavier(material, material_below_right) {
                    particle_moved = self.try_swap(idx, idx_right, moved);
                }
            }
        }

        if !particle_moved && y > 0 {
            let idx_above = idx - self.width;
            let material_above = self.grid[idx_above].material;
            if choice && x > 0 {
                let material_left = self.grid[idx_left].material;
                if heavier(material, material_left) && heavier(material_above, material_left) {
                    particle_moved = self.try_swap(idx, idx_left, moved);
                }
            } else if !choice && x < (self.width - 1) {
                let material_right = self.grid[idx_right].material;
                if heavier(material, material_right) && heavier(material_above, material_right) {
                    particle_moved = self.try_swap(idx, idx_right, moved);
                }
            }
        }
    }

    fn try_swap(&mut self, idx_a: usize, idx_b: usize, moved: &mut HashMap<usize, usize>) -> bool {
        if self.grid[idx_a].material == Material::Rock || self.grid[idx_b].material == Material::Rock {
            return false;
        }
        let moved_a = match moved.get(&idx_a) { Some(c) => *c, None => 0 };
        let moved_b = match moved.get(&idx_b) { Some(c) => *c, None => 0 };
        let viscocity_a = viscosity(self.grid[idx_a].material);
        let viscocity_b = viscosity(self.grid[idx_a].material);
        if moved_a < viscocity_a && moved_b < viscocity_b {
            self.grid.swap(idx_a, idx_b);
            moved.insert(idx_a, moved_a + 1);
            moved.insert(idx_b, moved_b + 1);
            true
        } else {
            false
        }
    }

    fn get_color(&self, pos: UVec2) -> Color {
        let y: usize = pos.y.try_into().unwrap();
        let x: usize = pos.x.try_into().unwrap();
        let idx: usize = y * self.width + x;
        self.get_tile_color(self.grid[idx].material)
    }

    fn get_tile_color(&self, tile: Material) -> Color {
        match tile {
            Material::Gas => Color::srgba(0.2, 0.8, 0.1, 0.8),
            Material::Air => Color::srgba(0.0, 0.0, 0.0, 0.0),
            Material::Water => Color::srgba(0.0, 0.0, 1.0, 0.8),
            Material::Sand => Color::srgba(1.0, 1.0, 0.1, 0.8),
            Material::Rock => Color::srgba(1.0, 1.0, 1.0, 0.6),
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
        .insert_resource(Time::<Fixed>::from_seconds(0.05))
        .add_systems(FixedUpdate, update)
        .run();
}

fn update(mut pb: QueryPixelBuffer, mut simulation: ResMut<Simulation>) {
    simulation.update();
    pb.frame().per_pixel(|pos, _| simulation.get_color(pos));
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_water_falls() {
        let mut sim = Simulation {
            width: 3,
            height: 3,
            grid: vec![
                Material::Air, Material::Air, Material::Air,
                Material::Water, Material::Air, Material::Air,
                Material::Rock, Material::Rock, Material::Rock,
            ].iter().map(|&m| Particle {material:m}).collect(),
            order: (0..9).collect()
        };

        sim.update();

        assert_eq!(sim.grid[3].material, Material::Air); // Water should fall
        assert_eq!(sim.grid[0].material, Material::Water); // Water should now be at index 0
    }

    #[test]
    fn test_sand_falls() {
        let mut sim = Simulation {
            width: 3,
            height: 3,
            grid: vec![
                Material::Rock, Material::Rock, Material::Rock,
                Material::Sand, Material::Water, Material::Air,
                Material::Air, Material::Air, Material::Air,
            ].iter().map(|&m| Particle {material:m}).collect(),
            order: (0..9).collect()
        };

        for _ in 0..9 {
            sim.update();
        }

        assert_eq!(sim.grid[1].material, Material::Sand); // Sand should fall on water
        assert_eq!(sim.grid[3].material, Material::Air); // Sand should move down
    }

    #[test]
    fn test_sand_moves_sideways() {
        let mut sim = Simulation {
            width: 3,
            height: 3,
            grid: vec![
                Material::Air, Material::Air, Material::Air,
                Material::Sand, Material::Rock, Material::Air,
                Material::Rock, Material::Rock, Material::Rock,
            ].iter().map(|&m| Particle {material:m}).collect(),
            order: (0..9).collect()
        };

        sim.update();

        assert_eq!(sim.grid[2].material, Material::Sand); // Sand should move to the right
    }
}
