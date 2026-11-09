use crate::{material::Recipe, simulation::{Motion, Simulation}};
use glam::Vec2;
use std::time::Instant;

pub fn run(count: usize, frames: usize) -> Result<String,String> {
    if frames < 10 { return Err("Measure at least 10 frames.".into()); }
    let mut recipe = Recipe::balanced(); recipe.budget = count;
    let mut output = format!("Particle solver | {} | fixed dt 1/120 | {} measured frame equivalents per workload\n", std::env::consts::ARCH, frames);
    for workload in ["settling", "rotation", "shaking"] {
        let mut sim = Simulation::new(&recipe)?;
        let mut times = Vec::with_capacity(frames);
        for frame in 0..frames+120 {
            let t = frame as f32/60.0;
            let motion = match workload {
                "rotation" => Motion { angular_velocity: 1.6, shake: Vec2::ZERO },
                "shaking" => Motion { angular_velocity: 0.6*(t*3.0).sin(), shake: Vec2::new(7.0*(t*12.0).sin(),4.0*(t*9.0).cos()) },
                _ => Motion::default(),
            };
            let start = Instant::now();
            sim.step(1.0/120.0,motion); sim.step(1.0/120.0,motion);
            if frame >= 120 { times.push(start.elapsed().as_secs_f64()*1000.0); }
        }
        if sim.particles.len() != count || sim.particles.iter().any(|p| !p.position.is_finite() || !p.velocity.is_finite() || p.position.length()+p.radius > 1.001) {
            return Err(format!("Invariant failure in {workload}"));
        }
        times.sort_by(f64::total_cmp);
        output.push_str(&format!("{workload:9} | particles {} | median {:.3} ms | p95 {:.3} ms | max {:.3} ms\n", sim.particles.len(), times[times.len()/2], times[((times.len() as f64*0.95) as usize).min(times.len()-1)], times[times.len()-1]));
    }
    Ok(output)
}
