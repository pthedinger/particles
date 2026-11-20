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
        self.advance_steps(elapsed, paused, 4, 0.0)
    }

    pub fn advance_with_budget(&mut self, elapsed: f32, paused: bool, max_steps: usize) -> usize {
        // Carry up to one step through small variations in presentation timing.
        self.advance_steps(
            elapsed,
            paused,
            max_steps,
            if max_steps > 0 { 1.0 } else { 0.0 },
        )
    }

    fn advance_steps(
        &mut self,
        elapsed: f32,
        paused: bool,
        max_steps: usize,
        pending_steps: f32,
    ) -> usize {
        if paused {
            self.accumulator = 0.0;
            return 0;
        }
        if !elapsed.is_finite() || elapsed <= 0.0 {
            return 0;
        }
        self.accumulator += elapsed;
        let available = (self.accumulator / FIXED_DT + 0.00001).floor();
        let steps = available.min(max_steps as f32) as usize;
        let dropped = (available - steps as f32 - pending_steps).max(0.0);
        self.dropped_time += dropped * FIXED_DT;
        self.accumulator = (self.accumulator - (steps as f32 + dropped) * FIXED_DT).max(0.0);
        steps
    }
}

/// Adaptive GPU physics budget. Leave headroom for rendering and input.
#[derive(Default)]
pub struct FrameBudget {
    tier: usize,
    mean_elapsed: f32,
    timely_time: f32,
    missed_frames: u8,
}

#[derive(Clone, Copy)]
pub struct PhysicsBudget {
    pub iterations: usize,
    pub max_steps: usize,
}

impl PhysicsBudget {
    pub const FULL: Self = Self {
        iterations: 6,
        max_steps: 4,
    };
}

impl FrameBudget {
    pub fn update(&mut self, elapsed: f32, heavy_motion: bool, busy: bool) -> PhysicsBudget {
        // At high counts, trim work before a turn can miss its first deadline.
        const TURN_TIER: usize = 2;
        const LOWEST_TIER: usize = 3;
        let minimum_tier = if heavy_motion { TURN_TIER } else { 0 };
        self.tier = self.tier.max(minimum_tier);
        let valid_sample = elapsed.is_finite() && elapsed > 0.0;
        if valid_sample {
            self.mean_elapsed += (elapsed - self.mean_elapsed) * 0.08;
        }
        // Smooth small variations around 16.7 ms, while reacting quickly
        // to larger misses or a queue that remains backed up.
        if valid_sample
            && (elapsed > 0.021 || self.mean_elapsed > 0.018 || (busy && elapsed > 0.012))
        {
            self.missed_frames += 1;
            self.timely_time = 0.0;
            if self.missed_frames >= 2 {
                self.tier = (self.tier + 1).min(LOWEST_TIER);
                self.missed_frames = 0;
            }
        } else if valid_sample {
            self.missed_frames = self.missed_frames.saturating_sub(1);
            if self.tier > minimum_tier && self.mean_elapsed <= 0.0175 {
                self.timely_time += elapsed;
                if self.timely_time >= 1.5 {
                    self.tier -= 1;
                    self.timely_time = 0.0;
                }
            }
        }
        // Keep the collision time step small. The lowest tier preserves
        // accuracy per step and advances less time per displayed frame.
        let (iterations, max_steps) = if self.tier < LOWEST_TIER {
            (6 - self.tier, 2)
        } else {
            (4, 1)
        };
        PhysicsBudget {
            iterations,
            max_steps: if busy { 0 } else { max_steps },
        }
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

/// Advances vessel speed and remaining requested half-turn angle.
pub fn rotation_step(speed: f32, desired: f32, remaining: f32, dt: f32) -> (f32, f32) {
    let acceleration = 8.0;
    let target = if remaining > 0.0 {
        // Reserve half the acceleration budget for discrete end-step rounding.
        // This begins braking early enough to stop without an impulsive snap.
        let braking_speed = (acceleration * remaining).sqrt();
        braking_speed.min(1.8)
    } else {
        desired
    };
    let mut next = speed + (target - speed).clamp(-acceleration * dt, acceleration * dt);
    if remaining > 0.0 {
        next = next.min(remaining / dt);
        (next, (remaining - next * dt).max(0.0))
    } else {
        (next, 0.0)
    }
}
