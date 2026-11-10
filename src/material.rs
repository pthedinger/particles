use std::ops::{Deref, DerefMut};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Liquid,
    Gas,
    Solid,
}
impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Liquid => "Liquid",
            Self::Gas => "Gas",
            Self::Solid => "Solid",
        }
    }
}

/// Reference properties at 20°C. Solver units are relative, not calibrated SI.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialProperties {
    pub density: f32,
    pub viscosity: f32,
    pub radius: f32,
    pub friction: f32,
    pub color: [u8; 3],
    pub surface_tension: f32,
    pub compression_resistance: f32,
    pub mixing_resistance: f32,
    pub thermal_expansion: f32,
    pub viscosity_temperature: f32,
    pub thermal_motion: f32,
}
impl MaterialProperties {
    pub fn for_phase(phase: Phase) -> Self {
        Self {
            density: match phase {
                Phase::Liquid => 1.,
                Phase::Gas => 0.04,
                Phase::Solid => 2.,
            },
            viscosity: match phase {
                Phase::Liquid => 0.65,
                Phase::Gas => 0.08,
                Phase::Solid => 0.25,
            },
            radius: 1.,
            friction: if phase == Phase::Solid { 0.65 } else { 0. },
            color: match phase {
                Phase::Liquid => [47, 123, 145],
                Phase::Gas => [35, 60, 75],
                Phase::Solid => [230, 188, 105],
            },
            surface_tension: if phase == Phase::Solid { 0. } else { 1. },
            compression_resistance: if phase == Phase::Gas { 0.25 } else { 1. },
            mixing_resistance: 1.,
            thermal_expansion: match phase {
                Phase::Liquid => 0.0003,
                Phase::Gas => 1. / 293.15,
                Phase::Solid => 0.00002,
            },
            viscosity_temperature: match phase {
                Phase::Liquid => 0.025,
                Phase::Gas => -0.002,
                Phase::Solid => 0.,
            },
            thermal_motion: match phase {
                Phase::Liquid => 0.005,
                Phase::Gas => 0.05,
                Phase::Solid => 0.,
            },
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub name: String,
    pub phase: Phase,
    pub share: f32,
    pub properties: MaterialProperties,
    neighbor_target: f32,
}
impl Deref for Material {
    type Target = MaterialProperties;
    fn deref(&self) -> &Self::Target {
        &self.properties
    }
}
impl DerefMut for Material {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.properties
    }
}
impl Material {
    pub fn new(name: &str, phase: Phase, share: f32, properties: MaterialProperties) -> Self {
        Self {
            name: name.into(),
            phase,
            share,
            properties,
            neighbor_target: 1.65,
        }
    }
    pub fn neighbor_target(&self) -> f32 {
        self.neighbor_target
    }
    pub fn at_temperature(&self, celsius: f32) -> Self {
        if celsius == 20. {
            return self.clone();
        }
        let t = celsius.clamp(-20., 150.);
        let delta = t - 20.;
        let kelvin_ratio = (t + 273.15) / 293.15;
        let mut result = self.clone();
        result.density =
            (self.density / (1. + self.thermal_expansion * delta).clamp(0.2, 5.)).clamp(0.005, 20.);
        result.viscosity =
            (self.viscosity * (-self.viscosity_temperature * delta).exp()).clamp(0., 32.);
        result.thermal_motion = self.thermal_motion * kelvin_ratio.sqrt();
        result.surface_tension = self.surface_tension * (1. - 0.0015 * delta).clamp(0.1, 1.5);
        if self.phase == Phase::Gas {
            result.neighbor_target = 1.65 / kelvin_ratio;
        }
        result
    }
}
#[derive(Clone, Debug)]
pub struct Recipe {
    pub materials: Vec<Material>,
    pub budget: usize,
    pub seed: u64,
    pub temperature_c: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    SandArt,
    DryGrains,
    LiquidsAndBubbles,
    ViscousSlurry,
    FloatAndSink,
    PureWater,
}
impl Preset {
    pub const ALL: [Self; 6] = [
        Self::SandArt,
        Self::DryGrains,
        Self::LiquidsAndBubbles,
        Self::ViscousSlurry,
        Self::FloatAndSink,
        Self::PureWater,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::SandArt => "Sand art",
            Self::DryGrains => "Dry grains",
            Self::LiquidsAndBubbles => "Liquids + bubbles",
            Self::ViscousSlurry => "Viscous slurry",
            Self::FloatAndSink => "Float + sink",
            Self::PureWater => "Pure water",
        }
    }
    pub fn recipe(self) -> Recipe {
        let mut r = Recipe::balanced();
        let shares = match self {
            Self::SandArt => [44., 8., 12., 18., 11., 7.],
            Self::DryGrains => [0., 0., 0., 50., 30., 20.],
            Self::LiquidsAndBubbles => [65., 15., 20., 0., 0., 0.],
            Self::ViscousSlurry => [55., 0., 10., 20., 10., 5.],
            Self::FloatAndSink => [70., 0., 10., 10., 5., 5.],
            Self::PureWater => [100., 0., 0., 0., 0., 0.],
        };
        for (m, s) in r.materials.iter_mut().zip(shares) {
            m.share = s;
        }
        if self == Self::ViscousSlurry {
            r.materials[0].viscosity = 5.;
            r.materials[0].surface_tension = 0.3;
        }
        if self == Self::FloatAndSink {
            r.materials[3].name = "Light grains".into();
            r.materials[3].density = 0.6;
        }
        r
    }
}

