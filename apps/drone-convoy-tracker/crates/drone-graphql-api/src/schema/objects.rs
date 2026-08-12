//! # GraphQL Output Types
//!
//! Object type definitions for GraphQL responses.

use async_graphql::{ComplexObject, Context, Object, SimpleObject, ID};
use chrono::{DateTime, Utc};

use super::enums::*;
use crate::error::ApiResult;
use drone_domain as domain;

// =============================================================================
// VALUE OBJECTS
// =============================================================================

/// Geographic coordinates with flight vector
#[derive(Debug, Clone, SimpleObject)]
pub struct Coordinates {
    /// Latitude in decimal degrees
    pub latitude: f64,
    /// Longitude in decimal degrees
    pub longitude: f64,
    /// Altitude in meters above sea level
    pub altitude_m: f64,
    /// Heading in degrees (0-360, 0 = North)
    pub heading_deg: f32,
    /// Speed in meters per second
    pub speed_mps: f32,
}

impl From<domain::Coordinates> for Coordinates {
    fn from(c: domain::Coordinates) -> Self {
        Self {
            latitude: c.latitude,
            longitude: c.longitude,
            altitude_m: c.altitude_m,
            heading_deg: c.heading_deg,
            speed_mps: c.speed_mps,
        }
    }
}

// =============================================================================
// LEADERBOARD TYPES
// =============================================================================

/// Individual entry in the drone accuracy leaderboard
#[derive(Debug, Clone)]
pub struct LeaderboardEntry {
    pub convoy_id: String,
    pub drone_id: String,
    pub callsign: String,
    pub platform_type: PlatformType,
    pub rank: i32,
    pub accuracy_pct: f32,
    pub total_engagements: i32,
    pub successful_hits: i32,
    pub current_streak: i32,
    pub best_streak: i32,
    pub updated_at: DateTime<Utc>,
}

#[Object]
impl LeaderboardEntry {
    /// Unique drone identifier
    async fn drone_id(&self) -> ID {
        ID(self.drone_id.clone())
    }

    /// Drone callsign
    async fn callsign(&self) -> &str {
        &self.callsign
    }

    /// Platform type
    async fn platform_type(&self) -> PlatformType {
        self.platform_type
    }

    /// Current rank in leaderboard (1-indexed)
    async fn rank(&self) -> i32 {
        self.rank
    }

    /// Accuracy percentage (0-100)
    async fn accuracy_pct(&self) -> f32 {
        self.accuracy_pct
    }

    /// Total engagement attempts
    async fn total_engagements(&self) -> i32 {
        self.total_engagements
    }

    /// Successful hits
    async fn successful_hits(&self) -> i32 {
        self.successful_hits
    }

    /// Number of misses
    async fn misses(&self) -> i32 {
        self.total_engagements - self.successful_hits
    }

    /// Hit rate as decimal (0.0 - 1.0)
    async fn hit_rate(&self) -> f32 {
        if self.total_engagements > 0 {
            self.successful_hits as f32 / self.total_engagements as f32
        } else {
            0.0
        }
    }

    /// Current consecutive hit streak
    async fn current_streak(&self) -> i32 {
        self.current_streak
    }

    /// Best ever consecutive hit streak
    async fn best_streak(&self) -> i32 {
        self.best_streak
    }

    /// Last update timestamp
    async fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }
}

impl From<domain::LeaderboardEntry> for LeaderboardEntry {
    fn from(e: domain::LeaderboardEntry) -> Self {
        Self {
            convoy_id: e.convoy_id.to_string(),
            drone_id: e.drone_id.to_string(),
            callsign: e.callsign,
            platform_type: e.platform_type.into(),
            rank: e.rank as i32,
            accuracy_pct: e.accuracy_pct,
            total_engagements: e.total_engagements,
            successful_hits: e.successful_hits,
            current_streak: e.current_streak,
            best_streak: e.best_streak,
            updated_at: e.updated_at,
        }
    }
}

/// Full leaderboard for a convoy
#[derive(Debug, Clone)]
pub struct Leaderboard {
    pub convoy_id: String,
    pub convoy_callsign: String,
    pub entries: Vec<LeaderboardEntry>,
    pub generated_at: DateTime<Utc>,
}

#[Object]
impl Leaderboard {
    /// Convoy ID
    async fn convoy_id(&self) -> ID {
        ID(self.convoy_id.clone())
    }

