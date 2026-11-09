use crate::simulation::Motion;
use glam::Vec2;

pub const FIXED_DT: f32 = 1.0 / 120.0;
#[derive(Default)]
pub struct Clock {
    accumulator: f32,
    pub dropped_time: f32,
}
impl Clock {
    pub fn advance(&mut self, elapsed: f32, paused: bool) -> usize {
        if paused {
            self.accumulator = 0.0;
            return 0;
        }
        if !elapsed.is_finite() || elapsed <= 0.0 {
            return 0;
        }
        self.accumulator += elapsed;
        let available = (self.accumulator / FIXED_DT + 0.00001).floor();
        let steps = available.min(4.0) as usize;
        if available > 4.0 {
            self.dropped_time += (available - 4.0) * FIXED_DT;
        }
        self.accumulator = (self.accumulator - available * FIXED_DT).max(0.0);
        steps
    }
}

pub fn view_to_world(point: Vec2, center: Vec2, radius: f32) -> Vec2 {
    if !radius.is_finite() || radius <= 0.0 {
        return Vec2::ZERO;
    }
    (point - center) * Vec2::new(1.0, -1.0) / radius
}

pub fn bounded_motion(angular_velocity: f32, shake: Vec2) -> Motion {
    Motion {
        angular_velocity: if angular_velocity.is_finite() {
            angular_velocity.clamp(-3.0, 3.0)
        } else {
            0.0
        },
        shake: if shake.is_finite() {
            shake.clamp_length_max(12.0)
        } else {
            Vec2::ZERO
        },
    }
}
