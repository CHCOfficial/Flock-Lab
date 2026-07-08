use crate::simulation::spatial_hash::SpatialHash;
use glam::Vec3;
use rand::{rngs::StdRng, Rng, SeedableRng};

const MIN_SPEED: f32 = 0.8;

#[derive(Debug, Clone)]
pub struct SimulationSettings {
    pub agent_count: usize,
    pub max_speed: f32,
    pub separation_strength: f32,
    pub alignment_strength: f32,
    pub cohesion_strength: f32,
    pub predator_avoidance_strength: f32,
    pub goal_strength: f32,
    pub obstacle_avoidance_strength: f32,
    pub predator_count: usize,
    pub obstacle_count: usize,
    pub trail_length: usize,
    pub bounds: f32,
    pub neighbor_radius: f32,
    pub separation_radius: f32,
    pub pause: bool,
}

impl Default for SimulationSettings {
    fn default() -> Self {
        Self {
            agent_count: 3_500,
            max_speed: 18.0,
            separation_strength: 2.0,
            alignment_strength: 0.9,
            cohesion_strength: 0.55,
            predator_avoidance_strength: 4.5,
            goal_strength: 0.12,
            obstacle_avoidance_strength: 2.8,
            predator_count: 2,
            obstacle_count: 10,
            trail_length: 18,
            bounds: 90.0,
            neighbor_radius: 8.5,
            separation_radius: 3.0,
            pause: false,
        }
    }
}

