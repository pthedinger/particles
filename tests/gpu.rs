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

    let before = draw_pixels(&d, &q, &e);
    let center = (32 * 64 + 32) * 4;
    assert!(before[center] > 100, "grain was not drawn from GPU state");
    s.materials[3].color = [0., 0., 1., 1.];
    e.upload_materials(&q, &s.materials);
    let after = draw_pixels(&d, &q, &e);
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
#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_temperature_changes_thermal_motion_without_changing_identity() {
    let (d, q) = device();
    let mut recipe = Recipe::balanced();
    recipe.budget = 100;
    recipe.materials[2].thermal_motion = 1.;
    recipe.temperature_c = -20.;
    let cold = Simulation::seeded(&recipe).unwrap();
    recipe.temperature_c = 150.;
    let hot = Simulation::seeded(&recipe).unwrap();
    let run = |sim: &Simulation, noise: bool| {
        let mut s = Seed::from(sim);
        s.particles = vec![p(0., 0., 2)];
        if !noise {
            for m in &mut s.materials {
                m.thermal_motion = 0.;
            }
        }
        let mut e = Engine::new(&d, &q, &s);
        e.advance(&d, &q, &[Step::default()]);
        e.read_particles(&d, &q)[0]
    };
    let c = run(&cold, true);
    let h = run(&hot, true);
    let baseline = run(&cold, false);
    let delta = |p: GpuParticle| {
        ((p.velocity[0] - baseline.velocity[0]).powi(2)
            + (p.velocity[1] - baseline.velocity[1]).powi(2))
        .sqrt()
    };
    assert!(
        delta(h) > delta(c) * 1.2,
        "temperature did not change GPU thermal motion"
    );
    assert_eq!(h.material, 2);
    assert_eq!(h.radius, c.radius);
}

#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_material_controls_change_cohesion_mixing_and_compression() {
    let (d, q) = device();
    let run = |s: &Seed| {
        let mut e = Engine::new(&d, &q, s);
        e.advance(&d, &q, &[Step::default()]);
        e.read_particles(&d, &q)
    };
    let width = |p: &[GpuParticle]| {
        p.iter()
            .map(|p| p.position[0])
            .fold(f32::NEG_INFINITY, f32::max)
            - p.iter()
                .map(|p| p.position[0])
                .fold(f32::INFINITY, f32::min)
    };
    let mut s = seed();
    s.base_radius = 0.02;
    for m in &mut s.materials {
        m.thermal_motion = 0.;
        m.viscosity = 0.;
        m.surface_tension = 0.;
        m.mixing_resistance = 0.;
    }
    s.particles = vec![p(-0.025, 0., 0), p(0.025, 0., 0)];
    let neutral = width(&run(&s));
    s.materials[0].surface_tension = 2.;
    let cohesive = width(&run(&s));
    assert!(
        cohesive < neutral - 1e-6,
        "cohesion had no effect: {cohesive} / {neutral}"
    );
    s.materials[0].surface_tension = 0.;
    s.particles[1].material = 1;
    let miscible = width(&run(&s));
    s.materials[0].mixing_resistance = 1.;
    s.materials[1].mixing_resistance = 1.;
    let immiscible = width(&run(&s));
    assert!(
        immiscible > miscible + 1e-6,
        "mixing resistance had no effect"
    );
    s.particles = (0..9)
        .map(|i| {
            p(
                (i % 3) as f32 * 0.002 - 0.002,
                (i / 3) as f32 * 0.002 - 0.002,
                0,
            )
        })
        .collect();
    s.materials[0].compression_resistance = 0.05;
    let soft = width(&run(&s));
    s.materials[0].compression_resistance = 1.;
    let firm = width(&run(&s));
    assert!(
        firm > soft + 1e-4,
        "compression control had no effect: {firm} / {soft}"
    );
}

