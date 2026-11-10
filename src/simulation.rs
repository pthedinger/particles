use crate::material::{Material, Phase, Recipe};
use glam::Vec2;
use rand::{rngs::StdRng, seq::SliceRandom, Rng, SeedableRng};

pub const GRAVITY: Vec2 = Vec2::new(0.0, -1.8);
const ITERATIONS: usize = 2;

#[derive(Clone, Debug, PartialEq)]
pub struct Particle {
    pub position: Vec2,
    pub velocity: Vec2,
    pub material: usize,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Motion {
    pub angular_velocity: f32,
    pub shake: Vec2,
}

struct GrainContact {
    i: usize,
    j: usize,
    normal: Vec2,
    relative_correction: f32,
}

/// CPU data stays separate from rendering/ECS; neighbor storage is reused each step.
pub struct Simulation {
    pub particles: Vec<Particle>,
    pub materials: Vec<Material>,
    pub base_radius: f32,
    pub angle: f32,
    pub time: f32,
    previous: Vec<Vec2>,
    inverse_mass: Vec<f32>,
    pressure_inverse_mass: Vec<f32>,
    volume: Vec<f32>,
    density: Vec<f32>,
    gradient: Vec<Vec2>,
    denominator: Vec<f32>,
    lambda: Vec<f32>,
    delta: Vec<Vec2>,
    acceleration: Vec<Vec2>,
    environment: Vec<Vec2>,
    buoyancy: Vec<Vec2>,
    heads: Vec<i32>,
    next: Vec<i32>,
    pairs: Vec<(usize, usize)>,
    contacts: Vec<GrainContact>,
    support: f32,
    side: usize,
}

impl Simulation {
    pub fn new(recipe: &Recipe) -> Result<Self, String> {
        let mut sim = Self::seeded(recipe)?;
        for _ in 0..24 {
            sim.constraints(false);
        }
        Ok(sim)
    }

    /// Reproducible seed without running the CPU constraint solver.
    pub fn seeded(recipe: &Recipe) -> Result<Self, String> {
        let counts = recipe.counts()?;
        let area: f32 = counts
            .iter()
            .zip(&recipe.materials)
            .map(|(n, m)| *n as f32 * m.radius * m.radius)
            .sum();
        let base_radius = (0.72 / area).sqrt();
        if recipe
            .materials
            .iter()
            .any(|m| m.radius * base_radius > 0.3)
        {
            return Err(
                "Increase the resolution: these grains are too large for the vessel.".into(),
            );
        }
        let mut rng = StdRng::seed_from_u64(recipe.seed);
        let mut identities = Vec::with_capacity(recipe.budget);
        for (material, count) in counts.iter().enumerate() {
            identities.extend(std::iter::repeat(material).take(*count));
        }
        identities.shuffle(&mut rng);
        let offset = rng.gen_range(0.0..std::f32::consts::TAU);
        let particles = identities
            .into_iter()
            .enumerate()
            .map(|(i, material)| {
                let a = offset + i as f32 * 2.399_963_1;
                let r = 0.955 * ((i as f32 + 0.5) / recipe.budget as f32).sqrt();
                Particle {
                    position: Vec2::from_angle(a) * r,
                    velocity: Vec2::ZERO,
                    material,
                    radius: base_radius * recipe.materials[material].radius,
                }
            })
            .collect();
        let mut sim = Self {
            particles,
            materials: recipe.materials.clone(),
            base_radius,
            angle: 0.0,
            time: 0.0,
            previous: Vec::new(),
            inverse_mass: Vec::new(),
            pressure_inverse_mass: Vec::new(),
            volume: Vec::new(),
            density: Vec::new(),
            gradient: Vec::new(),
            denominator: Vec::new(),
            lambda: Vec::new(),
            delta: Vec::new(),
            acceleration: Vec::new(),
            environment: Vec::new(),
            buoyancy: Vec::new(),
            heads: Vec::new(),
            next: Vec::new(),
            pairs: Vec::new(),
            contacts: Vec::new(),
            support: base_radius * 4.4,
            side: 0,
        };
        sim.prepare();
        Ok(sim)
    }

