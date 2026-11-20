use glam::Vec2;
use particles::controls::{bounded_motion, view_to_world, Clock, FIXED_DT};

#[test]
fn clock_advances_at_wall_time_and_reports_capped_catchup() {
    let mut clock = Clock::default();
    assert_eq!(clock.advance(1.0 / 60.0, false), 2);
    assert_eq!(clock.advance(1.0, false), 4);
    assert!(clock.dropped_time > 0.96);
}

#[test]
fn pause_discards_backlog_and_nonfinite_time_cannot_poison_clock() {
    let mut clock = Clock::default();
    clock.advance(0.004, false);
    assert_eq!(clock.advance(0.5, true), 0);
    assert_eq!(clock.advance(f32::NAN, false), 0);
    assert_eq!(clock.advance(1.0 / 120.0, false), 1);
}

#[test]
fn scaled_offset_vessel_maps_to_world_with_upward_y() {
    assert_eq!(
        view_to_world(Vec2::new(600.0, 100.0), Vec2::new(600.0, 300.0), 200.0),
        Vec2::new(0.0, 1.0)
    );
    assert_eq!(
        view_to_world(Vec2::new(700.0, 300.0), Vec2::new(600.0, 300.0), 200.0),
        Vec2::new(0.5, 0.0)
    );
}

#[test]
fn invalid_and_extreme_input_remains_bounded() {
    let motion = bounded_motion(f32::INFINITY, Vec2::new(f32::NAN, 1.0));
    assert!(motion.angular_velocity.is_finite() && motion.shake.is_finite());
    let motion = bounded_motion(1e20, Vec2::new(1000.0, 1000.0));
    assert!(motion.angular_velocity.abs() <= 3.0);
    assert!(motion.shake.length() <= 12.001);
}

#[test]
fn half_turn_brakes_at_exact_target_even_when_initially_moving() {
    use particles::controls::rotation_step;
    for initial in [0.0, 1.6, -1.6] {
        let mut speed = initial;
        let mut remaining = std::f32::consts::PI;
        let mut angle = 0.0;
        for _ in 0..720 {
            let desired = 0.0;
            let previous = speed;
            (speed, remaining) = rotation_step(speed, desired, remaining, FIXED_DT);
            assert!((speed-previous).abs()<=8.0*FIXED_DT+0.0001,"initial {initial}, previous {previous}, speed {speed}, remaining {remaining}, angle {angle}");
            angle += speed * FIXED_DT;
        }
        assert!(speed.abs() < 0.001);
        assert!(
            (angle - std::f32::consts::PI).abs() < 0.002,
            "half-turn from {initial} ended at {} degrees",
            angle.to_degrees()
        );
    }
}

#[test]
fn adaptive_budget_prioritizes_turns_then_recovers_after_sustained_headroom() {
    use particles::controls::FrameBudget;

    let mut budget = FrameBudget::default();
    assert_eq!(budget.update(1.0 / 60.0, false, false).iterations, 6);
    let turn = budget.update(1.0 / 60.0, true, false);
    assert_eq!(turn.iterations, 4);
    assert_eq!(turn.max_steps, 2);
    budget.update(0.034, true, true);
    let missed = budget.update(0.034, true, true);
    assert_eq!(missed.iterations, 4);
    assert_eq!(missed.max_steps, 0);
    let next = budget.update(1.0 / 60.0, true, false);
    assert_eq!(next.max_steps, 1);
    for _ in 0..30 {
        assert_eq!(budget.update(1.0 / 60.0, false, false).max_steps, 1);
    }
    for _ in 0..650 {
        budget.update(1.0 / 60.0, false, false);
    }
    assert_eq!(budget.update(1.0 / 60.0, false, false).iterations, 6);
    for _ in 0..120 {
        budget.update(0.020, false, false);
    }
    assert_eq!(budget.update(0.020, false, false).max_steps, 1);
}

#[test]
fn a_busy_adaptive_frame_cannot_build_up_catch_up_work() {
    let mut clock = Clock::default();
    assert_eq!(clock.advance_with_budget(0.5, false, 0), 0);
    assert!((clock.dropped_time - 0.5).abs() < 1e-5);
    assert_eq!(clock.advance_with_budget(FIXED_DT, false, 1), 1);
    assert_eq!(clock.advance_with_budget(0.0, false, 1), 0);
}