    /// Convoy callsign
    async fn convoy_callsign(&self) -> &str {
        &self.convoy_callsign
    }

    /// Leaderboard entries sorted by rank
    async fn entries(&self) -> &[LeaderboardEntry] {
        &self.entries
    }

    /// Total drones in leaderboard
    async fn total_drones(&self) -> usize {
        self.entries.len()
    }

    /// Average accuracy across all drones
    async fn average_accuracy(&self) -> f32 {
        if self.entries.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.entries.iter().map(|e| e.accuracy_pct).sum();
        sum / self.entries.len() as f32
    }

    /// Top performer (rank 1)
    async fn leader(&self) -> Option<&LeaderboardEntry> {
        self.entries.first()
    }

    /// Total engagements across all drones
    async fn total_engagements(&self) -> i32 {
        self.entries.iter().map(|e| e.total_engagements).sum()
    }

    /// Total hits across all drones
    async fn total_hits(&self) -> i32 {
        self.entries.iter().map(|e| e.successful_hits).sum()
    }

    /// Timestamp when leaderboard was generated
    async fn generated_at(&self) -> DateTime<Utc> {
        self.generated_at
    }
}

// =============================================================================
// DRONE TYPES
// =============================================================================

/// Drone platform details
#[derive(Debug, Clone)]
pub struct Drone {
    pub drone_id: String,
    pub convoy_id: String,
    pub tail_number: String,
    pub callsign: String,
    pub platform_type: PlatformType,
    pub status: DroneStatus,
    pub current_position: Coordinates,
    pub fuel_remaining_pct: f32,
    pub accuracy_pct: f32,
    pub total_engagements: i32,
    pub successful_hits: i32,
    pub current_waypoint: i32,
    pub total_waypoints: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[Object]
impl Drone {
    /// Unique drone identifier
    async fn drone_id(&self) -> ID {
        ID(self.drone_id.clone())
    }

    /// Parent convoy ID
    async fn convoy_id(&self) -> ID {
        ID(self.convoy_id.clone())
    }

    /// Military tail number
    async fn tail_number(&self) -> &str {
        &self.tail_number
    }

    /// Radio callsign
    async fn callsign(&self) -> &str {
        &self.callsign
    }

    /// Platform type
    async fn platform_type(&self) -> PlatformType {
        self.platform_type
    }

    /// Current operational status
    async fn status(&self) -> DroneStatus {
        self.status
    }

    /// Current geographic position
    async fn current_position(&self) -> &Coordinates {
        &self.current_position
    }

    /// Fuel remaining as percentage (0-100)
    async fn fuel_remaining_pct(&self) -> f32 {
        self.fuel_remaining_pct
    }

    /// Is fuel critical (below 20%)
    async fn fuel_critical(&self) -> bool {
        self.fuel_remaining_pct < 20.0
    }

    /// Accuracy percentage
    async fn accuracy_pct(&self) -> f32 {
        self.accuracy_pct
    }

    /// Total engagements
    async fn total_engagements(&self) -> i32 {
        self.total_engagements
    }

    /// Successful hits
    async fn successful_hits(&self) -> i32 {
        self.successful_hits
    }

    /// Current waypoint (1-indexed)
    async fn current_waypoint(&self) -> i32 {
        self.current_waypoint
    }

    /// Total waypoints in mission
    async fn total_waypoints(&self) -> i32 {
        self.total_waypoints
    }

    /// Mission progress percentage
    async fn mission_progress_pct(&self) -> f32 {
        if self.total_waypoints > 0 {
            (self.current_waypoint as f32 / self.total_waypoints as f32) * 100.0
        } else {
            0.0
        }
    }

    /// Is drone currently airborne
    async fn is_airborne(&self) -> bool {
        matches!(
            self.status,
            DroneStatus::Airborne
                | DroneStatus::Loiter
                | DroneStatus::Ingress
                | DroneStatus::Egress
        )
    }

    /// Creation timestamp
    async fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Last update timestamp
    async fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }
}

// =============================================================================
// CONVOY TYPES
// =============================================================================

