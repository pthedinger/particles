use glam::Vec2;
use particles::material::{Phase, Recipe};
use particles::simulation::{Motion, Particle, Simulation};

#[test]
fn initialization_is_reproducible_spread_and_seeded() {
    let mut recipe = Recipe::balanced(); recipe.budget = 200;
    let a = Simulation::new(&recipe).unwrap();
    let b = Simulation::new(&recipe).unwrap();
    assert_eq!(a.particles, b.particles);
    assert!(a.particles.iter().any(|p| p.position.length() > 0.7));
    recipe.seed += 1;
    let c = Simulation::new(&recipe).unwrap();
    assert_ne!(a.particles, c.particles);
}

#[test]
fn driven_mixture_stays_contained_finite_and_conserves_materials() {
    let mut recipe = Recipe::balanced(); recipe.budget = 400;
    recipe.materials[3].radius = 0.5;
    recipe.materials[5].radius = 2.0;
    let mut sim = Simulation::new(&recipe).unwrap();
    let counts = recipe.counts().unwrap();
    for step in 0..300 {
        let t = step as f32 * 0.07;
        sim.step(1.0/120.0, Motion { angular_velocity: 2.0*t.sin(), shake: Vec2::new(8.0*t.cos(), 4.0*t.sin()) });
        for p in &sim.particles {
            assert!(p.position.is_finite() && p.velocity.is_finite());
            assert!(p.position.length() + p.radius <= 1.001, "particle escaped: {p:?}");
        }
    }
    let mut actual = vec![0; counts.len()];
    for p in &sim.particles { actual[p.material] += 1; }
    assert_eq!(actual, counts);
    assert!(sim.time > 2.4);
    assert!(sim.particles.iter().any(|p| p.velocity.length() > 0.01));
}

fn buoyancy_fixture(phase: Phase, grain_density: f32) -> Simulation {
    let mut recipe = Recipe::balanced(); recipe.budget = 40;
    let mut sim = Simulation::new(&recipe).unwrap();
    sim.materials[0].phase = phase;
    sim.materials[0].density = if phase == Phase::Gas { 0.04 } else { 1.0 };
    sim.materials[3].density = grain_density;
    sim.particles = vec![Particle { position: Vec2::ZERO, velocity: Vec2::ZERO, material: 3, radius: sim.base_radius }];
    for i in 0..8 {
        let angle = i as f32 * std::f32::consts::TAU / 8.0;
        sim.particles.push(Particle { position: Vec2::from_angle(angle) * sim.base_radius * 2.2, velocity: Vec2::ZERO, material: 0, radius: sim.base_radius });
    }
    sim
}

#[test]
fn density_contrast_gives_buoyant_and_sinking_solid_motion() {
    let mut light = buoyancy_fixture(Phase::Liquid, 0.4);
    let mut heavy = buoyancy_fixture(Phase::Liquid, 2.5);
    assert!(light.accelerations(Motion::default())[0].y > 0.0);
    assert!(heavy.accelerations(Motion::default())[0].y < 0.0);
    for _ in 0..5 { light.step(1.0/120.0, Motion::default()); heavy.step(1.0/120.0, Motion::default()); }
    assert!(light.particles[0].velocity.y > 0.0);
    assert!(heavy.particles[0].velocity.y < 0.0);
}

#[test]
fn gas_surroundings_do_not_apply_water_buoyancy() {
    let mut sim = buoyancy_fixture(Phase::Gas, 0.4);
    assert!(sim.accelerations(Motion::default())[0].y < 0.0);
    sim.step(1.0/120.0, Motion::default());
    assert!(sim.particles[0].velocity.y < 0.0);
}

#[test]
fn buoyancy_reaction_conserves_total_external_force() {
    let mut sim = buoyancy_fixture(Phase::Liquid, 0.4);
    let acc = sim.accelerations(Motion::default());
    let mut actual = Vec2::ZERO;
    let mut mass = 0.0;
    for (p,a) in sim.particles.iter().zip(acc) {
        let m = sim.materials[p.material].density * (p.radius / sim.base_radius).powi(2);
        actual += a*m; mass += m;
    }
    assert!((actual - Vec2::new(0.0, -1.8)*mass).length() < 0.001);
}

#[test]
fn coincident_contacts_separate_without_nonfinite_state() {
    let mut sim = buoyancy_fixture(Phase::Gas, 2.0);
    sim.particles.truncate(2);
    sim.particles[1] = sim.particles[0].clone();
    for _ in 0..12 { sim.step(1.0/120.0, Motion::default()); }
    assert!(sim.particles.iter().all(|p| p.position.is_finite()));
    assert!(sim.particles[0].position.distance(sim.particles[1].position) > sim.base_radius);
}

#[test]
fn granular_bed_settles_and_remains_still() {
    let mut recipe = Recipe::balanced(); recipe.budget = 100;
    for (i,m) in recipe.materials.iter_mut().enumerate() { m.share = if i == 3 { 1.0 } else { 0.0 }; }
    let mut sim = Simulation::new(&recipe).unwrap();
    sim.particles.truncate(40);
    for _ in 0..900 { sim.step(1.0/120.0, Motion::default()); }
    let speed: f32 = sim.particles.iter().map(|p| p.velocity.length()).sum::<f32>() / sim.particles.len() as f32;
    assert!(speed < 0.08, "bed did not settle: {speed}");
    let mean_y = sim.particles.iter().map(|p| p.position.y).sum::<f32>()/sim.particles.len() as f32;
    assert!(mean_y < -0.4, "bed has not sunk: {mean_y}");
}
