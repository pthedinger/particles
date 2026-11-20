use glam::Vec2;
use particles::{
    material::Recipe,
    simulation::{Motion, Simulation},
    vessel::{Vessel, VesselKind},
};

#[test]
fn geometry_signs_obstacles_rotation_and_projection() {
    let square = Vessel::new(VesselKind::Square, 1);
    assert!(square.clearance(Vec2::ZERO, 0.) > 0.5);
    assert!(square.clearance(Vec2::new(0.8, 0.), 0.) < 0.);
    assert!(square.clearance(Vec2::new(0.8, 0.), std::f32::consts::FRAC_PI_4) > 0.);
    let shelves = Vessel::new(VesselKind::Cascade, 1);
    assert!(shelves.clearance(Vec2::new(-0.2, 0.38), 0.) < 0.);
    assert!(shelves.clearance(Vec2::new(0.5, 0.38), 0.) > 0.05);
    let timer = Vessel::new(VesselKind::Hourglass, 1);
    for i in -7..=7 {
        assert!(timer.clearance(Vec2::new(0., i as f32 * 0.1), 0.) > 0.02);
    }
    for kind in VesselKind::ALL {
        let v = Vessel::new(kind, 42);
        for angle in [0., 0.6, 2.4] {
            for start in [
                Vec2::new(0.92, 0.2),
                Vec2::new(-0.2, 0.38),
                Vec2::new(0., -0.15),
            ] {
                let out = v.project(start, 0.008, angle);
                assert!(
                    v.clearance(out, angle) >= 0.0075,
                    "{kind:?} failed projection: {out}"
                );
            }
        }
    }
}
#[test]
fn layouts_are_repeatable_and_reroll_changes_only_garden() {
    let a = Vessel::new(VesselKind::RockGarden, 12);
    let b = Vessel::new(VesselKind::RockGarden, 12);
    let c = Vessel::new(VesselKind::RockGarden, 13);
    assert_eq!(a.map, b.map);
    assert_ne!(a.map, c.map);
    assert_eq!(
        Vessel::new(VesselKind::Cascade, 12).map,
        Vessel::new(VesselKind::Cascade, 13).map
    );
}
#[test]
fn initialization_fits_each_vessel_and_preserves_counts_and_seed() {
    for kind in VesselKind::ALL {
        let mut r = Recipe::balanced();
        r.budget = 1200;
        r.vessel = kind;
        let s = Simulation::seeded(&r).unwrap();
        assert_eq!(s.particles.len(), r.budget);
        assert_eq!(s.particles, Simulation::seeded(&r).unwrap().particles);
        let mut counts = vec![0; r.materials.len()];
        for p in &s.particles {
            assert!(
                s.vessel.clearance(p.position, 0.) + 1e-5 >= p.radius,
                "{kind:?}: {:?}",
                p
            );
            if kind == VesselKind::Hourglass {
                assert!(p.position.y > 0.08);
            }
            counts[p.material] += 1;
        }
        assert_eq!(counts, r.counts().unwrap());
    }
}
#[test]
fn cpu_grains_land_on_a_shelf_and_do_not_cross_it() {
    let mut r = Recipe::balanced();
    r.vessel = VesselKind::Cascade;
    r.budget = 800;
    let mut s = Simulation::new(&r).unwrap();
    s.particles.truncate(1);
    let p = &mut s.particles[0];
    p.material = 3;
    p.position = Vec2::new(-0.25, 0.65);
    p.radius = 0.012;
    p.velocity = Vec2::new(0., -2.);
    for _ in 0..300 {
        s.step(1. / 120., Motion::default());
    }
    assert!(
        s.particles[0].position.y > 0.43,
        "grain fell through shelf: {:?}",
        s.particles[0]
    );
    assert!(s.particles[0].velocity.length() < 0.03);
}

#[test]
fn compact_100k_seed_does_not_repeat_float_quantized_positions() {
    let mut r = Recipe::balanced();
    r.budget = 100_000;
    r.vessel = VesselKind::Hourglass;
    let s = Simulation::seeded(&r).unwrap();
    let unique: std::collections::HashSet<_> = s
        .particles
        .iter()
        .map(|p| (p.position.x.to_bits(), p.position.y.to_bits()))
        .collect();
    assert_eq!(
        unique.len(),
        r.budget,
        "seed repeated positions at high resolution"
    );
}

