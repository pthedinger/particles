use bevy::prelude::*;
use bevy_pixel_buffer::prelude::*;
use rand::prelude::*;
use rand::seq::SliceRandom;
use std::collections::HashSet;

const GRID_WIDTH: usize = 640;
const GRID_HEIGHT: usize = 300;
const PIXEL_SIZE: usize = 2;

// Order is important - lighter at the top
#[derive(Debug, Clone, Copy, PartialEq)]
enum Tile {
    Gas,
    Air,
    Water,
    Sand,
    Rock,
}

fn viscosity(tile: Tile) -> usize {
    match tile {
        Tile::Gas => 4,
        Tile::Air => 3,
        Tile::Water => 2,
        Tile::Sand => 1,
        Tile::Rock => 0
    }
}

fn heavier(a: Tile, b: Tile) -> bool {
    a as usize > b as usize
}

#[derive(Resource)]
struct Simulation {
    width: usize,
    height: usize,
    grid: Vec<Tile>,
    order: Vec<usize>,
}

impl Simulation {
    fn new(width: usize, height: usize) -> Self {
        let mut rng = rand::thread_rng();
        let grid = (0..width * height)
            .map(|_| match rng.gen_range(0..5) {
                0 => Tile::Gas,
                1 => Tile::Air,
                2 => Tile::Water,
                3 => Tile::Sand,
                _ => Tile::Rock,
            })
            .collect();
        let mut order: Vec<usize> = (0..width * height).map(|v| v).collect();
        order.shuffle(&mut rng);
        Self { width, height, grid, order }
    }

    fn update(&mut self) {
        let mut rng = rand::thread_rng();
        let mut moved = HashSet::new();
        for order_idx in 0..self.order.len() {
            self.update_tile(order_idx, &mut rng, &mut moved);
        }
    }

    fn update_tile(&mut self, order_idx: usize, rng: &mut ThreadRng, moved: &mut HashSet<usize>) {
        // Re-compute the x/y of the chosen location
        let idx = self.order[order_idx];
        let x = idx % self.width;
        let y = idx / self.width;

        let idx_below = idx + self.width;
        let idx_left = if idx > 0 { idx - 1 } else { 0 };
        let idx_right = idx + 1;

        let choice = rng.gen_ratio(1, 2);

        // 0,0 is top left
        let tile = self.grid[idx];
        if y < self.height - 1 && heavier(tile, self.grid[idx_below]) {
            self.try_swap(idx, idx_below, moved);

        } else if viscosity(tile) > 1 {
            if choice && x > 0 {
                let tile_left = self.grid[idx_left];
                if viscosity(tile_left) > 1 && heavier(self.grid[idx_left], tile) {
                    self.try_swap(idx, idx_left, moved);
                }
            } else if !choice && x < (self.width - 1) {
                let tile_right = self.grid[idx_right];
                if viscosity(tile_right) > 1 && heavier(self.grid[idx_right], tile) {
                    self.try_swap(idx, idx_right, moved);
                }
            }

        } else if y < (self.height - 1) {
            if choice && x > 0 {
                let tile_left = self.grid[idx_left];
                let idx_below_left = idx_below - 1;
                let tile_below_left = self.grid[idx_below_left];
                if heavier(tile, tile_left) && heavier(tile, tile_below_left) {
                    self.try_swap(idx, idx_left, moved);
                }
            } else if !choice && x < (self.width - 1) {
                let tile_right = self.grid[idx_right];
                let idx_below_right = idx_below + 1;
                let tile_below_right = self.grid[idx_below_right];
                if heavier(tile, tile_right) && heavier(tile, tile_below_right) {
                    self.try_swap(idx, idx_right, moved);
                }
            }
        }
    }

    fn try_swap(&mut self, idx_a: usize, idx_b: usize, moved: &mut HashSet<usize>) {
        if self.grid[idx_a] == Tile::Rock || self.grid[idx_b] == Tile::Rock {
            return;
        }
        if !moved.contains(&idx_a) && !moved.contains(&idx_b) {
            self.grid.swap(idx_a, idx_b);
            moved.insert(idx_a);
            moved.insert(idx_b);
        }
    }

    fn get_color(&self, pos: UVec2) -> Color {
        let y: usize = pos.y.try_into().unwrap();
        let x: usize = pos.x.try_into().unwrap();
        let idx: usize = y * self.width + x;
        self.get_tile_color(self.grid[idx])
    }

    fn get_tile_color(&self, tile: Tile) -> Color {
        match tile {
            Tile::Gas => Color::srgba(0.2, 0.8, 0.1, 0.8),
            Tile::Air => Color::srgba(0.0, 0.0, 0.0, 0.0),
            Tile::Water => Color::srgba(0.0, 0.0, 1.0, 0.8),
            Tile::Sand => Color::srgba(1.0, 1.0, 0.1, 0.8),
            Tile::Rock => Color::srgba(1.0, 1.0, 1.0, 0.6),
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
                Tile::Air, Tile::Air, Tile::Air,
                Tile::Water, Tile::Air, Tile::Air,
                Tile::Rock, Tile::Rock, Tile::Rock,
            ],
            order: (0..9).collect()
        };

        sim.update();

        assert_eq!(sim.grid[3], Tile::Air); // Water should fall
        assert_eq!(sim.grid[0], Tile::Water); // Water should now be at index 0
    }

    #[test]
    fn test_sand_falls() {
        let mut sim = Simulation {
            width: 3,
            height: 3,
            grid: vec![
                Tile::Rock, Tile::Rock, Tile::Rock,
                Tile::Sand, Tile::Water, Tile::Air,
                Tile::Air, Tile::Air, Tile::Air,
            ],
            order: (0..9).collect()
        };

        for _ in 0..9 {
            sim.update();
        }

        assert_eq!(sim.grid[1], Tile::Sand); // Sand should fall on water
        assert_eq!(sim.grid[3], Tile::Air); // Sand should move down
    }

    #[test]
    fn test_sand_moves_sideways() {
        let mut sim = Simulation {
            width: 3,
            height: 3,
            grid: vec![
                Tile::Air, Tile::Air, Tile::Air,
                Tile::Sand, Tile::Rock, Tile::Air,
                Tile::Rock, Tile::Rock, Tile::Rock,
            ],
            order: (0..9).collect()
        };

        sim.update();

        assert_eq!(sim.grid[2], Tile::Sand); // Sand should move to the right
    }
}
