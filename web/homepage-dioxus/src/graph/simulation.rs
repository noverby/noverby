use glam::Vec3;

use super::data::{LINKS, NODES};

pub struct Simulation {
    pub positions: Vec<Vec3>,
    pub velocities: Vec<Vec3>,
    alpha: f32,
    alpha_decay: f32,
    alpha_min: f32,
    velocity_decay: f32,
}

impl Simulation {
    pub fn new() -> Self {
        let n = NODES.len();
        let mut positions = Vec::with_capacity(n);
        // Initialize positions in a deterministic spread using a simple hash-like pattern
        for i in 0..n {
            let fi = i as f32;
            let x = (fi * 137.508).sin() * 50.0;
            let y = (fi * 73.254).cos() * 50.0;
            let z = (fi * 41.132).sin() * 50.0;
            positions.push(Vec3::new(x, y, z));
        }
        let velocities = vec![Vec3::ZERO; n];

        Self {
            positions,
            velocities,
            alpha: 1.0,
            alpha_decay: 0.99,
            alpha_min: 0.001,
            velocity_decay: 0.6,
        }
    }

    pub fn is_active(&self) -> bool {
        self.alpha > self.alpha_min
    }

    pub fn tick(&mut self) {
        if !self.is_active() {
            return;
        }

        let n = self.positions.len();

        // Charge force (repulsion between all pairs)
        let strength = -300.0;
        for i in 0..n {
            for j in (i + 1)..n {
                let diff = self.positions[i] - self.positions[j];
                let dist_sq = diff.length_squared().max(1.0);
                let force = diff.normalize_or_zero() * (strength * self.alpha / dist_sq);
                self.velocities[i] += force;
                self.velocities[j] -= force;
            }
        }

        // Link force (spring attraction)
        let link_strength = 0.03;
        let link_distance = 100.0;
        for link in LINKS {
            let si = node_index(link.source);
            let ti = node_index(link.target);
            if let (Some(si), Some(ti)) = (si, ti) {
                let diff = self.positions[ti] - self.positions[si];
                let dist = diff.length().max(0.01);
                let force = diff.normalize_or_zero()
                    * ((dist - link_distance) * link_strength * self.alpha);
                self.velocities[si] += force;
                self.velocities[ti] -= force;
            }
        }

        // Center force
        let center_strength = 0.05;
        for i in 0..n {
            self.velocities[i] -= self.positions[i] * center_strength * self.alpha;
        }

        // Apply velocities and damping
        for i in 0..n {
            self.velocities[i] *= self.velocity_decay;
            self.positions[i] += self.velocities[i];
        }

        // Cool alpha
        self.alpha *= self.alpha_decay;
    }
}

fn node_index(id: &str) -> Option<usize> {
    NODES.iter().position(|n| n.id == id)
}