impl Recipe {
    pub fn balanced() -> Self {
        let make = |name: &str, phase, share, density, viscosity, radius, friction, color| {
            let mut properties = MaterialProperties::for_phase(phase);
            properties.density = density;
            properties.viscosity = viscosity;
            properties.radius = radius;
            properties.friction = friction;
            properties.color = color;
            if name == "Oil" {
                properties.thermal_expansion = 0.0007;
                properties.viscosity_temperature = 0.03;
            }
            Material::new(name, phase, share, properties)
        };
        Self {
            materials: vec![
                make(
                    "Water",
                    Phase::Liquid,
                    44.0,
                    1.0,
                    0.65,
                    1.0,
                    0.0,
                    [47, 123, 145],
                ),
                make(
                    "Oil",
                    Phase::Liquid,
                    8.0,
                    0.78,
                    2.0,
                    1.0,
                    0.0,
                    [183, 144, 76],
                ),
                make("Air", Phase::Gas, 12.0, 0.04, 0.08, 1.0, 0.0, [13, 30, 40]),
                make(
                    "Golden sand",
                    Phase::Solid,
                    18.0,
                    1.65,
                    0.25,
                    0.85,
                    0.65,
                    [230, 188, 105],
                ),
                make(
                    "Terracotta",
                    Phase::Solid,
                    11.0,
                    2.4,
                    0.35,
                    1.1,
                    0.75,
                    [193, 103, 78],
                ),
                make(
                    "Obsidian",
                    Phase::Solid,
                    7.0,
                    3.2,
                    0.3,
                    1.35,
                    0.85,
                    [115, 133, 145],
                ),
            ],
            budget: 3000,
            seed: 42,
            temperature_c: 20.,
        }
    }
    pub fn effective_materials(&self) -> Vec<Material> {
        self.materials
            .iter()
            .map(|m| m.at_temperature(self.temperature_c))
            .collect()
    }
    pub fn set_percentage(&mut self, index: usize, percentage: f32) -> Result<(), String> {
        if index >= self.materials.len()
            || !percentage.is_finite()
            || !(0.0..=100.0).contains(&percentage)
        {
            return Err("Choose a valid material and 0–100%.".into());
        }
        if self.materials.len() == 1 {
            if percentage != 100. {
                return Err("The only material must occupy 100%.".into());
            }
            self.materials[0].share = 100.;
            return Ok(());
        }
        let remainder = 100. - percentage;
        let others: f64 = self
            .materials
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != index)
            .map(|(_, m)| m.share.max(0.) as f64)
            .sum();
        for (i, m) in self.materials.iter_mut().enumerate() {
            if i == index {
                m.share = percentage;
            } else {
                m.share = if others > 0. {
                    (m.share.max(0.) as f64 / others * remainder as f64) as f32
                } else {
                    0.
                };
            }
        }
        if remainder > 0. && others == 0. {
            let fallback = if index == 0 { 1 } else { 0 };
            self.materials[fallback].share = remainder;
        }
        let sum: f32 = self.materials.iter().map(|m| m.share).sum();
        if let Some(i) = (0..self.materials.len())
            .filter(|i| *i != index && self.materials[*i].share > 0.)
            .max_by(|a, b| {
                self.materials[*a]
                    .share
                    .total_cmp(&self.materials[*b].share)
            })
        {
            self.materials[i].share += 100. - sum;
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), String> {
        if !self.temperature_c.is_finite() || !(-20.0..=150.0).contains(&self.temperature_c) {
            return Err("Choose a temperature from -20 to 150°C.".into());
        }
        if self.budget == 0
            || self.budget > 100_000
            || self.materials.is_empty()
            || self.materials.len() > 16
        {
            return Err("Choose 1–100,000 particles and 1–16 materials.".into());
        }
        let mut total = 0.0;
        let mut enabled = 0;
        for m in &self.materials {
            if !m.surface_tension.is_finite()
                || !(0.0..=2.0).contains(&m.surface_tension)
                || !m.compression_resistance.is_finite()
                || !(0.05..=1.0).contains(&m.compression_resistance)
                || !m.mixing_resistance.is_finite()
                || !(0.0..=1.0).contains(&m.mixing_resistance)
                || !m.thermal_expansion.is_finite()
                || !(0.0..=0.005).contains(&m.thermal_expansion)
                || !m.viscosity_temperature.is_finite()
                || !(-0.05..=0.05).contains(&m.viscosity_temperature)
                || !m.thermal_motion.is_finite()
                || !(0.0..=4.0).contains(&m.thermal_motion)
                || !m.share.is_finite()
                || !(0.0..=10_000.0).contains(&m.share)
                || !m.density.is_finite()
                || !(0.02..=10.0).contains(&m.density)
                || !m.viscosity.is_finite()
                || !(0.0..=8.0).contains(&m.viscosity)
                || !m.radius.is_finite()
                || !(0.5..=2.0).contains(&m.radius)
                || !m.friction.is_finite()
                || !(0.0..=1.0).contains(&m.friction)
            {
                return Err(format!(
                    "{} has a property outside its supported range.",
                    m.name
                ));
            }
            if m.share > 0.0 {
                enabled += 1;
            }
            total += m.share;
        }
        if total <= 0.0 {
            return Err("Give at least one material a positive share.".into());
        }
        if enabled > self.budget {
            return Err("Particle budget must represent every enabled material.".into());
        }
        Ok(())
    }
    pub fn counts(&self) -> Result<Vec<usize>, String> {
        self.validate()?;
        let weights: Vec<f64> = self
            .materials
            .iter()
            .map(|m| m.share as f64 / (m.radius as f64).powi(2))
            .collect();
        let total: f64 = weights.iter().sum();
        let targets: Vec<f64> = weights
            .iter()
            .map(|w| w / total * self.budget as f64)
            .collect();
        let mut counts: Vec<usize> = targets
            .iter()
            .zip(&weights)
            .map(|(v, w)| if *w > 0.0 { (*v as usize).max(1) } else { 0 })
            .collect();
        let mut sum: usize = counts.iter().sum();
        while sum < self.budget {
            let i = (0..counts.len())
                .filter(|i| weights[*i] > 0.0)
                .max_by(|a, b| {
                    (targets[*a] - counts[*a] as f64).total_cmp(&(targets[*b] - counts[*b] as f64))
                })
                .unwrap();
            counts[i] += 1;
            sum += 1;
        }
        while sum > self.budget {
            let i = (0..counts.len())
                .filter(|i| counts[*i] > 1)
                .max_by(|a, b| {
                    (counts[*a] as f64 - targets[*a]).total_cmp(&(counts[*b] as f64 - targets[*b]))
                })
                .ok_or("Not enough particles for this mixture.")?;
            counts[i] -= 1;
            sum -= 1;
        }
        Ok(counts)
    }
}