    fn prepare(&mut self) {
        let n = self.particles.len();
        self.previous.resize(n, Vec2::ZERO);
        self.inverse_mass.resize(n, 1.0);
        self.pressure_inverse_mass.resize(n, 0.0);
        self.volume.resize(n, 1.0);
        self.density.resize(n, 0.0);
        self.gradient.resize(n, Vec2::ZERO);
        self.denominator.resize(n, 0.0);
        self.lambda.resize(n, 0.0);
        self.delta.resize(n, Vec2::ZERO);
        self.acceleration.resize(n, Vec2::ZERO);
        self.environment.resize(n, Vec2::ZERO);
        self.buoyancy.resize(n, Vec2::ZERO);
        self.next.resize(n, -1);
        let max_radius = self
            .particles
            .iter()
            .map(|p| p.radius)
            .fold(self.base_radius, f32::max);
        self.support = (self.base_radius * 4.4).max(max_radius * 2.05);
        self.side = (2.0 / self.support).ceil() as usize + 3;
        self.heads.resize(self.side * self.side, -1);
        for (i, p) in self.particles.iter().enumerate() {
            self.volume[i] = (p.radius / self.base_radius).powi(2);
            self.inverse_mass[i] = 1.0 / (self.materials[p.material].density * self.volume[i]);
            // Grains are immersed boundaries for the fluid pressure projection,
            // not movable nodes of its coarse incompressibility lattice.
            self.pressure_inverse_mass[i] = if self.materials[p.material].phase == Phase::Solid {
                0.0
            } else {
                self.inverse_mass[i]
            };
        }
    }

    fn bin(&self, p: Vec2) -> (usize, usize) {
        let x = ((p.x + 1.0) / self.support).floor() as isize + 1;
        let y = ((p.y + 1.0) / self.support).floor() as isize + 1;
        (
            x.clamp(0, self.side as isize - 1) as usize,
            y.clamp(0, self.side as isize - 1) as usize,
        )
    }

    fn neighbors(&mut self) {
        self.heads.fill(-1);
        self.pairs.clear();
        for i in 0..self.particles.len() {
            let (x, y) = self.bin(self.particles[i].position);
            let b = y * self.side + x;
            self.next[i] = self.heads[b];
            self.heads[b] = i as i32;
        }
        let h2 = self.support * self.support;
        for i in 0..self.particles.len() {
            let (x, y) = self.bin(self.particles[i].position);
            for cy in y.saturating_sub(1)..=(y + 1).min(self.side - 1) {
                for cx in x.saturating_sub(1)..=(x + 1).min(self.side - 1) {
                    let mut j = self.heads[cy * self.side + cx];
                    while j >= 0 {
                        let k = j as usize;
                        if k > i
                            && self.particles[i]
                                .position
                                .distance_squared(self.particles[k].position)
                                < h2
                        {
                            self.pairs.push((i, k));
                        }
                        j = self.next[k];
                    }
                }
            }
        }
    }

