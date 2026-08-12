//! Flight path generation for drone simulation.

use rand::Rng;
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Geographic coordinates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coordinates {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_m: f64,
    pub heading_deg: f32,
    pub speed_mps: f32,
}

impl Default for Coordinates {
    fn default() -> Self {
        Self {
            latitude: 31.6289,  // Kandahar
            longitude: 65.7372,
            altitude_m: 5000.0,
            heading_deg: 0.0,
            speed_mps: 80.0,
        }
    }
}

/// Waypoint definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Waypoint {
    pub id: Uuid,
    pub sequence: u32,
    pub name: String,
    pub coordinates: Coordinates,
    pub waypoint_type: WaypointType,
    pub loiter_time_sec: Option<u32>,
}

/// Type of waypoint.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum WaypointType {
    Takeoff,
    Navigation,
    Loiter,
    Target,
    Rtb,
    Landing,
}

/// Flight path generator.
pub struct FlightPathGenerator {
    /// Center point for mission area
    center: Coordinates,
    /// Mission radius in km
    radius_km: f64,
    /// Base altitude in meters
    base_altitude: f64,
    /// RNG
    rng: rand::rngs::ThreadRng,
}

impl FlightPathGenerator {
    /// Create a new flight path generator centered on a location.
    pub fn new(center: Coordinates, radius_km: f64) -> Self {
        let base_altitude = center.altitude_m;
        Self {
            center,
            radius_km,
            base_altitude,
            rng: rand::thread_rng(),
        }
    }

    /// Create generator for Kandahar AOR.
    pub fn kandahar() -> Self {
        Self::new(
            Coordinates {
                latitude: 31.6289,
                longitude: 65.7372,
                altitude_m: 5000.0,
                heading_deg: 0.0,
                speed_mps: 0.0,
            },
            50.0,
        )
    }

    /// Generate a complete mission flight path with 25 waypoints.
    pub fn generate_mission_path(&mut self, callsign: &str) -> Vec<Waypoint> {
        let mut waypoints = Vec::with_capacity(25);

        // Takeoff
        waypoints.push(self.create_waypoint(0, "TAKEOFF", WaypointType::Takeoff, None));

        // Climb to altitude
        waypoints.push(self.create_waypoint(1, "CLIMB", WaypointType::Navigation, None));

        // Ingress waypoints
        for i in 2..=6 {
            let name = format!("INGRESS-{}", i - 1);
            waypoints.push(self.create_waypoint(i, &name, WaypointType::Navigation, None));
        }

        // Target area with loiter points
        for i in 7..=15 {
            let wp_type = if i % 3 == 0 {
                WaypointType::Target
            } else {
                WaypointType::Loiter
            };
            let loiter = if wp_type == WaypointType::Loiter {
                Some(self.rng.gen_range(300..900))
            } else {
                None
            };
            let name = format!("OP-AREA-{}", i - 6);
            waypoints.push(self.create_waypoint(i, &name, wp_type, loiter));
        }

        // Egress waypoints
        for i in 16..=20 {
            let name = format!("EGRESS-{}", i - 15);
            waypoints.push(self.create_waypoint(i, &name, WaypointType::Navigation, None));
        }

        // RTB
        for i in 21..=23 {
            let name = format!("RTB-{}", i - 20);
            waypoints.push(self.create_waypoint(i, &name, WaypointType::Rtb, None));
        }

        // Descent and landing
        waypoints.push(self.create_waypoint(23, "DESCENT", WaypointType::Navigation, None));
        waypoints.push(self.create_waypoint(24, "LANDING", WaypointType::Landing, None));

        waypoints
    }

    /// Create a single waypoint.
    fn create_waypoint(
        &mut self,
        sequence: u32,
        name: &str,
        waypoint_type: WaypointType,
        loiter_time: Option<u32>,
    ) -> Waypoint {
        let coords = self.random_coordinates_in_area(waypoint_type);

        Waypoint {
            id: Uuid::new_v4(),
            sequence,
            name: name.to_string(),
            coordinates: coords,
            waypoint_type,
            loiter_time_sec: loiter_time,
        }
    }