impl SimulationSettings {
    pub fn validated(mut self) -> Self {
        self.agent_count = self.agent_count.clamp(1, 50_000);
        self.max_speed = self.max_speed.clamp(MIN_SPEED, 80.0);
        self.separation_strength = self.separation_strength.clamp(0.0, 12.0);
        self.alignment_strength = self.alignment_strength.clamp(0.0, 12.0);
        self.cohesion_strength = self.cohesion_strength.clamp(0.0, 12.0);
        self.predator_avoidance_strength = self.predator_avoidance_strength.clamp(0.0, 20.0);
        self.goal_strength = self.goal_strength.clamp(0.0, 5.0);
        self.obstacle_avoidance_strength = self.obstacle_avoidance_strength.clamp(0.0, 20.0);
        self.predator_count = self.predator_count.clamp(0, 32);
        self.obstacle_count = self.obstacle_count.clamp(0, 128);
        self.trail_length = self.trail_length.clamp(0, 96);
        self.bounds = self.bounds.clamp(20.0, 300.0);
        self.neighbor_radius = self.neighbor_radius.clamp(1.0, 40.0);
        self.separation_radius = self.separation_radius.clamp(0.25, self.neighbor_radius);
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Boid {
    pub position: Vec3,
    pub velocity: Vec3,
    pub hue: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Predator {
    pub position: Vec3,
    pub velocity: Vec3,
}

#[derive(Debug, Clone, Copy)]
pub struct Obstacle {
    pub position: Vec3,
    pub radius: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SimulationStats {
    pub average_speed: f32,
    pub neighbor_samples: usize,
}

#[derive(Debug)]
pub struct FlockSimulation {
    pub settings: SimulationSettings,
    pub boids: Vec<Boid>,
    pub predators: Vec<Predator>,
    pub obstacles: Vec<Obstacle>,
    pub trails: Vec<Vec<Vec3>>,
    pub goal: Vec3,
    pub stats: SimulationStats,
    rng: StdRng,
    spatial_hash: SpatialHash,
    time: f32,
}

impl FlockSimulation {
    pub fn new(settings: SimulationSettings) -> Self {
        let settings = settings.validated();
        let mut simulation = Self {
            spatial_hash: SpatialHash::new(settings.neighbor_radius),
            settings,
            boids: Vec::new(),
            predators: Vec::new(),
            obstacles: Vec::new(),
            trails: Vec::new(),
            goal: Vec3::ZERO,
            stats: SimulationStats::default(),
            rng: StdRng::seed_from_u64(0xF10C_AB1E),
            time: 0.0,
        };
        simulation.randomize();
        simulation
    }

    pub fn randomize(&mut self) {
        self.settings = self.settings.clone().validated();
        self.spatial_hash.set_cell_size(self.settings.neighbor_radius);
        self.boids.clear();
        self.predators.clear();
        self.obstacles.clear();
        self.trails.clear();
        self.goal = Vec3::ZERO;
        self.time = 0.0;

        for index in 0..self.settings.agent_count {
            let position = self.random_in_bounds(0.72);
            let velocity = self.random_unit() * self.rng.gen_range(4.0..self.settings.max_speed);
            self.boids.push(Boid {
                position,
                velocity,
                hue: index as f32 / self.settings.agent_count.max(1) as f32,
            });
        }
        self.trails = self
            .boids
            .iter()
            .map(|boid| vec![boid.position])
            .collect::<Vec<_>>();
        self.ensure_predator_count();
        self.ensure_obstacle_count();
    }

    pub fn reset(&mut self) {
        self.randomize();
    }

    pub fn update(&mut self, dt: f32) {
        self.settings = self.settings.clone().validated();
        self.resize_agents();
        self.ensure_predator_count();
        self.ensure_obstacle_count();
        self.spatial_hash.set_cell_size(self.settings.neighbor_radius);

        if self.settings.pause {
            return;
        }

        let dt = dt.clamp(0.0, 1.0 / 20.0);
        self.time += dt;
        self.goal = Vec3::new(
            (self.time * 0.31).cos() * self.settings.bounds * 0.36,
            (self.time * 0.21).sin() * self.settings.bounds * 0.18,
            (self.time * 0.27).sin() * self.settings.bounds * 0.36,
        );

        self.update_predators(dt);
        self.spatial_hash
            .rebuild(self.boids.iter().enumerate().map(|(i, boid)| (i, boid.position)));

        let previous = self.boids.clone();
        let mut next = previous.clone();
        let mut total_speed = 0.0;
        let mut neighbor_samples = 0;

        for (index, boid) in previous.iter().enumerate() {
            let (force, sampled_neighbors) = self.boid_force_and_neighbor_count(index, &previous);
            let mut velocity = boid.velocity + force * dt;
            velocity = clamp_velocity(velocity, self.settings.max_speed);

            let mut position = boid.position + velocity * dt;
            apply_bounds(&mut position, &mut velocity, self.settings.bounds);

            next[index].position = position;
            next[index].velocity = velocity;
            total_speed += velocity.length();
            neighbor_samples += sampled_neighbors;
        }

        self.boids = next;
        self.stats = SimulationStats {
            average_speed: total_speed / self.boids.len().max(1) as f32,
            neighbor_samples,
        };
        self.update_trails();
    }

    pub fn boid_force(&self, index: usize, snapshot: &[Boid]) -> Vec3 {
        self.boid_force_and_neighbor_count(index, snapshot).0
    }

    fn boid_force_and_neighbor_count(&self, index: usize, snapshot: &[Boid]) -> (Vec3, usize) {
        let boid = snapshot[index];
        let mut separation = Vec3::ZERO;
        let mut alignment = Vec3::ZERO;
        let mut cohesion = Vec3::ZERO;
        let mut neighbor_count = 0.0;
        let mut sampled_neighbors = 0;
        let neighbor_radius_sq = self.settings.neighbor_radius * self.settings.neighbor_radius;
        let separation_radius_sq = self.settings.separation_radius * self.settings.separation_radius;

        for candidate in self.spatial_hash.nearby_indices(boid.position) {
            if candidate == index {
                continue;
            }
            let other = snapshot[candidate];
            let offset = other.position - boid.position;
            let distance_sq = offset.length_squared();
            if distance_sq > neighbor_radius_sq || distance_sq <= f32::EPSILON {
                continue;
            }

            neighbor_count += 1.0;
            sampled_neighbors += 1;
            alignment += other.velocity;
            cohesion += other.position;
            if distance_sq < separation_radius_sq {
                separation -= offset.normalize_or_zero() / distance_sq.sqrt().max(0.001);
            }
        }

        let mut force = Vec3::ZERO;
        if neighbor_count > 0.0 {
            let inv_count = 1.0 / neighbor_count;
            let desired_alignment = (alignment * inv_count).normalize_or_zero() * self.settings.max_speed;
            let center = cohesion * inv_count;
            force += (desired_alignment - boid.velocity) * self.settings.alignment_strength;
            force += (center - boid.position).normalize_or_zero()
                * self.settings.cohesion_strength
                * self.settings.max_speed;
            force += separation.normalize_or_zero()
                * self.settings.separation_strength
                * self.settings.max_speed;
        }

        force += (self.goal - boid.position).normalize_or_zero()
            * self.settings.goal_strength
            * self.settings.max_speed;
        force += self.predator_avoidance(boid.position);
        force += self.obstacle_avoidance(boid.position);
        force += self.boundary_force(boid.position);
        (force, sampled_neighbors)
    }

    fn predator_avoidance(&self, position: Vec3) -> Vec3 {
        let mut force = Vec3::ZERO;
        let range = self.settings.neighbor_radius * 3.4;
        let range_sq = range * range;
        for predator in &self.predators {
            let offset = position - predator.position;
            let distance_sq = offset.length_squared();
            if distance_sq < range_sq && distance_sq > f32::EPSILON {
                let falloff = 1.0 - (distance_sq.sqrt() / range);
                force += offset.normalize_or_zero()
                    * falloff
                    * self.settings.predator_avoidance_strength
                    * self.settings.max_speed;
            }
        }
        force
    }

    fn obstacle_avoidance(&self, position: Vec3) -> Vec3 {
        let mut force = Vec3::ZERO;
        for obstacle in &self.obstacles {
            let offset = position - obstacle.position;
            let safe_radius = obstacle.radius + self.settings.neighbor_radius;
            let distance = offset.length();
            if distance < safe_radius && distance > f32::EPSILON {
                let falloff = 1.0 - distance / safe_radius;
                force += offset.normalize_or_zero()
                    * falloff
                    * self.settings.obstacle_avoidance_strength
                    * self.settings.max_speed;
            }
        }
        force
    }

    fn boundary_force(&self, position: Vec3) -> Vec3 {
        let bound = self.settings.bounds;
        let margin = bound * 0.18;
        let mut force = Vec3::ZERO;
        for axis in 0..3 {
            let value = position[axis];
            let distance_to_wall = bound - value.abs();
            if distance_to_wall < margin {
                force[axis] -= value.signum() * (1.0 - distance_to_wall / margin) * self.settings.max_speed;
            }
        }
        force
    }

    fn resize_agents(&mut self) {
        let target = self.settings.agent_count;
        while self.boids.len() < target {
            let position = self.random_in_bounds(0.65);
            let velocity = self.random_unit() * self.rng.gen_range(4.0..self.settings.max_speed);
            let hue = self.rng.gen();
            self.boids.push(Boid {
                position,
                velocity,
                hue,
            });
            self.trails.push(vec![position]);
        }
        self.boids.truncate(target);
        self.trails.truncate(target);
    }

    fn ensure_predator_count(&mut self) {
        while self.predators.len() < self.settings.predator_count {
            let position = self.random_in_bounds(0.45);
            let velocity = self.random_unit() * self.settings.max_speed * 0.72;
            self.predators.push(Predator {
                position,
                velocity,
            });
        }
        self.predators.truncate(self.settings.predator_count);
    }

    fn ensure_obstacle_count(&mut self) {
        while self.obstacles.len() < self.settings.obstacle_count {
            let position = self.random_in_bounds(0.55);
            let radius = self.rng.gen_range(4.0..11.0);
            self.obstacles.push(Obstacle {
                position,
                radius,
            });
        }
        self.obstacles.truncate(self.settings.obstacle_count);
    }

    fn update_predators(&mut self, dt: f32) {
        let bound = self.settings.bounds * 0.82;
        for (index, predator) in self.predators.iter_mut().enumerate() {
            let target = Vec3::new(
                ((self.time * 0.43) + index as f32).cos() * bound,
                ((self.time * 0.37) + index as f32 * 1.7).sin() * bound * 0.36,
                ((self.time * 0.49) + index as f32 * 0.8).sin() * bound,
            );
            let desired = (target - predator.position).normalize_or_zero()
                * self.settings.max_speed
                * 0.9;
            predator.velocity += (desired - predator.velocity) * 0.8 * dt;
            predator.velocity = clamp_velocity(predator.velocity, self.settings.max_speed * 1.25);
            predator.position += predator.velocity * dt;
            apply_bounds(&mut predator.position, &mut predator.velocity, self.settings.bounds);
        }
    }

    fn update_trails(&mut self) {
        let max_len = self.settings.trail_length;
        if max_len == 0 {
            for trail in &mut self.trails {
                trail.clear();
            }
            return;
        }

        for (trail, boid) in self.trails.iter_mut().zip(self.boids.iter()) {
            trail.push(boid.position);
            let overflow = trail.len().saturating_sub(max_len);
            if overflow > 0 {
                trail.drain(0..overflow);
            }
        }
    }

    fn random_in_bounds(&mut self, scale: f32) -> Vec3 {
        let radius = self.settings.bounds * scale;
        Vec3::new(
            self.rng.gen_range(-radius..radius),
            self.rng.gen_range(-radius * 0.55..radius * 0.55),
            self.rng.gen_range(-radius..radius),
        )
    }

    fn random_unit(&mut self) -> Vec3 {
        loop {
            let candidate = Vec3::new(
                self.rng.gen_range(-1.0..1.0),
                self.rng.gen_range(-1.0..1.0),
                self.rng.gen_range(-1.0..1.0),
            );
            if candidate.length_squared() > 0.001 {
                return candidate.normalize();
            }
        }
    }
}

pub fn clamp_velocity(velocity: Vec3, max_speed: f32) -> Vec3 {
    let speed = velocity.length();
    if speed > max_speed {
        velocity / speed * max_speed
    } else if speed < MIN_SPEED && speed > f32::EPSILON {
        velocity / speed * MIN_SPEED
    } else if speed <= f32::EPSILON {
        Vec3::X * MIN_SPEED
    } else {
        velocity
    }
}

pub fn apply_bounds(position: &mut Vec3, velocity: &mut Vec3, bounds: f32) {
    for axis in 0..3 {
        if position[axis] > bounds {
            position[axis] = bounds;
            velocity[axis] = -velocity[axis].abs();
        } else if position[axis] < -bounds {
            position[axis] = -bounds;
            velocity[axis] = velocity[axis].abs();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn settings_validation_clamps_unsafe_values() {
        let settings = SimulationSettings {
            agent_count: 0,
            max_speed: 0.01,
            separation_radius: 99.0,
            neighbor_radius: 2.0,
            trail_length: 9_999,
            bounds: 2.0,
            ..SimulationSettings::default()
        }
        .validated();

        assert_eq!(settings.agent_count, 1);
        assert_relative_eq!(settings.max_speed, MIN_SPEED);
        assert_relative_eq!(settings.separation_radius, settings.neighbor_radius);
        assert_eq!(settings.trail_length, 96);
        assert_relative_eq!(settings.bounds, 20.0);
    }

    #[test]
    fn separation_pushes_boids_apart() {
        let mut simulation = FlockSimulation::new(SimulationSettings {
            agent_count: 2,
            alignment_strength: 0.0,
            cohesion_strength: 0.0,
            goal_strength: 0.0,
            predator_count: 0,
            obstacle_count: 0,
            neighbor_radius: 10.0,
            separation_radius: 5.0,
            ..SimulationSettings::default()
        });
        simulation.boids = vec![
            Boid {
                position: Vec3::ZERO,
                velocity: Vec3::X,
                hue: 0.0,
            },
            Boid {
                position: Vec3::X,
                velocity: Vec3::X,
                hue: 0.1,
            },
        ];
        simulation
            .spatial_hash
            .rebuild(simulation.boids.iter().enumerate().map(|(i, b)| (i, b.position)));

        let force = simulation.boid_force(0, &simulation.boids);
        assert!(force.x < 0.0, "expected force away from nearby boid, got {force:?}");
    }

    #[test]
    fn bounds_reflect_velocity_and_clamp_position() {
        let mut position = Vec3::new(12.0, -13.0, 2.0);
        let mut velocity = Vec3::new(4.0, -6.0, 1.0);
        apply_bounds(&mut position, &mut velocity, 10.0);

        assert_relative_eq!(position.x, 10.0);
        assert_relative_eq!(position.y, -10.0);
        assert!(velocity.x < 0.0);
        assert!(velocity.y > 0.0);
    }
}
