//! Run explicitly with cargo test --release --test gpu -- --ignored --test-threads=1.
//! These tests execute real compute shaders; no software/CPU solver fallback.
use particles::{
    gpu::{Engine, GpuParticle, Seed, Step},
    material::Recipe,
    simulation::Simulation,
};
fn device() -> (wgpu::Device, wgpu::Queue) {
    bevy::tasks::block_on(async {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .expect("GPU adapter required");
        adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("physics acceptance"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .unwrap()
    })
}
fn seed() -> Seed {
    let mut recipe = Recipe::balanced();
    recipe.budget = 100;
    let sim = Simulation::seeded(&recipe).unwrap();
    Seed::from(&sim)
}
fn p(x: f32, y: f32, material: u32) -> GpuParticle {
    GpuParticle {
        position: [x, y],
        velocity: [0.; 2],
        radius: 0.02,
        material,
        id: 0,
        padding: 0,
    }
}
#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_gravity_advances_particle_and_contains_it() {
    let (d, q) = device();
    let mut s = seed();
    s.particles = vec![p(0., 0., 3)];
    let mut e = Engine::new(&d, &q, &s);
    e.advance(&d, &q, &vec![Step::default(); 240]);
    let out = e.read_particles(&d, &q);
    assert!(
        out[0].position[1] < -0.5,
        "gravity did not advance actual GPU state: {:?}",
        out[0]
    );
    assert!(out[0].position[0].hypot(out[0].position[1]) + out[0].radius <= 1.001);
}
#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_contact_separates_overlap_without_gluing_a_gap() {
    let (d, q) = device();
    let mut s = seed();
    s.base_radius = 0.02;
    s.particles = vec![p(-0.015, 0., 3), p(0.015, 0., 3)];
    s.particles[0].velocity[1] = 0.1;
    s.particles[1].velocity[1] = -0.1;
    let mut e = Engine::new(&d, &q, &s);
    e.advance(&d, &q, &[Step::default()]);
    let out = e.read_particles(&d, &q);
    assert!(
        (out[0].velocity[1] - out[1].velocity[1]).abs() < 0.1,
        "loaded contact friction reversed or preserved sliding: {:?}",
        out
    );
    assert!(
        out[1].position[0] - out[0].position[0] > 0.035,
        "loaded grains failed to separate"
    );
    s.particles = vec![p(-0.0208, 0., 3), p(0.0208, 0., 3)];
    s.particles[0].velocity[1] = 0.1;
    s.particles[1].velocity[1] = -0.1;
    s.materials[3].viscosity = 8.;
    let mut e = Engine::new(&d, &q, &s);
    e.advance(&d, &q, &[Step::default()]);
    let out = e.read_particles(&d, &q);
    assert!(
        (out[0].velocity[1] - out[1].velocity[1]).abs() > 0.18,
        "unloaded grains glued together"
    );
}
#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_fluid_sample_cannot_wedge_grains_apart() {
    let (d, q) = device();
    let mut s = seed();
    s.base_radius = 0.02;
    s.particles = vec![p(-0.021, 0., 3), p(0.021, 0., 3), p(0., 0., 0)];
    for m in &mut s.materials {
        m.density = 1.;
        m.viscosity = 0.;
        m.friction = 0.;
    }
    let mut e = Engine::new(&d, &q, &s);
    e.advance(&d, &q, &[Step::default()]);
    let out = e.read_particles(&d, &q);
    assert!((out[1].position[0] - out[0].position[0] - 0.042).abs() < 1e-5);
}
#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_crowded_cell_does_not_drop_neighbours() {
    let (d, q) = device();
    let mut s = seed();
    s.base_radius = 0.02;
    s.particles = (0..600)
        .map(|i| p((i % 25) as f32 * 0.0001, (i / 25) as f32 * 0.0001, 3))
        .collect();
    let mut e = Engine::new(&d, &q, &s);
    e.advance(&d, &q, &[Step::default()]);
    let out = e.read_particles(&d, &q);
    assert_eq!(out.len(), 600);
    assert!(
        out.iter().enumerate().all(|(i, p)| p.id == i as u32),
        "crowded scatter lost or duplicated a particle"
    );
    assert!(out
        .iter()
        .all(|p| p.position[0].is_finite() && p.position[1].is_finite()));
    assert!(
        out.iter()
            .filter(|p| p.position[0].abs() > 0.003 || p.position[1].abs() > 0.003)
            .count()
            > 500,
        "crowded cell lost its contacts"
    );
}
#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_100k_driven_mixture_preserves_identity_and_containment() {
    let (d, q) = device();
    let mut recipe = Recipe::balanced();
    recipe.budget = 100_000;
    let sim = Simulation::seeded(&recipe).unwrap();
    let s = Seed::from(&sim);
    let mut e = Engine::new(&d, &q, &s);
    let steps: Vec<_> = (0..240)
        .map(|i| Step {
            dt: 1. / 120.,
            angular_speed: 1.6,
            shake: [7. * (i as f32 * 0.3).sin(), 4. * (i as f32 * 0.2).cos()],
        })
        .collect();
    e.advance(&d, &q, &steps);
    let out = e.read_particles(&d, &q);
    assert_eq!(out.len(), 100_000);
    for (a, b) in s.particles.iter().zip(&out) {
        assert_eq!(a.material, b.material);
        assert!(b.position.iter().chain(&b.velocity).all(|v| v.is_finite()));
        assert!(b.position[0].hypot(b.position[1]) + b.radius <= 1.001);
    }
    assert!(
        out.iter()
            .zip(&s.particles)
            .filter(|(a, b)| (a.position[1] - b.position[1]).abs() > 0.02)
            .count()
            > 1000
    );
}
#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_draw_reads_particle_state_and_live_material_colour() {
    let (d, q) = device();
    let mut s = seed();
    s.particles = vec![p(0., 0., 3)];
    s.particles[0].radius = 0.2;
    let e = Engine::new(&d, &q, &s);
    let texture = d.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let pixels = |e: &Engine| {
        e.draw(&d, &q, &texture.create_view(&Default::default()));
        let b = d.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 64 * 256,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = d.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::ImageCopyBuffer {
                buffer: &b,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
        );
        q.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        b.slice(..)
            .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
        d.poll(wgpu::Maintain::Wait);
        rx.recv().unwrap().unwrap();
        let view = b.slice(..).get_mapped_range();
        let out = view.to_vec();
        drop(view);
        b.unmap();
        out
    };
    let before = pixels(&e);
    let center = (32 * 64 + 32) * 4;
    assert!(before[center] > 100, "grain was not drawn from GPU state");
    s.materials[3].color = [0., 0., 1., 1.];
    e.upload_materials(&q, &s.materials);
    let after = pixels(&e);
    assert!(
        after[center + 2] > after[center] + 100,
        "live colour did not reach GPU renderer"
    );
}
#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_full_mixture_settles_without_a_suspended_grain_scaffold() {
    let (d, q) = device();
    let mut recipe = Recipe::balanced();
    recipe.budget = 1200;
    let sim = Simulation::seeded(&recipe).unwrap();
    let s = Seed::from(&sim);
    let mut e = Engine::new(&d, &q, &s);
    let steps: Vec<_> = (0..1200)
        .map(|i| {
            let t = i as f32 / 120.;
            if i < 180 {
                Step {
                    angular_speed: 1.6,
                    shake: [7. * (t * 37.).sin(), 4.5 * (t * 29.).cos()],
                    ..Step::default()
                }
            } else {
                Step::default()
            }
        })
        .collect();
    e.advance(&d, &q, &steps);
    let out = e.read_particles(&d, &q);
    let grains: Vec<_> = out
        .iter()
        .filter(|p| s.materials[p.material as usize].phase == 2)
        .collect();
    let suspended = grains
        .iter()
        .filter(|p| p.position[1] > -0.3 && p.position[0].hypot(p.position[1]) < 0.9)
        .count();
    println!("GPU full mixture upper/open {suspended}/{}", grains.len());
    assert!((suspended as f32) / (grains.len() as f32) < 0.15);
}
#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_high_resolution_liquid_keeps_its_bulk_volume() {
    let (d, q) = device();
    let mut r = Recipe::balanced();
    r.budget = 100_000;
    for (i, m) in r.materials.iter_mut().enumerate() {
        m.share = if i == 0 { 1. } else { 0. };
    }
    let s = Seed::from(&Simulation::seeded(&r).unwrap());
    let mut e = Engine::new(&d, &q, &s);
    e.advance(&d, &q, &vec![Step::default(); 600]);
    let out = e.read_particles(&d, &q);
    // 72% initial occupied area should fill most of the circle. A mean below
    // -0.35 means that the nominally incompressible liquid collapsed into a cap.
    let mean = out.iter().map(|p| p.position[1]).sum::<f32>() / out.len() as f32;
    println!("100k liquid mean y {mean}");
    assert!(
        mean > -0.35,
        "liquid collapsed under its own weight: mean y {mean}"
    );
}
#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_fast_small_grains_cannot_tunnel_through_each_other() {
    let (d, q) = device();
    for separation in [0.02, 0.004] {
        let mut s = seed();
        s.base_radius = 0.0027;
        s.particles = vec![p(-separation * 0.5, 0., 3), p(separation * 0.5, 0., 3)];
        for p in &mut s.particles {
            p.radius = 0.0027;
        }
        s.particles[0].velocity = [2.4, 0.];
        s.particles[1].velocity = [-2.4, 0.];
        s.materials[3].friction = 0.;
        let mut e = Engine::new(&d, &q, &s);
        e.advance(&d, &q, &[Step::default()]);
        let out = e.read_particles(&d, &q);
        assert!(
            out[0].position[0] < out[1].position[0],
            "fast grains crossed: {:?}",
            out
        );
        assert!(
            out[1].position[0] - out[0].position[0] >= 0.0053,
            "swept impact was not resolved"
        );
    }
}
#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_maximum_valid_material_sizes_fit_the_grid_scan() {
    let (d, q) = device();
    let mut r = Recipe::balanced();
    r.budget = 100_000;
    for m in &mut r.materials {
        m.radius = 2.;
    }
    let s = Seed::from(&Simulation::seeded(&r).unwrap());
    let mut e = Engine::new(&d, &q, &s);
    e.advance(&d, &q, &[Step::default()]);
    let out = e.read_particles(&d, &q);
    assert_eq!(out.len(), 100_000);
    assert!(out
        .iter()
        .enumerate()
        .all(|(i, p)| p.id == i as u32 && p.position.iter().all(|v| v.is_finite())));
}
#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_gas_retains_resolved_pockets_in_a_driven_liquid_mixture() {
    let (d, q) = device();
    let mut r = Recipe::balanced();
    r.budget = 100_000;
    let s = Seed::from(&Simulation::seeded(&r).unwrap());
    let mut e = Engine::new(&d, &q, &s);
    let steps: Vec<_> = (0..1200)
        .map(|i| {
            let t = i as f32 / 120.;
            if (120..480).contains(&i) {
                Step {
                    angular_speed: 1.6,
                    ..Step::default()
                }
            } else if (480..840).contains(&i) {
                Step {
                    shake: [7. * (t * 37.).sin(), 4.5 * (t * 29.).cos()],
                    ..Step::default()
                }
            } else {
                Step::default()
            }
        })
        .collect();
    e.advance(&d, &q, &steps);
    let out = e.read_particles(&d, &q);
    let h = s.base_radius * 4.4;
    let side = e.side as usize;
    let mut cells = vec![0usize; side * side];
    for p in &out {
        if s.materials[p.material as usize].phase != 1 {
            continue;
        }
        let x =
            (((p.position[0] + 1.) / h).floor() as isize + 1).clamp(0, side as isize - 1) as usize;
        let y =
            (((p.position[1] + 1.) / h).floor() as isize + 1).clamp(0, side as isize - 1) as usize;
        cells[y * side + x] += 1;
    }
    let max = *cells.iter().max().unwrap();
    println!("Gas peak cell population {max}");
    assert!(
        max < 64,
        "liquid coupling collapsed gas into {max} samples per cell"
    );
}
#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_large_grain_bed_retains_volume_under_its_own_weight() {
    let (d, q) = device();
    let mut r = Recipe::balanced();
    r.budget = 12_000;
    for (i, m) in r.materials.iter_mut().enumerate() {
        m.share = if i == 3 { 1. } else { 0. };
    }
    let s = Seed::from(&Simulation::seeded(&r).unwrap());
    let mut e = Engine::new(&d, &q, &s);
    e.advance(&d, &q, &vec![Step::default(); 1200]);
    let out = e.read_particles(&d, &q);
    let mean = out.iter().map(|p| p.position[1]).sum::<f32>() / out.len() as f32;
    let speed = out
        .iter()
        .map(|p| p.velocity[0].hypot(p.velocity[1]))
        .sum::<f32>()
        / out.len() as f32;
    println!("Grain bed mean y {mean}, mean speed {speed}");
    assert!(mean > -0.25, "grain bed collapsed into a dense cap: {mean}");
    assert!(speed < 0.12, "settled bed keeps generating motion: {speed}");
}
#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_cached_support_disappears_when_grains_separate() {
    let (d, q) = device();
    let mut s = seed();
    s.base_radius = 0.02;
    s.particles = vec![p(-0.019, 0., 3), p(0.019, 0., 3)];
    let mut running = Engine::new(&d, &q, &s);
    running.advance(&d, &q, &[Step::default(), Step::default()]);
    let separated = running.read_particles(&d, &q);
    assert!(
        separated[1].position[0] - separated[0].position[0] > 0.0401,
        "fixture did not separate"
    );
    s.particles = separated;
    let mut fresh = Engine::new(&d, &q, &s);
    running.advance(&d, &q, &[Step::default()]);
    fresh.advance(&d, &q, &[Step::default()]);
    let a = running.read_particles(&d, &q);
    let b = fresh.read_particles(&d, &q);
    for (a, b) in a.iter().zip(&b) {
        for i in 0..2 {
            assert!(
                (a.velocity[i] - b.velocity[i]).abs() < 0.0001,
                "old contact exerted force across a gap: {:?} vs {:?}",
                a,
                b
            );
        }
    }
}