    fn compute_accelerations(&mut self, motion: Motion) {
        self.environment.fill(Vec2::ZERO);
        self.buoyancy.fill(Vec2::ZERO);
        self.acceleration.fill(GRAVITY - motion.shake);
        for &(i, j) in &self.pairs {
            let q = 1.0
                - self.particles[i]
                    .position
                    .distance(self.particles[j].position)
                    / self.support;
            let w = q * q;
            let a = &self.materials[self.particles[i].material];
            let b = &self.materials[self.particles[j].material];
            if b.phase == Phase::Liquid
                && (a.phase != Phase::Liquid
                    || self.particles[i].material != self.particles[j].material)
            {
                self.environment[i] += Vec2::new(b.density * w, w);
            }
            if a.phase == Phase::Liquid
                && (b.phase != Phase::Liquid
                    || self.particles[i].material != self.particles[j].material)
            {
                self.environment[j] += Vec2::new(a.density * w, w);
            }
        }
        for i in 0..self.particles.len() {
            let env = self.environment[i];
            if env.y > 0.0001 {
                let rho = self.materials[self.particles[i].material].density;
                let submerged = (env.y / 0.8).min(1.0);
                let density_ratio = (env.x / env.y / rho).min(3.0);
                self.buoyancy[i] =
                    if self.materials[self.particles[i].material].phase == Phase::Solid {
                        // The grain no longer receives a second hydrostatic lift
                        // from fluid positional projection: apply Archimedes here.
                        -GRAVITY * density_ratio * submerged
                    } else {
                        // Fluid phases remain part of the shared pressure solve.
                        GRAVITY * (1.0 - density_ratio).clamp(-2.0, 1.0) * submerged
                    };
                self.acceleration[i] += self.buoyancy[i];
            }
        }
        for &(i, j) in &self.pairs {
            let q = 1.0
                - self.particles[i]
                    .position
                    .distance(self.particles[j].position)
                    / self.support;
            let w = q * q;
            let a = &self.materials[self.particles[i].material];
            let b = &self.materials[self.particles[j].material];
            if self.environment[i].y > 0.0001
                && b.phase == Phase::Liquid
                && (a.phase != Phase::Liquid
                    || self.particles[i].material != self.particles[j].material)
            {
                self.acceleration[j] -= self.buoyancy[i]
                    * (w / self.environment[i].y)
                    * (self.inverse_mass[j] / self.inverse_mass[i]);
            }
            if self.environment[j].y > 0.0001
                && a.phase == Phase::Liquid
                && (b.phase != Phase::Liquid
                    || self.particles[i].material != self.particles[j].material)
            {
                self.acceleration[i] -= self.buoyancy[j]
                    * (w / self.environment[j].y)
                    * (self.inverse_mass[i] / self.inverse_mass[j]);
            }
        }
    }

    /// The same force pass used by step; useful for inspecting material response.
    pub fn accelerations(&mut self, motion: Motion) -> Vec<Vec2> {
        self.prepare();
        self.neighbors();
        self.compute_accelerations(motion);
        self.acceleration.clone()
    }

    fn direction(&self, i: usize, j: usize) -> (Vec2, f32) {
        let d = self.particles[i].position - self.particles[j].position;
        let r2 = d.length_squared();
        if r2 < 1e-12 {
            (
                Vec2::from_angle(((i * 13 + j * 7) % 101) as f32 * 0.0622),
                0.000001,
            )
        } else {
            let r = r2.sqrt();
            (d / r, r)
        }
    }

