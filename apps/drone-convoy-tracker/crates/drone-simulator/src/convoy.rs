//! Convoy-level simulation orchestrating multiple drones.

use crate::engagement::{EngagementSimulator, SimulatedEngagement};
use crate::flight::{FlightPathGenerator, Waypoint};
use crate::telemetry::{TelemetryGenerator, TelemetrySnapshot};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Simulated drone in convoy.
pub struct SimulatedDrone {
    pub drone_id: Uuid,
    pub callsign: String,
    pub platform_type: String,
    /// Formation slot (0 = lead). Each successive slot flies the same route a
    /// fixed fraction of the mission BEHIND the one ahead — line astern — so
    /// four drones on one track are four visibly distinct positions.
    pub slot: usize,
    pub waypoints: Vec<Waypoint>,
    pub telemetry_gen: TelemetryGenerator,
    pub engagement_sim: EngagementSimulator,
    pub total_engagements: u32,
    pub successful_hits: u32,
}

impl SimulatedDrone {
    /// Create a new simulated drone flying `theater`'s published route.
    ///
    /// `slot` is the drone's index in the formation; it sets a small lateral
    /// offset (metres, perpendicular to the leg) so four drones on one route
    /// read as a formation instead of a single stacked pixel.
    pub fn new(callsign: &str, platform_type: &str, theater: &drone_domain::Theater, slot: usize) -> Self {
        // Deterministic id: UUIDv5 of the callsign. A simulator restart
        // re-derives the SAME id per callsign, so its upserts OVERWRITE the
        // previous run's rows (drones, waypoints, telemetry buckets) instead
        // of accumulating ghost drones with fresh random ids — which is
        // exactly what a random v4 here did across restarts.
        let drone_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, callsign.as_bytes());
        let mut flight_gen = FlightPathGenerator::for_theater(theater);
        // Lateral offset alternates sides of the track and scales with the
        // theater: ~0.6% of the AOR radius per step (Kandahar 150 km -> 900 m,
        // Syria 160 km -> 960 m, Libya 200 km -> 1.2 km) so the formation is
        // legible at each theater's zoom, and each card's GPS is visibly its
        // own. Sub-pixel spread was the "only one drone in Syria" report.
        let step_m = theater.aor_radius_m * 0.006;
        let spread_m = ((slot as f64 + 1.0) / 2.0).floor() * step_m * if slot % 2 == 1 { 1.0 } else { -1.0 };
        let waypoints = flight_gen.generate_route_path(theater.route, spread_m);
        let telemetry_gen = TelemetryGenerator::new(drone_id, callsign, waypoints.clone());

        Self {
            drone_id,
            callsign: callsign.to_string(),
            platform_type: platform_type.to_string(),
            slot,
            waypoints,
            telemetry_gen,
            engagement_sim: EngagementSimulator::new(),
            total_engagements: 0,
            successful_hits: 0,
        }
    }

    /// Get current accuracy percentage.
    pub fn accuracy_pct(&self) -> f32 {
        if self.total_engagements == 0 {
            0.0
        } else {
            (self.successful_hits as f32 / self.total_engagements as f32) * 100.0
        }
    }
}

/// Convoy simulation state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvoyState {
    pub convoy_id: Uuid,
    pub callsign: String,
    pub mission_type: String,
    pub status: ConvoyStatus,
    pub start_time: DateTime<Utc>,
    pub drone_count: usize,
    pub progress_pct: f32,
}

/// Convoy operational status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConvoyStatus {
    Planning,
    Active,
    Rtb,
    Complete,
    Abort,
}

/// Convoy simulator managing multiple drones.
pub struct ConvoySimulator {
    pub convoy_id: Uuid,
    /// The theater whose route this convoy flies.
    pub theater: drone_domain::TheaterId,
    pub callsign: String,
    pub mission_type: String,
    pub drones: HashMap<Uuid, SimulatedDrone>,
    pub status: ConvoyStatus,
    pub start_time: DateTime<Utc>,
    mission_progress: f64,
}

impl ConvoySimulator {
    /// Create a new convoy simulation.
    /// Well-known demo convoy. The API's `activeConvoys` resolver currently
    /// returns this same id, so the dashboard, the simulator's writes and the
    /// leaderboard reads all agree on one convoy. A random id here means the
    /// simulator writes to a convoy the UI never asks about, and every panel
    /// stays empty while both processes look healthy.
    ///
    /// Override with DRONE_CONVOY_ID once the convoy repository is wired up.
    pub const DEMO_CONVOY_ID: &'static str = "550e8400-e29b-41d4-a716-446655440000";

    /// The convoy id this process flies: DRONE_CONVOY_ID if set and valid,
    /// else the well-known demo id. Factored out so the service can read the
    /// convoy's tasking order BEFORE constructing a convoy for it.
    pub fn resolve_convoy_id() -> Uuid {
        std::env::var("DRONE_CONVOY_ID")
            .ok()
            .and_then(|raw| Uuid::parse_str(&raw).ok())
            .or_else(|| Uuid::parse_str(Self::DEMO_CONVOY_ID).ok())
            .unwrap_or_else(Uuid::new_v4)
    }

