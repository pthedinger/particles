//! Completion-inclusive GPU benchmark. Each sample waits for submitted work.
use particles::{
    gpu::{Engine, Seed, Step},
    material::Recipe,
    simulation::Simulation,
};
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut recipe = Recipe::balanced();
    recipe.budget = args
        .get(1)
        .map(|s| s.parse().expect("particle count"))
        .unwrap_or(100_000);
    let frames = args
        .get(2)
        .map(|s| s.parse::<usize>().expect("frames"))
        .unwrap_or(300);
    assert!(frames > 0);
    let instance = wgpu::Instance::default();
    let adapter = bevy::tasks::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .expect("GPU required");
    println!("GPU: {:?}", adapter.get_info());
    let (d, q) = bevy::tasks::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("GPU benchmark"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
        },
        None,
    ))
    .unwrap();
    let seed = Seed::from(&Simulation::seeded(&recipe).unwrap());
    let mut engine = Engine::new(&d, &q, &seed);
    for mode in ["settling", "rotation", "shaking"] {
        let mut values = Vec::new();
        for i in 0..frames + 120 {
            let t = i as f32 / 60.;
            let step = Step {
                dt: 1. / 120.,
                angular_speed: if mode == "rotation" { 1.6 } else { 0. },
                shake: if mode == "shaking" {
                    [7. * (t * 37.).sin(), 4.5 * (t * 29.).cos()]
                } else {
                    [0.; 2]
                },
            };
            let start = std::time::Instant::now();
            engine.advance(&d, &q, &[step, step]);
            d.poll(wgpu::Maintain::Wait);
            if i >= 120 {
                values.push(start.elapsed().as_secs_f64() * 1000.);
            }
        }
        values.sort_by(f64::total_cmp);
        println!("{mode}: {} particles; {} frames; completion-inclusive median {:.3} ms, p95 {:.3} ms, max {:.3} ms / two 1/120 s steps",recipe.budget,frames,values[values.len()/2],values[(values.len()as f64*0.95)as usize],values[values.len()-1]);
    }
    let out = engine.read_particles(&d, &q);
    println!(
        "Finite/contained: {}; identities preserved: {}",
        out.iter().all(
            |p| p.position.iter().chain(&p.velocity).all(|v| v.is_finite())
                && p.position[0].hypot(p.position[1]) + p.radius <= 1.001
        ),
        out.iter()
            .zip(&seed.particles)
            .all(|(a, b)| a.material == b.material)
    );
}