#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_pure_fluid_size_and_temperature_endpoints_stay_finite() {
    let (d, q) = device();
    for (material, radius, temperature) in
        [(0, 0.5, -20.), (0, 2., 150.), (2, 0.5, 150.), (2, 2., -20.)]
    {
        let mut recipe = Recipe::balanced();
        recipe.budget = 12_000;
        recipe.set_percentage(material, 100.).unwrap();
        recipe.materials[material].radius = radius;
        recipe.temperature_c = temperature;
        let seed = Seed::from(&Simulation::seeded(&recipe).unwrap());
        let mut engine = Engine::new(&d, &q, &seed);
        engine.advance(&d, &q, &vec![Step::default(); 240]);
        let out = engine.read_particles(&d, &q);
        assert_eq!(out.len(), recipe.budget);
        assert!(out
            .iter()
            .enumerate()
            .all(|(i, p)| p.id == i as u32 && p.material == material as u32));
        assert!(out.iter().all(
            |p| p.position.iter().chain(&p.velocity).all(|v| v.is_finite())
                && p.position[0].hypot(p.position[1]) + p.radius <= 1.001
        ));
    }
}

#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_vessel_shelf_stops_a_falling_grain() {
    let (d, q) = device();
    let mut r = Recipe::balanced();
    r.vessel = particles::vessel::VesselKind::Cascade;
    r.budget = 1200;
    let mut s = Seed::from(&Simulation::seeded(&r).unwrap());
    s.base_radius = 0.012;
    s.particles = vec![p(-0.25, 0.65, 3)];
    s.particles[0].radius = 0.012;
    s.particles[0].velocity[1] = -2.;
    let mut e = Engine::new(&d, &q, &s);
    e.advance(&d, &q, &vec![Step::default(); 300]);
    let out = e.read_particles(&d, &q);
    assert!(
        out[0].position[1] > 0.43,
        "grain fell through shelf: {:?}",
        out[0]
    );
    assert!(out[0].velocity[0].hypot(out[0].velocity[1]) < 0.04);
}
#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_hourglass_transfers_grains_through_the_neck() {
    let (d, q) = device();
    let mut r = Recipe::balanced();
    r.vessel = particles::vessel::VesselKind::Hourglass;
    r.budget = 12000;
    r.set_percentage(3, 100.).unwrap();
    let s = Seed::from(&Simulation::seeded(&r).unwrap());
    let mut e = Engine::new(&d, &q, &s);
    e.advance(&d, &q, &vec![Step::default(); 600]);
    let out = e.read_particles(&d, &q);
    let transferred = out.iter().filter(|p| p.position[1] < -0.12).count();
    assert!(transferred > 100, "no usable hourglass flow: {transferred}");
}

#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_rotating_vessels_keep_all_materials_out_of_walls() {
    let (d, q) = device();
    for kind in particles::vessel::VesselKind::ALL.into_iter().skip(1) {
        let mut r = Recipe::balanced();
        r.vessel = kind;
        r.budget = 12000;
        let s = Seed::from(&Simulation::seeded(&r).unwrap());
        let mut e = Engine::new(&d, &q, &s);
        let steps: Vec<_> = (0..240)
            .map(|i| Step {
                dt: 1. / 120.,
                angular_speed: 1.6,
                shake: [4. * (i as f32 * 0.4).sin(), 3. * (i as f32 * 0.3).cos()],
            })
            .collect();
        e.advance(&d, &q, &steps);
        let out = e.read_particles(&d, &q);
        let min_clearance = out
            .iter()
            .map(|p| {
                s.vessel
                    .clearance(glam::Vec2::from_array(p.position), e.angle)
                    - p.radius
            })
            .fold(f32::INFINITY, f32::min);
        assert!(
            min_clearance > -0.0005,
            "{kind:?} wall penetration {min_clearance}"
        );
        assert!(out.iter().enumerate().all(|(i, p)| p.id == i as u32
            && p.material == s.particles[i].material
            && p.velocity.iter().all(|v| v.is_finite())));
        assert!((e.angle - 3.2).abs() < 0.001);
    }
}
#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_compact_100k_large_reference_radii_fit_grid_and_remain_inside() {
    let (d, q) = device();
    let mut r = Recipe::balanced();
    r.vessel = particles::vessel::VesselKind::Hourglass;
    r.budget = 100000;
    r.set_percentage(0, 100.).unwrap();
    for m in &mut r.materials {
        m.radius = 2.;
    }
    let s = Seed::from(&Simulation::seeded(&r).unwrap());
    let mut e = Engine::new(&d, &q, &s);
    assert!(e.side <= 360);
    e.advance(&d, &q, &vec![Step::default(); 30]);
    let out = e.read_particles(&d, &q);
    assert_eq!(out.len(), 100000);
    assert!(out.iter().enumerate().all(|(i, p)| p.id == i as u32
        && s.vessel
            .clearance(glam::Vec2::from_array(p.position), e.angle)
            + 0.0005
            >= p.radius));
}
#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_fluid_viscosity_does_not_act_through_a_shelf() {
    let (d, q) = device();
    let mut r = Recipe::balanced();
    r.vessel = particles::vessel::VesselKind::Cascade;
    r.budget = 1200;
    let mut s = Seed::from(&Simulation::seeded(&r).unwrap());
    s.base_radius = 0.04;
    s.particles = vec![p(-0.25, 0.33, 0)];
    s.particles[0].radius = 0.004;
    s.particles[0].velocity = [0.1, 0.];
    s.materials[0].viscosity = 8.;
    s.materials[0].thermal_motion = 0.;
    s.materials[0].surface_tension = 0.;
    let mut single = Engine::new(&d, &q, &s);
    single.advance(&d, &q, &[Step::default()]);
    let baseline = single.read_particles(&d, &q)[0];
    let mut other = p(-0.25, 0.43, 0);
    other.radius = 0.004;
    other.velocity = [-0.1, 0.];
    s.particles.push(other);
    let mut pair = Engine::new(&d, &q, &s);
    pair.advance(&d, &q, &[Step::default()]);
    let actual = pair.read_particles(&d, &q)[0];
    assert!(
        (actual.velocity[0] - baseline.velocity[0]).abs() < 1e-5,
        "viscosity leaked through wall: {actual:?} vs {baseline:?}"
    );
}

