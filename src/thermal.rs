//! Bounded zero-mean, unit-variance increments for an approximate thermal bath.
use glam::Vec2;
pub fn noise(id: u32, tick: u32) -> Vec2 {
    let mut x = id ^ tick.wrapping_mul(747796405) ^ 2891336453;
    x ^= x >> 16;
    x = x.wrapping_mul(2246822519);
    x ^= x >> 13;
    x = x.wrapping_mul(3266489917);
    x ^= x >> 16;
    Vec2::new(
        (x & 65535) as f32 / 32767.5 - 1.,
        (x >> 16) as f32 / 32767.5 - 1.,
    ) * 1.7320508
}