    pub fn new(callsign: &str, mission_type: &str, drone_count: usize, theater: drone_domain::TheaterId) -> Self {
        let convoy_id = Self::resolve_convoy_id();
        let mut drones = HashMap::new();

        // Generate drones with military callsigns
        let platforms = ["MQ9_REAPER", "MQ1C_GRAY_EAGLE", "RQ4_GLOBAL_HAWK"];
        for i in 0..drone_count {
            let drone_callsign = format!("{}-{:02}", callsign, i + 1);
            let platform = platforms[i % platforms.len()];
            let drone = SimulatedDrone::new(&drone_callsign, platform, theater.theater(), i);
            drones.insert(drone.drone_id, drone);
        }

        Self {
            convoy_id,
            callsign: callsign.to_string(),
            mission_type: mission_type.to_string(),
            theater,
            drones,
            status: ConvoyStatus::Active,
            start_time: Utc::now(),
            mission_progress: 0.0,
        }
    }

    /// Advance mission progress.
    pub fn advance(&mut self, delta_progress: f64) {
        self.mission_progress = (self.mission_progress + delta_progress).min(1.0);

        if self.mission_progress >= 0.9 {
            self.status = ConvoyStatus::Rtb;
        }
        if self.mission_progress >= 1.0 {
            self.status = ConvoyStatus::Complete;
        }
    }

    /// Get current convoy state.
    pub fn state(&self) -> ConvoyState {
        ConvoyState {
            convoy_id: self.convoy_id,
            callsign: self.callsign.clone(),
            mission_type: self.mission_type.clone(),
            status: self.status,
            start_time: self.start_time,
            drone_count: self.drones.len(),
            progress_pct: (self.mission_progress * 100.0) as f32,
        }
    }

    /// Along-track spacing between formation slots, as a fraction of the
    /// mission. 2% ≈ 6 s at the default 300 s sortie, ≈ one-quarter of a leg
    /// on the 12-14 point routes: line astern, never overlapping, and the
    /// whole formation still completes the sortie (the trail drone is
    /// clamped to the final waypoint for its last few seconds).
    pub const FORMATION_STAGGER: f64 = 0.02;

    /// Generate telemetry for all drones — each slot trails the lead by
    /// `slot × FORMATION_STAGGER` of the mission along the SAME route.
    pub fn generate_telemetry(&mut self) -> Vec<TelemetrySnapshot> {
        let progress = self.mission_progress;
        self.drones
            .values_mut()
            .filter_map(|drone| {
                let p = (progress - drone.slot as f64 * Self::FORMATION_STAGGER).clamp(0.0, 1.0);
                drone.telemetry_gen.next_snapshot(p)
            })
            .collect()
    }

    /// Simulate engagements for drones in target area.
    pub fn simulate_engagements(&mut self) -> Vec<SimulatedEngagement> {
        // Only simulate engagements in middle phase of mission
        if self.mission_progress < 0.25 || self.mission_progress > 0.75 {
            return vec![];
        }

        let convoy_id = self.convoy_id;
        let mut engagements = Vec::new();

        for drone in self.drones.values_mut() {
            // Random chance of engagement per tick
            if rand::random::<f32>() > 0.3 {
                continue;
            }

            let altitude = drone.waypoints
                .get(drone.telemetry_gen.current_waypoint())
                .map(|wp| wp.coordinates.altitude_m)
                .unwrap_or(5000.0);

            let engagement = drone.engagement_sim.simulate_engagement(
                convoy_id,
                drone.drone_id,
                &drone.callsign,
                altitude,
            );

            drone.total_engagements += 1;
            if engagement.hit {
                drone.successful_hits += 1;
            }

            engagements.push(engagement);
        }

        engagements
    }

    /// Get leaderboard sorted by accuracy.
    pub fn leaderboard(&self) -> Vec<LeaderboardEntry> {
        let mut entries: Vec<_> = self.drones.values()
            .map(|d| LeaderboardEntry {
                drone_id: d.drone_id,
                callsign: d.callsign.clone(),
                platform_type: d.platform_type.clone(),
                accuracy_pct: d.accuracy_pct(),
                total_engagements: d.total_engagements,
                successful_hits: d.successful_hits,
                rank: 0, // Will be set after sorting
            })
            .collect();

        // total_cmp is a total order over f64, so a NaN accuracy sorts last
        // instead of panicking mid-sort.
        entries.sort_by(|a, b| b.accuracy_pct.total_cmp(&a.accuracy_pct));

        for (i, entry) in entries.iter_mut().enumerate() {
            entry.rank = i as u32 + 1;
        }

        entries
    }
}

/// Leaderboard entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub drone_id: Uuid,
    pub callsign: String,
    pub platform_type: String,
    pub accuracy_pct: f32,
    pub total_engagements: u32,
    pub successful_hits: u32,
    #[serde(default)]
    pub rank: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_convoy() {
        let convoy = ConvoySimulator::new("ALPHA", "STRIKE", 4);
        assert_eq!(convoy.drones.len(), 4);
        assert_eq!(convoy.status, ConvoyStatus::Active);
    }

    #[test]
    fn test_advance_mission() {
        let mut convoy = ConvoySimulator::new("BRAVO", "ISR", 2);
        convoy.advance(0.5);
        assert!((convoy.state().progress_pct - 50.0).abs() < 0.1);

        convoy.advance(0.5);
        assert_eq!(convoy.status, ConvoyStatus::Complete);
    }

    #[test]
    fn test_generate_telemetry() {
        let mut convoy = ConvoySimulator::new("CHARLIE", "STRIKE", 3);
        let telemetry = convoy.generate_telemetry();
        assert_eq!(telemetry.len(), 3);
    }
}