#[test]
fn unchanged_geometry_is_reused_across_particle_reseeds() {
    let mut r = Recipe::balanced();
    r.vessel = VesselKind::RockGarden;
    r.budget = 1200;
    let first = Simulation::seeded(&r).unwrap();
    r.seed += 1;
    let second = Simulation::seeded(&r).unwrap();
    assert!(
        std::sync::Arc::ptr_eq(&first.vessel, &second.vessel),
        "unchanged geometry rebuilt on particle remix"
    );
}
#[test]
fn cpu_rotating_shelf_does_not_sweep_a_grain_to_its_underside() {
    let mut r = Recipe::balanced();
    r.vessel = VesselKind::Cascade;
    r.budget = 1200;
    let mut s = Simulation::seeded(&r).unwrap();
    s.particles.truncate(1);
    s.base_radius = 0.04;
    let p = &mut s.particles[0];
    p.position = Vec2::new(-0.58, 0.429);
    p.radius = 0.003;
    p.material = 3;
    p.velocity = Vec2::new(0., -2.5);
    s.step(
        1. / 60.,
        Motion {
            angular_velocity: -3.,
            shake: Vec2::ZERO,
        },
    );
    let local = Vec2::from_angle(-s.angle).rotate(s.particles[0].position);
    assert!(
        local.y > 0.425,
        "rotating shelf put grain on wrong side: {local}"
    );
}

#[test]
fn asymmetric_segment_cannot_cut_through_a_shelf() {
    let v = Vessel::new(VesselKind::Cascade, 1);
    assert!(!v.segment_clear(Vec2::new(-0.25, 0.30), Vec2::new(-0.25, 0.56), 0.));
    assert!(v.segment_clear(Vec2::new(-0.25, 0.30), Vec2::new(-0.15, 0.30), 0.));
}

#[test]
fn adjustable_hourglass_neck_drives_clearance_visibility_and_cache() {
    let narrow = Vessel::with_neck_width(VesselKind::Hourglass, 17, 50);
    let normal = Vessel::with_neck_width(VesselKind::Hourglass, 17, 100);
    let wide = Vessel::with_neck_width(VesselKind::Hourglass, 17, 250);
    let legacy = Vessel::new(VesselKind::Hourglass, 1);
    assert!(std::sync::Arc::ptr_eq(&normal, &legacy));
    assert_eq!(normal.barriers[3][..4], [0.06, 0.08, 0.06, -0.08]);
    assert!(std::sync::Arc::ptr_eq(
        &wide,
        &Vessel::with_neck_width(VesselKind::Hourglass, 99, 250)
    ));
    assert!(std::sync::Arc::ptr_eq(
        &Vessel::new(VesselKind::RockGarden, 17),
        &Vessel::with_neck_width(VesselKind::RockGarden, 17, 250)
    ));

    let angle = 0.4;
    let rotate = |p| Vec2::from_angle(angle).rotate(p);
    let a = rotate(Vec2::new(0.10, 0.20));
    let b = rotate(Vec2::new(0.10, -0.20));
    for (vessel, half_width, clear) in [
        (&narrow, 0.03, false),
        (&normal, 0.06, false),
        (&wide, 0.15, true),
    ] {
        assert!((vessel.clearance(Vec2::ZERO, angle) - half_width).abs() < 0.003);
        assert!(vessel.clearance(a, angle) > 0.);
        assert!(vessel.clearance(b, angle) > 0.);
        assert_eq!(vessel.segment_clear(a, b, angle), clear);
        let p = rotate(Vec2::new(0.10, 0.));
        let projected = vessel.project(p, 0.01, angle);
        assert_eq!(projected == p, clear);
        assert!(vessel.clearance(projected, angle) >= 0.01);
    }
}

#[test]
fn five_percent_hourglass_resolves_the_centerline_clearance() {
    let vessel = Vessel::with_neck_width(VesselKind::Hourglass, 17, 5);
    let angle = 0.4;
    let point = |x, y| Vec2::from_angle(angle).rotate(Vec2::new(x, y));

    assert!((vessel.clearance(Vec2::ZERO, angle) - 0.003).abs() < 0.0001);
    assert!(vessel.clearance(point(0.002, 0.), angle) > 0.);
    assert!(vessel.clearance(point(0.004, 0.), angle) < 0.);
    assert!(vessel.segment_clear(point(0.002, 0.1), point(0.002, -0.1), angle));
    assert!(!vessel.segment_clear(point(0.01, 0.1), point(0.01, -0.1), angle));
}