fn draw_pixels(d: &wgpu::Device, q: &wgpu::Queue, e: &Engine) -> Vec<u8> {
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
}

#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_vessel_render_masks_fluid_and_grains_at_rotated_walls() {
    let (d, q) = device();
    let mut r = Recipe::balanced();
    r.vessel = particles::vessel::VesselKind::Cascade;
    r.budget = 1200;
    for angle in [0., 0.7] {
        for material in [0, 3] {
            let mut s = Seed::from(&Simulation::seeded(&r).unwrap());
            s.angle = angle;
            let world = glam::Vec2::from_angle(angle).rotate(glam::Vec2::new(-0.25, 0.38));
            s.particles = vec![p(world.x, world.y, material)];
            s.particles[0].radius = 0.06;
            s.materials[material as usize].color = [0., 0., 1., 1.];
            let e = Engine::new(&d, &q, &s);
            let pixels = draw_pixels(&d, &q, &e);
            let x = ((world.x + 1.) * 32.) as usize;
            let y = ((1. - world.y) * 32.) as usize;
            let at = (y * 64 + x) * 4;
            assert!(
                pixels[at + 2] < 110,
                "blue material painted inside rotated shelf: {:?}",
                &pixels[at..at + 4]
            );
        }
    }
}

#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_rotating_shelf_does_not_sweep_a_fast_grain_through() {
    let (d, q) = device();
    let mut r = Recipe::balanced();
    r.vessel = particles::vessel::VesselKind::Cascade;
    r.budget = 1200;
    let mut s = Seed::from(&Simulation::seeded(&r).unwrap());
    s.base_radius = 0.012;
    s.particles = vec![p(-0.58, 0.429, 3)];
    s.particles[0].radius = 0.003;
    s.particles[0].velocity = [0., -2.5];
    s.materials[3].friction = 0.;
    let mut e = Engine::new(&d, &q, &s);
    e.advance(
        &d,
        &q,
        &[Step {
            dt: 1. / 60.,
            angular_speed: -3.,
            shake: [0.; 2],
        }],
    );
    let out = e.read_particles(&d, &q);
    let local = glam::Vec2::from_angle(-e.angle).rotate(glam::Vec2::from_array(out[0].position));
    assert!(local.y > 0.425, "rotating shelf crossed grain: {local}");
}
#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_asymmetric_and_crowded_hidden_neighbours_do_not_couple_through_shelf() {
    let (d, q) = device();
    let mut r = Recipe::balanced();
    r.vessel = particles::vessel::VesselKind::Cascade;
    r.budget = 1200;
    let mut s = Seed::from(&Simulation::seeded(&r).unwrap());
    s.base_radius = 0.09;
    s.materials[0].viscosity = 8.;
    s.materials[0].thermal_motion = 0.;
    s.materials[0].surface_tension = 0.;
    s.particles = vec![p(-0.25, 0.30, 0)];
    s.particles[0].radius = 0.004;
    s.particles[0].velocity = [0.1, 0.];
    let run = |s: &Seed| {
        let mut e = Engine::new(&d, &q, s);
        e.advance(&d, &q, &[Step::default()]);
        e.read_particles(&d, &q)
    };
    let baseline = run(&s)[0];
    let mut other = p(-0.25, 0.56, 0);
    other.radius = 0.004;
    other.velocity = [-0.1, 0.];
    s.particles.push(other);
    let actual = run(&s)[0];
    assert!(
        (actual.velocity[0] - baseline.velocity[0]).abs() < 1e-5,
        "asymmetric barrier bypass: {actual:?} vs {baseline:?}"
    );
    s.particles[1] = p(-0.24, 0.30, 0);
    s.particles[1].radius = 0.004;
    s.particles[1].velocity = [-0.1, 0.];
    let visible = run(&s)[0];
    for i in 0..40 {
        let mut hidden = p(-0.45 + i as f32 * 0.01, 0.56, 0);
        hidden.radius = 0.004;
        s.particles.push(hidden);
    }
    let crowded = run(&s)[0];
    assert!(
        (crowded.velocity[0] - visible.velocity[0]).abs() < 1e-5,
        "hidden group changed visible damping: {crowded:?} vs {visible:?}"
    );
}