    fn constraints(&mut self, record_contacts: bool) {
        self.neighbors();
        self.density.copy_from_slice(&self.volume);
        self.gradient.fill(Vec2::ZERO);
        self.denominator.fill(0.0);
        self.delta.fill(Vec2::ZERO);
        let rest = 2.65;
        for &(i, j) in &self.pairs {
            let (direction, r) = self.direction(i, j);
            let q = (1.0 - r / self.support).max(0.0);
            self.density[i] += self.volume[j] * q * q;
            self.density[j] += self.volume[i] * q * q;
            let grad = direction * (-2.0 * q / self.support / rest);
            let gi = grad * self.volume[j];
            let gj = -grad * self.volume[i];
            self.gradient[i] += gi;
            self.gradient[j] += gj;
            self.denominator[i] += self.pressure_inverse_mass[j] * gi.length_squared();
            self.denominator[j] += self.pressure_inverse_mass[i] * gj.length_squared();
        }
        for i in 0..self.particles.len() {
            let phase = self.materials[self.particles[i].material].phase;
            // Pressure resists compression; negative pressure would pull grains
            // together through sparse fluid. Phase cohesion is handled below.
            let c = (self.density[i] / rest - 1.0).max(0.0);
            let stiffness = match phase {
                Phase::Liquid => 1.0,
                Phase::Gas => 0.25,
                Phase::Solid => 0.0,
            };
            self.lambda[i] = -stiffness * c
                / (self.denominator[i]
                    + self.pressure_inverse_mass[i] * self.gradient[i].length_squared()
                    + 80.0);
        }
        for &(i, j) in &self.pairs {
            let (direction, r) = self.direction(i, j);
            let q = (1.0 - r / self.support).max(0.0);
            let grad = direction * (-2.0 * q / self.support / rest);
            let f = grad * (self.lambda[i] * self.volume[j] + self.lambda[j] * self.volume[i]);
            self.delta[i] += f * self.pressure_inverse_mass[i];
            self.delta[j] -= f * self.pressure_inverse_mass[j];
            let a = &self.materials[self.particles[i].material];
            let b = &self.materials[self.particles[j].material];
            if self.particles[i].material == self.particles[j].material
                && a.phase != Phase::Solid
                && r > self.particles[i].radius + self.particles[j].radius
            {
                let tension = if a.phase == Phase::Gas {
                    0.0007
                } else {
                    0.00012
                };
                let f = direction * (q * q * self.base_radius * tension * 120.0);
                let wi = self.inverse_mass[i] / (self.inverse_mass[i] + self.inverse_mass[j]);
                self.delta[i] -= f * wi;
                self.delta[j] += f * (1.0 - wi);
            } else if a.phase == Phase::Liquid
                && b.phase == Phase::Liquid
                && self.particles[i].material != self.particles[j].material
            {
                let f = direction * (q * q * self.base_radius * 0.007);
                self.delta[i] += f;
                self.delta[j] -= f;
            }
        }
        for i in 0..self.particles.len() {
            let correction = self.delta[i].clamp_length_max(self.particles[i].radius * 0.2);
            self.particles[i].position += correction;
        }
        for k in 0..self.pairs.len() {
            let (i, j) = self.pairs[k];
            let a = self.materials[self.particles[i].material].phase;
            let b = self.materials[self.particles[j].material].phase;
            let both_grains = a == Phase::Solid && b == Phase::Solid;
            let (contact_distance, wi) = if both_grains {
                (
                    self.particles[i].radius + self.particles[j].radius,
                    self.inverse_mass[i] / (self.inverse_mass[i] + self.inverse_mass[j]),
                )
            } else if a == Phase::Solid {
                // A fluid parcel is a quadrature point, not a rigid bead with
                // its own collision radius. Move it out of the grain only.
                (self.particles[i].radius, 0.0)
            } else if b == Phase::Solid {
                (self.particles[j].radius, 1.0)
            } else {
                continue;
            };
            let (dir, r) = self.direction(i, j);
            let overlap = contact_distance - r;
            if overlap > 0.0 {
                let ci = dir * (overlap * wi * 0.95).min(self.particles[i].radius * 0.4);
                let cj = dir * (overlap * (1.0 - wi) * 0.95).min(self.particles[j].radius * 0.4);
                self.particles[i].position += ci;
                self.particles[j].position -= cj;
                if record_contacts && a == Phase::Solid && b == Phase::Solid {
                    // The position solve supplies the actual compressive load.
                    // Keep it for Coulomb friction after reconstructing velocities.
                    self.contacts.push(GrainContact {
                        i,
                        j,
                        normal: dir,
                        relative_correction: (ci + cj).dot(dir),
                    });
                }
            }
        }
        for p in &mut self.particles {
            p.position = p.position.clamp_length_max(1.0 - p.radius);
        }
    }

