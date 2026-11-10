//! GPU-owned particle state. CPU readback is reserved for explicit diagnostics.
mod engine;
mod render;
use crate::{
    material::{Material, Phase},
    simulation::{Particle, Simulation},
};
use bytemuck::{Pod, Zeroable};
pub use engine::Engine;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuParticle {
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub radius: f32,
    pub material: u32,
    /// Stable identity survives spatial reordering on the GPU.
    pub id: u32,
    pub padding: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GpuMaterial {
    pub color: [f32; 4],
    pub density: f32,
    pub viscosity: f32,
    pub friction: f32,
    pub phase: u32,
    pub surface_tension: f32,
    pub compression_resistance: f32,
    pub mixing_resistance: f32,
    pub neighbor_target: f32,
    pub thermal_motion: f32,
    pub padding: [f32; 3],
}
impl From<&Material> for GpuMaterial {
    fn from(m: &Material) -> Self {
        Self {
            color: [
                m.color[0] as f32 / 255.,
                m.color[1] as f32 / 255.,
                m.color[2] as f32 / 255.,
                1.,
            ],
            density: m.density,
            viscosity: m.viscosity,
            friction: m.friction,
            phase: match m.phase {
                Phase::Liquid => 0,
                Phase::Gas => 1,
                Phase::Solid => 2,
            },
            surface_tension: m.surface_tension,
            compression_resistance: m.compression_resistance,
            mixing_resistance: m.mixing_resistance,
            neighbor_target: m.neighbor_target(),
            thermal_motion: m.thermal_motion,
            padding: [0.; 3],
        }
    }
}
impl From<&Particle> for GpuParticle {
    fn from(p: &Particle) -> Self {
        Self {
            position: p.position.to_array(),
            velocity: p.velocity.to_array(),
            radius: p.radius,
            material: p.material as u32,
            id: 0,
            padding: 0,
        }
    }
}
#[derive(Clone)]
pub struct Seed {
    pub particles: Vec<GpuParticle>,
    pub materials: Vec<GpuMaterial>,
    pub base_radius: f32,
}
impl From<&Simulation> for Seed {
    fn from(sim: &Simulation) -> Self {
        Self {
            particles: sim.particles.iter().map(Into::into).collect(),
            materials: sim.materials.iter().map(Into::into).collect(),
            base_radius: sim.base_radius,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Step {
    pub dt: f32,
    pub angular_speed: f32,
    pub shake: [f32; 2],
}
impl Default for Step {
    fn default() -> Self {
        Self {
            dt: 1. / 120.,
            angular_speed: 0.,
            shake: [0.; 2],
        }
    }
}