#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_reseed_reuses_geometry_and_replaces_particle_state() {
    let (d, q) = device();
    let mut r = Recipe::balanced();
    r.vessel = particles::vessel::VesselKind::Cascade;
    r.budget = 1200;
    let mut s = Seed::from(&Simulation::seeded(&r).unwrap());
    let mut e = Engine::new(&d, &q, &s);
    r.seed += 1;
    s = Seed::from(&Simulation::seeded(&r).unwrap());
    assert!(e.reseed(&d, &q, &s));
    let out = e.read_particles(&d, &q);
    assert_eq!(out[0].position, s.particles[0].position);
    r.vessel = particles::vessel::VesselKind::RockGarden;
    s = Seed::from(&Simulation::seeded(&r).unwrap());
    assert!(!e.reseed(&d, &q, &s));
    e.advance(&d, &q, &[Step::default()]);
    let out = e.read_particles(&d, &q);
    assert!(out.iter().all(|p| s
        .vessel
        .clearance(glam::Vec2::from_array(p.position), e.angle)
        + 0.0005
        >= p.radius));
}

#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_fine_hourglass_neck_keeps_concave_corner_visibility() {
    let (d, q) = device();
    let mut r = Recipe::balanced();
    r.vessel = particles::vessel::VesselKind::Hourglass;
    r.budget = 1200;
    let mut s = Seed::from(&Simulation::seeded(&r).unwrap());
    s.base_radius = 0.0015;
    s.materials[0].viscosity = 8.;
    s.materials[0].thermal_motion = 0.;
    s.materials[0].surface_tension = 0.;
    s.particles = vec![p(0.05925, 0.0775, 0)];
    s.particles[0].radius = 0.0005;
    s.particles[0].velocity = [-0.1, 0.];
    let run = |s: &Seed| {
        let mut e = Engine::new(&d, &q, s);
        e.advance(&d, &q, &[Step::default()]);
        e.read_particles(&d, &q)[0]
    };
    let baseline = run(&s);
    let mut other = p(0.06165, 0.0825, 0);
    other.radius = 0.0005;
    other.velocity = [0.1, 0.];
    s.particles.push(other);
    let actual = run(&s);
    assert!(
        (actual.velocity[0] - baseline.velocity[0]).abs() < 1e-5,
        "fine corner leaked viscosity: {actual:?}/{baseline:?}"
    );
}