/// Convoy/mission summary
#[derive(Debug, Clone, SimpleObject)]
#[graphql(complex)]
pub struct Convoy {
    /// Convoy ID
    pub convoy_id: ID,
    /// Callsign
    pub callsign: String,
    /// Mission type
    pub mission_type: MissionType,
    /// Current status
    pub status: ConvoyStatus,
    /// Area of responsibility name
    pub aor_name: String,
    /// AOR center point
    pub aor_center: Coordinates,
    /// AOR radius in km
    pub aor_radius_km: f32,
    /// Total drones assigned
    pub drone_count: i32,
    /// Commanding unit
    pub commanding_unit: String,
    /// Mission start time
    pub mission_start: Option<DateTime<Utc>>,
    /// Mission end time
    pub mission_end: Option<DateTime<Utc>>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

#[ComplexObject]
impl Convoy {
    /// Is mission currently active
    async fn is_active(&self) -> bool {
        self.status == ConvoyStatus::Active
    }

    /// Mission duration in minutes (if started)
    async fn mission_duration_min(&self) -> Option<i64> {
        self.mission_start.map(|start| {
            let end = self.mission_end.unwrap_or_else(Utc::now);
            (end - start).num_minutes()
        })
    }
}

/// Convoy statistics summary
#[derive(Debug, Clone, SimpleObject)]
pub struct ConvoyStats {
    /// Convoy ID
    pub convoy_id: ID,
    /// Total drones
    pub drone_count: i32,
    /// Airborne drones
    pub airborne_count: i32,
    /// Total engagements
    pub total_engagements: i32,
    /// Total hits
    pub total_hits: i32,
    /// Average accuracy percentage
    pub average_accuracy_pct: f32,
    /// Average fuel percentage
    pub average_fuel_pct: f32,
    /// Snapshot timestamp
    pub timestamp: DateTime<Utc>,
}

// =============================================================================
// WAYPOINT TYPES
// =============================================================================

/// Route waypoint
#[derive(Debug, Clone, SimpleObject)]
#[graphql(complex)]
pub struct Waypoint {
    /// Waypoint ID
    pub waypoint_id: ID,
    /// Drone ID
    pub drone_id: ID,
    /// Sequence number (1-25)
    pub sequence_number: i32,
    /// Waypoint name
    pub name: String,
    /// Waypoint type
    pub waypoint_type: WaypointType,
    /// Coordinates
    pub coordinates: Coordinates,
    /// Status
    pub status: WaypointStatus,
    /// Planned arrival time
    pub planned_arrival: Option<DateTime<Utc>>,
    /// Actual arrival time
    pub actual_arrival: Option<DateTime<Utc>>,
    /// Planned departure time
    pub planned_departure: Option<DateTime<Utc>>,
    /// Actual departure time
    pub actual_departure: Option<DateTime<Utc>>,
    /// Loiter duration in minutes
    pub loiter_duration_min: Option<i32>,
}

#[ComplexObject]
impl Waypoint {
    /// Is waypoint completed
    async fn is_complete(&self) -> bool {
        self.status == WaypointStatus::Complete
    }