    pub fn step(&mut self, dt: f32, motion: Motion) {
        if !dt.is_finite() || dt <= 0.0 || dt > 1.0 / 60.0 {
            return;
        }
        let motion = Motion {
            angular_velocity: if motion.angular_velocity.is_finite() {
                motion.angular_velocity.clamp(-3.0, 3.0)
            } else {
                0.0
            },
            shake: if motion.shake.is_finite() {
                motion.shake.clamp_length_max(12.0)
            } else {
                Vec2::ZERO
            },
        };
        self.prepare();
        self.neighbors();
        self.compute_accelerations(motion);
        self.angle = (self.angle + motion.angular_velocity * dt).rem_euclid(std::f32::consts::TAU);
        self.time += dt;
        let speed_limit = self.base_radius * 0.85 / dt;
        for i in 0..self.particles.len() {
            let p = &mut self.particles[i];
            self.previous[i] = p.position;
            let m = &self.materials[p.material];
            let target = Vec2::new(-p.position.y, p.position.x) * motion.angular_velocity;
            let drag = match m.phase {
                Phase::Solid => 0.3 + m.friction * 3.0,
                Phase::Liquid => 0.12 + m.viscosity * 0.35,
                Phase::Gas => 0.08,
            };
            p.velocity += self.acceleration[i].clamp_length_max(18.0) * dt;
            p.velocity += (target - p.velocity) * (1.0 - (-drag * dt).exp());
            p.velocity = p.velocity.clamp_length_max(speed_limit);
            p.position += p.velocity * dt;
        }
        self.contacts.clear();
        for _ in 0..ITERATIONS {
            self.constraints(true);
        }
        for (i, p) in self.particles.iter_mut().enumerate() {
            p.velocity = ((p.position - self.previous[i]) / dt).clamp_length_max(speed_limit);
            let m = &self.materials[p.material];
            if p.position.length_squared() >= (1.0 - p.radius - 0.0005).powi(2) {
                let normal = p.position.normalize_or_zero();
                p.velocity -= normal * p.velocity.dot(normal).max(0.0);
                if m.phase == Phase::Solid {
                    let target = Vec2::new(-p.position.y, p.position.x) * motion.angular_velocity;
                    p.velocity += (target - p.velocity) * (m.friction * 0.25);
                }
            }
        }
        // Fluid viscosity may couple fluid/fluid and fluid/grain motion.
        // Dry grains have no viscous connection to nearby, noncontacting grains.
        for k in 0..self.pairs.len() {
            let (i, j) = self.pairs[k];
            let a = &self.materials[self.particles[i].material];
            let b = &self.materials[self.particles[j].material];
            if a.phase == Phase::Solid && b.phase == Phase::Solid {
                continue;
            }
            let r = self.particles[i]
                .position
                .distance(self.particles[j].position);
            let rel = self.particles[i].velocity - self.particles[j].velocity;
            let wi = self.inverse_mass[i] / (self.inverse_mass[i] + self.inverse_mass[j]);
            let q = (1.0 - r / self.support).max(0.0);
            let impulse = rel * (dt * (a.viscosity + b.viscosity) * q * 2.0).min(0.08);
            self.particles[i].velocity -= impulse * wi;
            self.particles[j].velocity += impulse * (1.0 - wi);
        }
        // Friction requires an actual contact and is capped by its normal load.
        // A grazing or separating grain must be able to slide free of a cluster.
        for contact in &self.contacts {
            let (i, j) = (contact.i, contact.j);
            let normal = contact.normal;
            let a = &self.materials[self.particles[i].material];
            let b = &self.materials[self.particles[j].material];
            let rel = self.particles[i].velocity - self.particles[j].velocity;
            let closing = rel.dot(normal).min(0.0);
            let tangent = rel - normal * rel.dot(normal);
            let slip = tangent.length();
            let normal_load = contact.relative_correction / dt - closing;
            let friction_limit = (a.friction * b.friction).sqrt() * normal_load;
            let friction = if slip > 1e-6 {
                tangent * (friction_limit / slip).min(1.0)
            } else {
                Vec2::ZERO
            };
            let impulse = normal * closing + friction;
            let wi = self.inverse_mass[i] / (self.inverse_mass[i] + self.inverse_mass[j]);
            self.particles[i].velocity -= impulse * wi;
            self.particles[j].velocity += impulse * (1.0 - wi);
        }
    }
}
