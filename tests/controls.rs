use glam::Vec2;
use particles::controls::{bounded_motion, view_to_world, Clock};

#[test]
fn clock_advances_at_wall_time_and_reports_capped_catchup() {
    let mut clock = Clock::default();
    assert_eq!(clock.advance(1.0/60.0,false),2);
    assert_eq!(clock.advance(1.0,false),4);
    assert!(clock.dropped_time > 0.96);
}

#[test]
fn pause_discards_backlog_and_nonfinite_time_cannot_poison_clock() {
    let mut clock = Clock::default();
    clock.advance(0.004,false);
    assert_eq!(clock.advance(0.5,true),0);
    assert_eq!(clock.advance(f32::NAN,false),0);
    assert_eq!(clock.advance(1.0/120.0,false),1);
}

#[test]
fn scaled_offset_vessel_maps_to_world_with_upward_y() {
    assert_eq!(view_to_world(Vec2::new(600.0,100.0), Vec2::new(600.0,300.0),200.0), Vec2::new(0.0,1.0));
    assert_eq!(view_to_world(Vec2::new(700.0,300.0), Vec2::new(600.0,300.0),200.0), Vec2::new(0.5,0.0));
}

#[test]
fn invalid_and_extreme_input_remains_bounded() {
    let motion = bounded_motion(f32::INFINITY,Vec2::new(f32::NAN,1.0));
    assert!(motion.angular_velocity.is_finite() && motion.shake.is_finite());
    let motion = bounded_motion(1e20,Vec2::new(1000.0,1000.0));
    assert!(motion.angular_velocity.abs() <= 3.0);
    assert!(motion.shake.length() <= 12.001);
}