#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_compact_grid_fast_grains_do_not_pass_through_each_other() {
    let (d, q) = device();
    let mut s = seed();
    s.base_radius = 0.0005;
    s.particles = vec![p(-0.002, 0., 3), p(0.002, 0., 3)];
    for p in &mut s.particles {
        p.radius = 0.0005;
    }
    s.particles[0].velocity = [0.6, 0.];
    s.particles[1].velocity = [-0.6, 0.];
    let mut e = Engine::new(&d, &q, &s);
    e.advance(&d, &q, &[Step::default()]);
    let out = e.read_particles(&d, &q);
    assert!(
        out[0].position[0] < out[1].position[0],
        "compact-grid collision missed: {out:?}"
    );
}

#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_settled_100k_hourglass_half_turn_keeps_neighbour_cells_bounded() {
    check_settled_100k_hourglass_half_turn(6);
}

#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_adaptive_100k_hourglass_half_turn_keeps_neighbour_cells_bounded() {
    check_settled_100k_hourglass_half_turn(4);
}

fn check_settled_100k_hourglass_half_turn(iterations: usize) {
    use particles::{
        controls::{rotation_step, FIXED_DT},
        vessel::VesselKind,
    };
    let (device, queue) = device();
    let mut recipe = Recipe::balanced();
    recipe.vessel = VesselKind::Hourglass;
    recipe.budget = 100_000;
    let seed = Seed::from(&Simulation::seeded(&recipe).unwrap());
    let mut engine = Engine::new(&device, &queue, &seed);
    engine.advance(&device, &queue, &[Step::default(); 720]);

    let mut remaining = std::f32::consts::PI;
    let mut speed = 0.0;
    let mut steps = Vec::new();
    while remaining > 0.0 {
        (speed, remaining) = rotation_step(speed, 0.0, remaining, FIXED_DT);
        steps.push(Step {
            dt: FIXED_DT,
            angular_speed: speed,
            shake: [0.0; 2],
        });
    }

    let side = engine.side as usize;
    let cell_size = engine.grid_cell_size();
    let mut cells = vec![0u32; side * side];
    let mut peak = 0;
    for batch in steps.chunks(16) {
        engine.advance_with_iterations(&device, &queue, batch, iterations);
        cells.fill(0);
        for p in engine.read_particles(&device, &queue) {
            let cell = |coordinate: f32| {
                (((coordinate + 1.0) / cell_size).floor() as i32 + 1).clamp(0, side as i32 - 1)
                    as usize
            };
            let bin = &mut cells[cell(p.position[1]) * side + cell(p.position[0])];
            *bin += 1;
            peak = peak.max(*bin);
        }
    }
    // Dense cells make every surrounding search expensive. Leave room for
    // different GPU scatter orders while rejecting a corner collapse.
    assert!(
        peak < 256,
        "Half-turn packed {peak} particles into one cell"
    );
    assert!((engine.angle - std::f32::consts::PI).abs() < 1e-4);
    let particles = engine.read_particles(&device, &queue);
    assert_eq!(particles.len(), seed.particles.len());
    assert!(particles
        .iter()
        .zip(&seed.particles)
        .enumerate()
        .all(|(id, (p, original))| {
            p.id == id as u32
                && p.material == original.material
                && p.position
                    .iter()
                    .chain(&p.velocity)
                    .all(|value| value.is_finite())
                && seed
                    .vessel
                    .clearance(glam::Vec2::from_array(p.position), engine.angle)
                    + 0.0005
                    >= p.radius
        }));
    eprintln!("Peak cell population during Half-turn with {iterations} passes: {peak}");
}

