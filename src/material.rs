#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase { Liquid, Gas, Solid }

#[derive(Clone, Debug)]
pub struct Material {
    pub name: String,
    pub phase: Phase,
    pub share: f32,
    pub density: f32,
    pub viscosity: f32,
    pub radius: f32,
    pub friction: f32,
    pub color: [u8; 3],
}

#[derive(Clone, Debug)]
pub struct Recipe {
    pub materials: Vec<Material>,
    pub budget: usize,
    pub seed: u64,
}

impl Recipe {
    pub fn balanced() -> Self {
        let make = |name: &str, phase, share, density, viscosity, radius, friction, color| Material {
            name: name.to_owned(), phase, share, density, viscosity, radius, friction, color,
        };
        Self { materials: vec![
            make("Water", Phase::Liquid, 44.0, 1.0, 0.65, 1.0, 0.0, [47, 123, 145]),
            make("Oil", Phase::Liquid, 8.0, 0.78, 2.0, 1.0, 0.0, [183, 144, 76]),
            make("Air", Phase::Gas, 12.0, 0.04, 0.08, 1.0, 0.0, [13, 30, 40]),
            make("Golden sand", Phase::Solid, 18.0, 1.65, 0.25, 0.85, 0.65, [230, 188, 105]),
            make("Terracotta", Phase::Solid, 11.0, 2.4, 0.35, 1.1, 0.75, [193, 103, 78]),
            make("Obsidian", Phase::Solid, 7.0, 3.2, 0.3, 1.35, 0.85, [115, 133, 145]),
        ], budget: 3000, seed: 42 }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.budget == 0 || self.budget > 100_000 || self.materials.is_empty() || self.materials.len() > 16 {
            return Err("Choose 1–100,000 particles and 1–16 materials.".into());
        }
        let mut total = 0.0;
        let mut enabled = 0;
        for m in &self.materials {
            if !m.share.is_finite() || !(0.0..=10_000.0).contains(&m.share)
                || !m.density.is_finite() || !(0.02..=10.0).contains(&m.density)
                || !m.viscosity.is_finite() || !(0.0..=8.0).contains(&m.viscosity)
                || !m.radius.is_finite() || !(0.5..=2.0).contains(&m.radius)
                || !m.friction.is_finite() || !(0.0..=1.0).contains(&m.friction) {
                return Err(format!("{} has a property outside its supported range.", m.name));
            }
            if m.share > 0.0 { enabled += 1; }
            total += m.share;
        }
        if total <= 0.0 { return Err("Give at least one material a positive share.".into()); }
        if enabled > self.budget { return Err("Particle budget must represent every enabled material.".into()); }
        Ok(())
    }
    pub fn counts(&self) -> Result<Vec<usize>, String> {
        self.validate()?;
        let weights: Vec<f64> = self.materials.iter().map(|m| m.share as f64 / (m.radius as f64).powi(2)).collect();
        let total: f64 = weights.iter().sum();
        let targets: Vec<f64> = weights.iter().map(|w| w / total * self.budget as f64).collect();
        let mut counts: Vec<usize> = targets.iter().zip(&weights).map(|(v,w)| if *w > 0.0 { (*v as usize).max(1) } else { 0 }).collect();
        let mut sum: usize = counts.iter().sum();
        while sum < self.budget {
            let i = (0..counts.len()).filter(|i| weights[*i] > 0.0).max_by(|a,b| (targets[*a] - counts[*a] as f64).total_cmp(&(targets[*b] - counts[*b] as f64))).unwrap();
            counts[i] += 1; sum += 1;
        }
        while sum > self.budget {
            let i = (0..counts.len()).filter(|i| counts[*i] > 1).max_by(|a,b| (counts[*a] as f64 - targets[*a]).total_cmp(&(counts[*b] as f64 - targets[*b]))).ok_or("Not enough particles for this mixture.")?;
            counts[i] -= 1; sum -= 1;
        }
        Ok(counts)
    }
}
