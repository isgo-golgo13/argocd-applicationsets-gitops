//! Telemetry data generation for drone simulation.

use crate::flight::{Coordinates, FlightPathGenerator, Waypoint};
use chrono::{DateTime, Utc};
use rand::Rng;
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Telemetry snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetrySnapshot {
    pub drone_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub position: Coordinates,
    pub fuel_remaining_pct: f32,
    pub fuel_burn_rate: f32,
    pub engine_rpm: u32,
    pub engine_temp_c: f32,
    pub airspeed_mps: f32,
    pub ground_speed_mps: f32,
    pub vertical_speed_mps: f32,
    pub roll_deg: f32,
    pub pitch_deg: f32,
    pub yaw_deg: f32,
    pub gps_satellites: u8,
    pub signal_strength_dbm: i32,
    pub current_waypoint: u32,
    pub distance_to_waypoint_m: f64,
}

/// Telemetry generator for a single drone.
pub struct TelemetryGenerator {
    drone_id: Uuid,
    callsign: String,
    waypoints: Vec<Waypoint>,
    current_waypoint_idx: usize,
    fuel_remaining: f32,
    base_fuel_burn: f32,
    flight_gen: FlightPathGenerator,
    rng: rand::rngs::ThreadRng,
    noise: Normal<f64>,
}

impl TelemetryGenerator {
    /// Create a new telemetry generator for a drone.
    pub fn new(drone_id: Uuid, callsign: &str, waypoints: Vec<Waypoint>) -> Self {
        Self {
            drone_id,
            callsign: callsign.to_string(),
            waypoints,
            current_waypoint_idx: 0,
            fuel_remaining: 100.0,
            base_fuel_burn: 0.02,
            flight_gen: FlightPathGenerator::kandahar(),
            rng: rand::thread_rng(),
            noise: Normal::new(0.0, 1.0).unwrap(),
        }
    }

    /// Generate next telemetry snapshot.
    pub fn next_snapshot(&mut self, progress: f64) -> Option<TelemetrySnapshot> {
        if self.waypoints.is_empty() {
            return None;
        }

        // Update waypoint index based on progress
        let total_segments = self.waypoints.len() - 1;
        let segment_progress = progress * total_segments as f64;
        self.current_waypoint_idx = (segment_progress as usize).min(total_segments);

        // Get current and next waypoint
        let current_wp = &self.waypoints[self.current_waypoint_idx];
        let next_wp = self.waypoints.get(self.current_waypoint_idx + 1);

        // Interpolate position
        let position = if let Some(next) = next_wp {
            let local_progress = segment_progress.fract();
            self.flight_gen
                .interpolate(&current_wp.coordinates, &next.coordinates, local_progress)
        } else {
            current_wp.coordinates.clone()
        };

        // Update fuel
        self.fuel_remaining -= self.base_fuel_burn * (1.0 + self.noise.sample(&mut self.rng) as f32 * 0.1);
        self.fuel_remaining = self.fuel_remaining.max(0.0);

        // Generate telemetry with realistic noise
        let snapshot = TelemetrySnapshot {
            drone_id: self.drone_id,
            timestamp: Utc::now(),
            position: position.clone(),
            fuel_remaining_pct: self.fuel_remaining,
            fuel_burn_rate: self.base_fuel_burn + self.noise.sample(&mut self.rng) as f32 * 0.005,
            engine_rpm: 5500 + self.rng.gen_range(0..500),
            engine_temp_c: 85.0 + self.noise.sample(&mut self.rng) as f32 * 5.0,
            airspeed_mps: position.speed_mps + self.noise.sample(&mut self.rng) as f32 * 2.0,
            ground_speed_mps: position.speed_mps * 0.95 + self.noise.sample(&mut self.rng) as f32 * 3.0,
            vertical_speed_mps: self.noise.sample(&mut self.rng) as f32 * 5.0,
            roll_deg: self.noise.sample(&mut self.rng) as f32 * 3.0,
            pitch_deg: self.noise.sample(&mut self.rng) as f32 * 2.0,
            yaw_deg: position.heading_deg,
            gps_satellites: self.rng.gen_range(8..14),
            signal_strength_dbm: -60 + self.rng.gen_range(-15..5),
            current_waypoint: current_wp.sequence,
            distance_to_waypoint_m: self.calculate_distance_to_waypoint(&position, next_wp),
        };

        Some(snapshot)
    }

    /// Calculate distance to next waypoint.
    fn calculate_distance_to_waypoint(&self, pos: &Coordinates, next_wp: Option<&Waypoint>) -> f64 {
        match next_wp {
            Some(wp) => {
                let dlat = (wp.coordinates.latitude - pos.latitude).to_radians();
                let dlon = (wp.coordinates.longitude - pos.longitude).to_radians();
                let a = (dlat / 2.0).sin().powi(2)
                    + pos.latitude.to_radians().cos()
                        * wp.coordinates.latitude.to_radians().cos()
                        * (dlon / 2.0).sin().powi(2);
                let c = 2.0 * a.sqrt().asin();
                6371000.0 * c // Distance in meters
            }
            None => 0.0,
        }
    }

    /// Get current fuel level.
    pub fn fuel_remaining(&self) -> f32 {
        self.fuel_remaining
    }

    /// Check if drone is fuel critical.
    pub fn is_fuel_critical(&self) -> bool {
        self.fuel_remaining < 20.0
    }

    /// Get current waypoint index.
    pub fn current_waypoint(&self) -> usize {
        self.current_waypoint_idx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_generation() {
        let mut flight_gen = FlightPathGenerator::kandahar();
        let waypoints = flight_gen.generate_mission_path("TEST-01");
        let mut telem_gen = TelemetryGenerator::new(Uuid::new_v4(), "TEST-01", waypoints);

        let snapshot = telem_gen.next_snapshot(0.0).unwrap();
        assert_eq!(snapshot.current_waypoint, 0);
        assert!(snapshot.fuel_remaining_pct > 99.0);

        let snapshot = telem_gen.next_snapshot(0.5).unwrap();
        assert!(snapshot.current_waypoint > 0);
    }
}