#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_liquids_and_bubbles_settle_after_shaking() {
    use particles::material::Preset;
    let (device, queue) = device();
    let mut recipe = Preset::LiquidsAndBubbles.recipe();
    recipe.budget = 100_000;
    let seed = Seed::from(&Simulation::seeded(&recipe).unwrap());
    let mut engine = Engine::new(&device, &queue, &seed);
    let rms_by_material = |particles: &[GpuParticle]| {
        let mut sum = [0.0f64; 3];
        let mut count = [0; 3];
        for p in particles {
            let i = p.material as usize;
            sum[i] += f64::from(glam::Vec2::from_array(p.velocity).length_squared());
            count[i] += 1;
        }
        std::array::from_fn::<_, 3, _>(|i| (sum[i] / f64::from(count[i])).sqrt())
    };

    let shaking: Vec<_> = (0..120)
        .map(|i| {
            let t = i as f32 / 120.;
            Step {
                shake: [7. * (t * 37.).sin(), 4.5 * (t * 29.).cos()],
                ..Step::default()
            }
        })
        .collect();
    engine.advance(&device, &queue, &shaking);
    let driven = rms_by_material(&engine.read_particles(&device, &queue));
    assert!(
        driven[0] > 0.1,
        "water barely moved during shaking: {driven:?}"
    );

    engine.advance(&device, &queue, &[Step::default(); 1440]);
    let settled = engine.read_particles(&device, &queue);
    let speeds = rms_by_material(&settled);
    eprintln!("After shaking {driven:.4?}; after settling {speeds:.4?}");
    assert!(
        speeds[0] < 0.08 && speeds[1] < 0.08 && speeds[2] < 0.13,
        "mixture kept churning after 12 seconds of rest: {speeds:?}"
    );

    let mut height = [0.0f64; 3];
    let mut count = [0; 3];
    for p in &settled {
        let i = p.material as usize;
        height[i] += f64::from(p.position[1]);
        count[i] += 1;
    }
    let levels = std::array::from_fn::<_, 3, _>(|i| height[i] / f64::from(count[i]));
    assert!(
        levels[0] + 0.08 < levels[1] && levels[1] + 0.15 < levels[2],
        "liquids and air failed to stratify: {levels:?}"
    );
}

#[test]
#[ignore = "requires a compute-capable GPU"]
fn hourglass_neck_width_controls_gpu_neighbor_visibility() {
    use particles::vessel::VesselKind;
    let (device, queue) = device();
    let particle = |x, y, vx| GpuParticle {
        position: [x, y],
        velocity: [vx, 0.],
        radius: 0.0005,
        material: 0,
        id: 0,
        padding: 0,
    };
    let a = particle(0.02925, 0.0775, -0.05);
    let b = particle(0.03165, 0.0825, 0.05);
    let run = |seed: &Seed| {
        let mut engine = Engine::new(&device, &queue, seed);
        engine.advance(&device, &queue, &[Step::default()]);
        engine.read_particles(&device, &queue)[0].velocity[0]
    };

    let mut recipe = Recipe::balanced();
    recipe.vessel = VesselKind::Hourglass;
    let mut neighbor_effect = |width| {
        recipe.neck_width_percent = width;
        let mut seed = Seed::from(&Simulation::seeded(&recipe).unwrap());
        seed.base_radius = 0.0015;
        seed.materials[0].viscosity = 8.;
        seed.materials[0].thermal_motion = 0.;
        seed.materials[0].surface_tension = 0.;
        seed.particles = vec![a];
        let isolated = run(&seed);
        seed.particles.push(b);
        (run(&seed) - isolated).abs()
    };

    let narrow = neighbor_effect(50);
    let wide = neighbor_effect(250);
    assert!(narrow < 1e-5, "narrow wall leaked viscosity: {narrow}");
    assert!(wide > 1e-4, "wide opening blocked viscosity: {wide}");
}

#[test]
#[ignore = "requires a compute-capable GPU"]
fn gpu_five_percent_hourglass_passes_only_grains_that_fit() {
    use particles::vessel::VesselKind;
    let (device, queue) = device();
    let mut recipe = Recipe::balanced();
    recipe.vessel = VesselKind::Hourglass;
    recipe.neck_width_percent = 5;
    let mut seed = Seed::from(&Simulation::seeded(&recipe).unwrap());

    for angle in [0., std::f32::consts::PI] {
        seed.angle = angle;
        for (radius, passes) in [(0.0024, true), (0.005, false)] {
            seed.particles = vec![GpuParticle {
                radius,
                ..p(0., 0.24, 3)
            }];
            let mut engine = Engine::new(&device, &queue, &seed);
            engine.advance(&device, &queue, &[Step::default(); 360]);
            let out = engine.read_particles(&device, &queue)[0];
            let chamber = if passes { -1. } else { 1. };
            assert!(
                out.position[1] * chamber >= 0.08,
                "wrong chamber at angle {angle} for grain with radius {radius}: {out:?}"
            );
        }
    }
}