    /// Generate random coordinates within mission area.
    fn random_coordinates_in_area(&mut self, wp_type: WaypointType) -> Coordinates {
        // Offset from center based on waypoint type
        let distance_factor = match wp_type {
            WaypointType::Takeoff | WaypointType::Landing => 0.1,
            WaypointType::Target | WaypointType::Loiter => 0.8,
            _ => self.rng.gen_range(0.3..0.9),
        };

        let angle: f64 = self.rng.gen_range(0.0..360.0);
        let distance = self.radius_km * distance_factor;

        // Convert to lat/lon offset (rough approximation)
        let lat_offset = (distance / 111.0) * angle.to_radians().cos();
        let lon_offset = (distance / 111.0) * angle.to_radians().sin();

        // Altitude variation
        // TODO(unwrap-sweep): infallible by construction (literal positive sigma),
        // but still a fallible constructor. Fix with a compiler in the loop.
        let alt_variation = Normal::new(0.0, 200.0).unwrap();
        let altitude = match wp_type {
            WaypointType::Takeoff => 0.0,
            WaypointType::Landing => 100.0,
            WaypointType::Target => self.base_altitude + 500.0,
            _ => self.base_altitude + alt_variation.sample(&mut self.rng),
        };

        // Calculate heading to next point (simplified)
        let heading = self.rng.gen_range(0.0..360.0) as f32;

        // Speed based on phase
        let speed = match wp_type {
            WaypointType::Takeoff => 40.0,
            WaypointType::Landing => 30.0,
            WaypointType::Loiter => 45.0,
            WaypointType::Target => 60.0,
            _ => self.rng.gen_range(70.0..100.0),
        };

        Coordinates {
            latitude: self.center.latitude + lat_offset,
            longitude: self.center.longitude + lon_offset,
            altitude_m: altitude.max(0.0),
            heading_deg: heading,
            speed_mps: speed,
        }
    }

    /// Interpolate position between two waypoints.
    pub fn interpolate(
        &self,
        from: &Coordinates,
        to: &Coordinates,
        progress: f64,
    ) -> Coordinates {
        let progress = progress.clamp(0.0, 1.0);

        Coordinates {
            latitude: from.latitude + (to.latitude - from.latitude) * progress,
            longitude: from.longitude + (to.longitude - from.longitude) * progress,
            altitude_m: from.altitude_m + (to.altitude_m - from.altitude_m) * progress,
            heading_deg: self.interpolate_heading(from.heading_deg, to.heading_deg, progress as f32),
            speed_mps: from.speed_mps + (to.speed_mps - from.speed_mps) * progress as f32,
        }
    }

    /// Interpolate heading (handling wrap-around).
    fn interpolate_heading(&self, from: f32, to: f32, progress: f32) -> f32 {
        let diff = ((to - from + 540.0) % 360.0) - 180.0;
        ((from + diff * progress) + 360.0) % 360.0
    }
}

impl PartialEq for WaypointType {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (WaypointType::Takeoff, WaypointType::Takeoff)
                | (WaypointType::Navigation, WaypointType::Navigation)
                | (WaypointType::Loiter, WaypointType::Loiter)
                | (WaypointType::Target, WaypointType::Target)
                | (WaypointType::Rtb, WaypointType::Rtb)
                | (WaypointType::Landing, WaypointType::Landing)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_mission_path() {
        let mut generator = FlightPathGenerator::kandahar();
        let path = generator.generate_mission_path("REAPER-01");
        assert_eq!(path.len(), 25);
        assert!(matches!(path[0].waypoint_type, WaypointType::Takeoff));
        assert!(matches!(path[24].waypoint_type, WaypointType::Landing));
    }

    #[test]
    fn test_interpolate() {
        let generator = FlightPathGenerator::kandahar();
        let from = Coordinates::default();
        let to = Coordinates {
            latitude: 32.0,
            longitude: 66.0,
            altitude_m: 6000.0,
            heading_deg: 90.0,
            speed_mps: 100.0,
        };

        let mid = generator.interpolate(&from, &to, 0.5);
        assert!((mid.latitude - 31.81445).abs() < 0.01);
    }
}
