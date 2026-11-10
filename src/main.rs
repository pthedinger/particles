mod gpu_view;
mod view;
use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::ScreenshotManager,
    window::{PresentMode, PrimaryWindow},
};
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use particles::{
    controls::{bounded_motion, rotation_step, Clock, FIXED_DT},
    material::Recipe,
    simulation::Simulation,
};
use std::time::Instant;

#[derive(Resource)]
pub struct Art {
    pub simulation: Simulation,
    pub gpu: bool,
    pub revision: u64,
    pub gpu_texture: Option<bevy_inspector_egui::egui::TextureId>,
    pub gpu_steps: Vec<particles::gpu::Step>,
    pub active_wall: f32,
    pub measurement_start: Option<(f32, f32, f32)>,
    pub measurement_end: Option<(f32, f32, f32)>,
    pub reported: bool,
    pub recipe: Recipe,
    pub pending: Recipe,
    pub clock: Clock,
    pub paused: bool,
    pub angular_speed: f32,
    pub button_turn: f32,
    pub desired_speed: f32,
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
    screenshot_result: std::sync::Arc<std::sync::Mutex<Option<Result<(), String>>>>,
}

impl Art {
    pub fn apply(&mut self, recipe: Recipe) {
        let next = if self.gpu {
            Simulation::seeded(&recipe)
        } else {
            Simulation::new(&recipe)
        };
        match next {
            Ok(simulation) => {
                self.simulation = simulation;
                self.revision += 1;
                self.gpu_steps.clear();
                self.active_wall = 0.;
                self.measurement_start = None;
                self.measurement_end = None;
                self.reported = false;
                self.options.screenshot_result = Default::default();
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
    let gpu = !args.iter().any(|s| s == "--cpu");
    if gpu {
        recipe.budget = 100_000;
    }
    let mut i = 1;
    while i < args.len() {
        let next = || args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--cpu" => {}
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
                println!("Sand Art (GPU, 100,000 particles by default)\n  --cpu (CPU reference, 3,000 particles)\n  --particles N\n  --smoke-frames N [--scripted] [--screenshot PATH] [--report PATH]\nHeadless benchmark: cargo run --release --bin benchmark -- 3000 600");
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
    let initial = if gpu {
        Simulation::seeded(&recipe)
    } else {
        Simulation::new(&recipe)
    };
    let simulation = match initial {
        Ok(sim) => sim,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.035, 0.055, 0.07)))
        .insert_resource(Art {
            simulation,
            gpu,
            revision: 1,
            gpu_texture: None,
            gpu_steps: Vec::new(),
            active_wall: 0.,
            measurement_start: None,
            measurement_end: None,
            reported: false,
            pending: recipe.clone(),
            recipe,
            clock: Clock::default(),
            paused: false,
            angular_speed: 0.0,
            button_turn: 0.0,
            desired_speed: 0.0,
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
        .add_systems(Update, (view::run_ui, simulate, smoke_test).chain());
    if gpu {
        app.add_plugins(gpu_view::GpuArtPlugin);
    }
    app.run();
}

fn simulate(time: Res<Time<Real>>, mut art: ResMut<Art>) {
    let state = &mut *art;
    let elapsed = time.delta_seconds();
    state.frame_ms += (elapsed * 1000.0 - state.frame_ms) * 0.08;
    if !state.paused {
        state.active_wall += elapsed;
    }
    let steps = state.clock.advance(elapsed, state.paused);
    let start = Instant::now();
    for _ in 0..steps {
        (state.angular_speed, state.turn_remaining) = rotation_step(
            state.angular_speed,
            state.desired_speed,
            state.turn_remaining,
            FIXED_DT,
        );
        let shake = if state.shake_remaining > 0.0 {
            state.shake_remaining = (state.shake_remaining - FIXED_DT).max(0.0);
            let t = state.simulation.time;
            glam::Vec2::new(7.0 * (t * 37.0).sin(), 4.5 * (t * 29.0).cos())
        } else {
            glam::Vec2::ZERO
        };
        let motion = bounded_motion(state.angular_speed, shake);
        if state.gpu {
            state.gpu_steps.push(particles::gpu::Step {
                dt: FIXED_DT,
                angular_speed: motion.angular_velocity,
                shake: motion.shake.to_array(),
            });
            state.simulation.time += FIXED_DT;
            state.simulation.angle = (state.simulation.angle + motion.angular_velocity * FIXED_DT)
                .rem_euclid(std::f32::consts::TAU);
        } else {
            state.simulation.step(FIXED_DT, motion);
        }
    }
    if !state.gpu {
        state.physics_ms = measured_ms(start);
    }
}

fn smoke_test(
    mut art: ResMut<Art>,
    time: Res<Time<Real>>,
    window: Query<Entity, With<PrimaryWindow>>,
    mut screenshots: ResMut<ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
    mut gpu: Option<ResMut<gpu_view::GpuFrame>>,
) {
    art.frames += 1;
    if art.reported {
        return;
    }
    let warmup = art
        .options
        .smoke_frames
        .map(|n| (n / 4).min(120))
        .unwrap_or(120);
    if art.frames == warmup {
        let simulated = gpu
            .as_ref()
            .map(|g| g.feedback.lock().unwrap().simulated)
            .unwrap_or(art.simulation.time);
        art.measurement_start = Some((art.active_wall, simulated, art.clock.dropped_time));
    }
    if art.frames > warmup
        && !art.paused
        && art.options.smoke_frames.is_none_or(|n| art.frames <= n)
    {
        art.samples.push(time.delta_seconds() * 1000.0);
        let physics = art.physics_ms;
        art.physics_samples.push(physics);
    }
    let Some(limit) = art.options.smoke_frames else {
        return;
    };
    if art.frames == limit.saturating_add(1) {
        if let Some(path) = art.options.screenshot.clone() {
            let result = art.options.screenshot_result.clone();
            if let Err(error) = screenshots.take_screenshot(window.single(), move |image| {
                let saved = image
                    .try_into_dynamic()
                    .map_err(|e| e.to_string())
                    .and_then(|image| image.to_rgb8().save(&path).map_err(|e| e.to_string()));
                *result.lock().unwrap() = Some(saved);
            }) {
                *art.options.screenshot_result.lock().unwrap() = Some(Err(error.to_string()));
            }
        }
    }
    if art.frames == limit {
        let simulated = gpu
            .as_ref()
            .map(|g| g.feedback.lock().unwrap().simulated)
            .unwrap_or(art.simulation.time);
        art.measurement_end = Some((art.active_wall, simulated, art.clock.dropped_time));
    }
    if art.frames >= limit && art.gpu {
        let frame = gpu.as_mut().unwrap();
        frame.check = true;
        if !frame.feedback.lock().unwrap().checked {
            return;
        }
    }
    if art.frames >= limit {
        if art.options.screenshot.is_some()
            && art.options.screenshot_result.lock().unwrap().is_none()
        {
            return;
        }
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
        let report = if art.gpu {
            let f = gpu.as_ref().unwrap().feedback.lock().unwrap();
            let start = art.measurement_start.unwrap_or_default();
            let end = art.measurement_end.unwrap_or_default();
            let elapsed = end.0 - start.0;
            let simulated = end.1 - start.1;
            let dropped = end.2 - start.2;
            format!("GPU windowed: target {} particles; {} measured frames\nFrame median {:.3} ms ({:.1} FPS), p95 {:.3} ms\nCPU submit median {:.3} ms (not GPU execution time)\nSimulation {:.3} s / active wall {:.3} s; ratio {:.3}; dropped {:.3} s\nActual GPU state: finite/contained {}; identities preserved {}; visible pixels {}\nGrains: {}; mean y {:.3}; upper/open {}/{}\n",art.simulation.particles.len(),samples.len(),median,1000./median.max(0.001),p95,physics.get(physics.len()/2).copied().unwrap_or(0.),simulated,elapsed,simulated/elapsed.max(0.001),dropped,f.finite_contained,f.identities_preserved,f.visible_pixels,f.grains,f.grain_mean_y,f.suspended,f.grains)
        } else {
            report
        };
        print!("{report}");
        if let Some(path) = &art.options.report {
            if let Err(error) = std::fs::write(path, &report) {
                error!("Report write failed: {error}");
            }
        }
        art.reported = true;
        let valid = gpu.as_ref().is_none_or(|g| {
            let f = g.feedback.lock().unwrap();
            f.finite_contained && f.identities_preserved && f.visible_pixels > 0
        });
        let capture_ok = art
            .options
            .screenshot_result
            .lock()
            .unwrap()
            .as_ref()
            .is_none_or(|r| r.is_ok());
        if let Some(Err(error)) = art.options.screenshot_result.lock().unwrap().as_ref() {
            error!("Screenshot failed: {error}");
        }
        exit.send(if valid && capture_ok {
            AppExit::Success
        } else {
            AppExit::error()
        });
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
            gpu: false,
            revision: 1,
            gpu_texture: None,
            gpu_steps: Vec::new(),
            active_wall: 0.,
            measurement_start: None,
            measurement_end: None,
            reported: false,
            pending: recipe.clone(),
            recipe,
            clock: Clock::default(),
            paused: false,
            angular_speed: 0.0,
            button_turn: 0.0,
            desired_speed: 0.0,
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
    #[test]
    fn real_one_second_frame_reports_all_lost_simulation_time() {
        use bevy::time::TimeUpdateStrategy;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_secs(1),
            ))
            .insert_resource(fixture())
            .add_systems(Update, simulate);
        app.update();
        app.update();
        let art = app.world().resource::<Art>();
        assert!(
            art.clock.dropped_time > 0.96,
            "lost time was only {}",
            art.clock.dropped_time
        );
        assert!(art.simulation.time < 0.034);
    }
}