    /// Arrival delay in minutes (negative = early)
    async fn arrival_delay_min(&self) -> Option<i64> {
        match (self.planned_arrival, self.actual_arrival) {
            (Some(planned), Some(actual)) => Some((actual - planned).num_minutes()),
            _ => None,
        }
    }
}

// =============================================================================
// ENGAGEMENT TYPES
// =============================================================================

/// Weapon engagement record
#[derive(Debug, Clone, SimpleObject)]
#[graphql(complex)]
pub struct Engagement {
    /// Engagement ID
    pub engagement_id: ID,
    /// Convoy ID
    pub convoy_id: ID,
    /// Drone ID
    pub drone_id: ID,
    /// Drone callsign
    pub drone_callsign: String,
    /// Engagement timestamp
    pub engaged_at: DateTime<Utc>,
    /// Weapon type used
    pub weapon_type: WeaponType,
    /// Target type
    pub target_type: TargetType,
    /// Target coordinates
    pub target_coordinates: Coordinates,
    /// Shooter position
    pub shooter_position: Coordinates,
    /// Range to target in km
    pub range_km: f32,
    /// Was it a hit
    pub hit: bool,
    /// Damage assessment
    pub damage_assessment: DamageAssessment,
    /// Authorization code
    pub authorization_code: String,
    /// ROE compliant
    pub roe_compliant: bool,
}

#[ComplexObject]
impl Engagement {
    /// Is BDA pending
    async fn bda_pending(&self) -> bool {
        self.damage_assessment == DamageAssessment::PendingBda
    }
}

// =============================================================================
// TELEMETRY TYPES
// =============================================================================

/// Telemetry snapshot
#[derive(Debug, Clone, SimpleObject)]
pub struct TelemetrySnapshot {
    /// Drone ID
    pub drone_id: ID,
    /// Recording timestamp
    pub recorded_at: DateTime<Utc>,
    /// Position
    pub position: Coordinates,
    /// Fuel remaining percentage
    pub fuel_remaining_pct: f32,
    /// Current waypoint number
    pub current_waypoint: i32,
    /// Velocity in m/s
    pub velocity_mps: f32,
    /// Mesh connectivity (0-1)
    pub mesh_connectivity: f32,
    /// Distance to next waypoint in km
    pub distance_to_next_km: f32,
}

// =============================================================================
// SUBSCRIPTION EVENT TYPES
// =============================================================================

/// Leaderboard update event
#[derive(Debug, Clone, SimpleObject)]
pub struct LeaderboardUpdateEvent {
    /// Convoy ID
    pub convoy_id: ID,
    /// Drone ID
    pub drone_id: ID,
    /// Drone callsign
    pub callsign: String,
    /// New rank
    pub new_rank: i32,
    /// Previous rank (if applicable)
    pub old_rank: Option<i32>,
    /// New accuracy percentage
    pub accuracy_pct: f32,
    /// Type of rank change
    pub change_type: RankChangeType,
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
}

/// Engagement event for real-time updates
#[derive(Debug, Clone, SimpleObject)]
pub struct EngagementEvent {
    /// Convoy ID
    pub convoy_id: ID,
    /// Drone ID
    pub drone_id: ID,
    /// Drone callsign
    pub callsign: String,
    /// Was it a hit
    pub hit: bool,
    /// Weapon type used
    pub weapon_type: WeaponType,
    /// New accuracy after engagement
    pub new_accuracy_pct: f32,
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
}

/// Drone status change event
#[derive(Debug, Clone, SimpleObject)]
pub struct DroneStatusEvent {
    /// Convoy ID
    pub convoy_id: ID,
    /// Drone ID
    pub drone_id: ID,
    /// Drone callsign
    pub callsign: String,
    /// Old status
    pub old_status: DroneStatus,
    /// New status
    pub new_status: DroneStatus,
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
}

/// Alert event
#[derive(Debug, Clone, SimpleObject)]
pub struct AlertEvent {
    /// Alert ID
    pub alert_id: ID,
    /// Convoy ID
    pub convoy_id: ID,
    /// Source drone ID
    pub drone_id: Option<ID>,
    /// Severity
    pub severity: AlertSeverity,
    /// Alert type code
    pub alert_type: String,
    /// Human readable message
    pub message: String,
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
}

// =============================================================================
// MUTATION RESPONSE TYPES
// =============================================================================

/// Result of recording an engagement
#[derive(Debug, Clone, SimpleObject)]
pub struct RecordEngagementResult {
    /// Success flag
    pub success: bool,
    /// Updated leaderboard entry
    pub entry: LeaderboardEntry,
    /// New rank position
    pub new_rank: i32,
    /// Rank change from previous
    pub rank_change: i32,
    /// New accuracy percentage
    pub new_accuracy_pct: f32,
}

/// Result of rebuilding leaderboard
#[derive(Debug, Clone, SimpleObject)]
pub struct RebuildLeaderboardResult {
    /// Success flag
    pub success: bool,
    /// Number of entries processed
    pub entries_processed: i32,
    /// Rebuild duration in milliseconds
    pub duration_ms: i64,
}

// =============================================================================
// PAGINATED RESPONSE TYPES
// =============================================================================

/// Paginated list wrapper
#[derive(Debug, Clone, SimpleObject)]
#[graphql(concrete(name = "EngagementConnection", params(Engagement)))]
#[graphql(concrete(name = "DroneConnection", params(Drone)))]
#[graphql(concrete(name = "TelemetryConnection", params(TelemetrySnapshot)))]
pub struct Connection<T: async_graphql::OutputType> {
    /// Items in this page
    pub items: Vec<T>,
    /// Total count across all pages
    pub total_count: i32,
    /// Has more pages
    pub has_next_page: bool,
    /// Has previous pages
    pub has_previous_page: bool,
}
