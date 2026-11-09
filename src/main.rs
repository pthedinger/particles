mod view;
use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::ScreenshotManager,
    window::{PresentMode, PrimaryWindow},
};
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use particles::{controls::Clock, material::Recipe, simulation::Simulation};
use std::time::Instant;

#[derive(Resource)]
pub struct Art {
    pub simulation: Simulation,
    pub recipe: Recipe,
    pub pending: Recipe,
    pub clock: Clock,
    pub paused: bool,
    pub angular_speed: f32,
    pub button_turn: f32,
    pub turn_remaining: f32,
    pub shake_remaining: f32,
    pub drag_angle: Option<f32>,
    pub error: Option<String>,
    pub frame_ms: f32,
    pub physics_ms: f32,
    pub drawing_ms: f32,
    pub field: view::FluidField,
    pub frames: u32,
    pub options: Options,
    pub samples: Vec<f32>,
    pub physics_samples: Vec<f32>,
}

#[derive(Default)]
pub struct Options {
    pub smoke_frames: Option<u32>,
    pub screenshot: Option<String>,
    pub report: Option<String>,
    pub scripted: bool,
}

impl Art {
    pub fn apply(&mut self, recipe: Recipe) {
        match Simulation::new(&recipe) {
            Ok(simulation) => {
                self.simulation = simulation;
                self.recipe = recipe.clone();
                self.pending = recipe;
                self.field.invalidate();
                self.clock = Clock::default();
                self.angular_speed = 0.0;
                self.turn_remaining = 0.0;
                self.shake_remaining = 0.0;
                self.drag_angle = None;
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut recipe = Recipe::balanced();
    let mut options = Options::default();
    let mut i = 1;
    while i < args.len() {
        let next = || args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--particles" => {
                recipe.budget = next().parse().unwrap_or(0);
                i += 1;
            }
            "--smoke-frames" => {
                options.smoke_frames = Some(next().parse().unwrap_or(0));
                i += 1;
            }
            "--screenshot" => {
                options.screenshot = Some(next());
                i += 1;
            }
            "--report" => {
                options.report = Some(next());
                i += 1;
            }
            "--scripted" => options.scripted = true,
            "--help" => {
                println!("Sand Art\n  --particles N\n  --smoke-frames N [--scripted] [--screenshot PATH] [--report PATH]\nHeadless benchmark: cargo run --release --bin benchmark -- 3000 600");
                return;
            }
            unknown => {
                eprintln!("Unknown argument: {unknown}; use --help");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    if options.smoke_frames.is_some_and(|n| n < 20) {
        eprintln!("Use at least 20 smoke frames.");
        std::process::exit(2);
    }
    let simulation = match Simulation::new(&recipe) {
        Ok(sim) => sim,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.035, 0.055, 0.07)))
        .insert_resource(Art {
            simulation,
            pending: recipe.clone(),
            recipe,
            clock: Clock::default(),
            paused: false,
            angular_speed: 0.0,
            button_turn: 0.0,
            turn_remaining: 0.0,
            shake_remaining: 0.0,
            drag_angle: None,
            error: None,
            frame_ms: 16.667,
            physics_ms: 0.0,
            drawing_ms: 0.0,
            field: view::FluidField::new(),
            frames: 0,
            options,
            samples: Vec::new(),
            physics_samples: Vec::new(),
        })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Sand Art — Matter in motion".into(),
                resolution: (1360.0, 900.0).into(),
                present_mode: PresentMode::AutoVsync,
                resize_constraints: bevy::window::WindowResizeConstraints {
                    min_width: 920.0,
                    min_height: 640.0,
                    ..default()
                },
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin)
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Camera2dBundle::default());
        })
        .add_systems(Update, (view::run_ui, smoke_test).chain())
        .run();
}

fn smoke_test(
    mut art: ResMut<Art>,
    time: Res<Time>,
    window: Query<Entity, With<PrimaryWindow>>,
    mut screenshots: ResMut<ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
) {
    art.frames += 1;
    if art.frames > 120 && !art.paused {
        art.samples.push(time.delta_seconds() * 1000.0);
        let physics = art.physics_ms;
        art.physics_samples.push(physics);
    }
    let Some(limit) = art.options.smoke_frames else {
        return;
    };
    if art.frames == limit.saturating_sub(5) {
        if let Some(path) = &art.options.screenshot {
            if let Err(error) = screenshots.save_screenshot_to_disk(window.single(), path) {
                error!("Screenshot failed: {error}");
            }
        }
    }
    if art.frames == limit {
        let mut samples = art.samples.clone();
        samples.sort_by(f32::total_cmp);
        let mut physics = art.physics_samples.clone();
        physics.sort_by(f32::total_cmp);
        let median = samples.get(samples.len() / 2).copied().unwrap_or(0.0);
        let p95 = samples
            .get(((samples.len() as f32 * 0.95) as usize).min(samples.len().saturating_sub(1)))
            .copied()
            .unwrap_or(0.0);
        let report = format!("Windowed smoke: {} frames; {} particles; {} samples\nFrame median {:.3} ms ({:.1} FPS), p95 {:.3} ms\nPhysics median {:.3} ms; simulation {:.3} s; dropped {:.3} s\nFinite/contained: {}\n", art.frames, art.simulation.particles.len(), samples.len(), median,1000.0/median.max(0.001),p95,physics.get(physics.len()/2).copied().unwrap_or(0.0),art.simulation.time,art.clock.dropped_time,art.simulation.particles.iter().all(|p| p.position.is_finite() && p.velocity.is_finite() && p.position.length()+p.radius<=1.001));
        print!("{report}");
        if let Some(path) = &art.options.report {
            if let Err(error) = std::fs::write(path, &report) {
                error!("Report write failed: {error}");
            }
        }
        exit.send(AppExit::Success);
    }
}

pub fn measured_ms(start: Instant) -> f32 {
    start.elapsed().as_secs_f32() * 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Art {
        let mut recipe = Recipe::balanced();
        recipe.budget = 80;
        let simulation = Simulation::new(&recipe).unwrap();
        Art {
            simulation,
            pending: recipe.clone(),
            recipe,
            clock: Clock::default(),
            paused: false,
            angular_speed: 0.0,
            button_turn: 0.0,
            turn_remaining: 0.0,
            shake_remaining: 0.0,
            drag_angle: None,
            error: None,
            frame_ms: 16.667,
            physics_ms: 0.0,
            drawing_ms: 0.0,
            field: view::FluidField::new(),
            frames: 0,
            options: Options::default(),
            samples: Vec::new(),
            physics_samples: Vec::new(),
        }
    }
    #[test]
    fn invalid_apply_preserves_running_mixture() {
        let mut art = fixture();
        let original = art.simulation.particles.clone();
        let mut invalid = art.recipe.clone();
        for m in &mut invalid.materials {
            m.share = 0.0;
        }
        art.apply(invalid);
        assert!(art.error.is_some());
        assert_eq!(art.simulation.particles, original);
        assert_eq!(art.recipe.budget, 80);
    }
    #[test]
    fn reset_reproduces_mixture_and_remix_changes_it() {
        let mut art = fixture();
        let original = art.simulation.particles.clone();
        art.simulation.step(1.0 / 120.0, Default::default());
        art.apply(art.recipe.clone());
        assert_eq!(art.simulation.particles, original);
        let mut next = art.recipe.clone();
        next.seed += 1;
        art.apply(next);
        assert_ne!(art.simulation.particles, original);
    }
}
