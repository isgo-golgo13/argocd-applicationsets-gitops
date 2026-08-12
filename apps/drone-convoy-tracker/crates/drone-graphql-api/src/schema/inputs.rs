//! # GraphQL Input Types
//!
//! Input object definitions for mutations and queries.

use async_graphql::InputObject;
use chrono::{DateTime, Utc};

use super::enums::*;

// =============================================================================
// COORDINATE INPUTS
// =============================================================================

/// Geographic coordinates input
#[derive(Debug, Clone, InputObject)]
pub struct CoordinatesInput {
    /// Latitude in decimal degrees (-90 to 90)
    pub latitude: f64,
    /// Longitude in decimal degrees (-180 to 180)
    pub longitude: f64,
    /// Altitude in meters above sea level
    #[graphql(default)]
    pub altitude_m: f64,
    /// Heading in degrees (0-360)
    #[graphql(default)]
    pub heading_deg: f64,
    /// Speed in meters per second
    #[graphql(default)]
    pub speed_mps: f64,
}

// =============================================================================
// ENGAGEMENT INPUTS
// =============================================================================

/// Input for recording a hit/miss engagement
#[derive(Debug, Clone, InputObject)]
pub struct RecordEngagementInput {
    /// Convoy ID
    pub convoy_id: String,
    /// Drone ID that performed the engagement
    pub drone_id: String,
    /// Whether the engagement was a hit
    pub hit: bool,
    /// Optional weapon type used
    pub weapon_type: Option<WeaponType>,
    /// Optional target type
    pub target_type: Option<TargetType>,
    /// Optional range to target in kilometers
    pub range_km: Option<f64>,
}

/// Input for creating a full engagement record
#[derive(Debug, Clone, InputObject)]
pub struct CreateEngagementInput {
    /// Convoy ID
    pub convoy_id: String,
    /// Drone ID that performed the engagement
    pub drone_id: String,
    /// Weapon type used
    pub weapon_type: WeaponType,
    /// Target information
    pub target: TargetInput,
    /// Whether the engagement was a hit
    pub hit: bool,
    /// Shooter position at time of engagement
    pub shooter_position: CoordinatesInput,
    /// Authorization code for the engagement
    pub authorization_code: String,
    /// ROE compliance flag
    #[graphql(default = true)]
    pub roe_compliance: bool,
}

/// Target information input
#[derive(Debug, Clone, InputObject)]
pub struct TargetInput {
    /// Target type
    pub target_type: TargetType,
    /// Target location
    pub coordinates: CoordinatesInput,
    /// Confidence level (0.0 - 1.0)
    #[graphql(default = 0.9)]
    pub confidence: f64,
    /// Threat level assessment
    #[graphql(default)]
    pub threat_level: Option<ThreatLevel>,
}

/// Input for updating BDA status
#[derive(Debug, Clone, InputObject)]
pub struct UpdateBdaInput {
    /// Convoy ID
    pub convoy_id: String,
    /// Engagement ID
    pub engagement_id: String,
    /// New damage assessment
    pub damage_assessment: DamageAssessment,
    /// BDA notes
    pub notes: Option<String>,
}

// =============================================================================
// DRONE INPUTS
// =============================================================================

/// Input for updating drone state
#[derive(Debug, Clone, InputObject)]
pub struct UpdateDroneStateInput {
    /// Convoy ID
    pub convoy_id: String,
    /// Drone ID
    pub drone_id: String,
    /// New status
    pub status: Option<DroneStatus>,
    /// Current position
    pub position: Option<CoordinatesInput>,
    /// Fuel remaining percentage
    pub fuel_pct: Option<f64>,
    /// Current waypoint number
    pub current_waypoint: Option<i32>,
}

/// Input for creating telemetry record
#[derive(Debug, Clone, InputObject)]
pub struct CreateTelemetryInput {
    /// Drone ID
    pub drone_id: String,
    /// Position data
    pub position: CoordinatesInput,
    /// Fuel remaining percentage
    pub fuel_pct: f64,
    /// Current waypoint number
    pub current_waypoint: i32,
    /// Velocity in m/s
    #[graphql(default)]
    pub velocity_mps: f64,
    /// Mesh connectivity (0.0 - 1.0)
    #[graphql(default = 1.0)]
    pub mesh_connectivity: f64,
}

// =============================================================================
// CONVOY INPUTS
// =============================================================================

/// Input for creating a new convoy
#[derive(Debug, Clone, InputObject)]
pub struct CreateConvoyInput {
    /// Convoy callsign
    pub callsign: String,
    /// Mission type
    pub mission_type: MissionType,
    /// Area of responsibility name
    pub aor_name: String,
    /// AOR center coordinates
    pub aor_center: CoordinatesInput,
    /// AOR radius in kilometers
    pub aor_radius_km: f64,
    /// Commanding unit
    pub commanding_unit: String,
    /// ROE profile name
    pub roe_profile: String,
}

/// Input for updating convoy status
#[derive(Debug, Clone, InputObject)]
pub struct UpdateConvoyStatusInput {
    /// Convoy ID
    pub convoy_id: String,
    /// New status
    pub status: ConvoyStatus,
}

// =============================================================================
// WAYPOINT INPUTS
// =============================================================================

/// Input for creating a waypoint
#[derive(Debug, Clone, InputObject)]
pub struct CreateWaypointInput {
    /// Drone ID
    pub drone_id: String,
    /// Sequence number (1-25)
    pub sequence_number: i32,
    /// Waypoint name
    pub name: String,
    /// Waypoint type
    pub waypoint_type: WaypointType,
    /// Coordinates
    pub coordinates: CoordinatesInput,
    /// Planned arrival time
    pub planned_arrival: Option<DateTime<Utc>>,
    /// Loiter duration in minutes (for LOITER type)
    pub loiter_duration_min: Option<i32>,
}

/// Input for batch creating waypoints
#[derive(Debug, Clone, InputObject)]
pub struct CreateWaypointsInput {
    /// Drone ID
    pub drone_id: String,
    /// List of waypoints
    pub waypoints: Vec<WaypointDefinition>,
}

/// Single waypoint definition for batch creation
#[derive(Debug, Clone, InputObject)]
pub struct WaypointDefinition {
    /// Sequence number
    pub sequence_number: i32,
    /// Waypoint name
    pub name: String,
    /// Waypoint type
    pub waypoint_type: WaypointType,
    /// Coordinates
    pub coordinates: CoordinatesInput,
}

// =============================================================================
// QUERY FILTER INPUTS
// =============================================================================

/// Time range filter
#[derive(Debug, Clone, InputObject)]
pub struct TimeRangeInput {
    /// Start time (inclusive)
    pub start: DateTime<Utc>,
    /// End time (inclusive)
    pub end: DateTime<Utc>,
}

/// Pagination input
#[derive(Debug, Clone, InputObject)]
pub struct PaginationInput {
    /// Maximum results to return
    #[graphql(default = 20)]
    pub limit: i32,
    /// Number of results to skip
    #[graphql(default = 0)]
    pub offset: i32,
}

impl Default for PaginationInput {
    fn default() -> Self {
        Self {
            limit: 20,
            offset: 0,
        }
    }
}

/// Leaderboard query filter
#[derive(Debug, Clone, InputObject, Default)]
pub struct LeaderboardFilter {
    /// Minimum accuracy percentage
    pub min_accuracy: Option<f64>,
    /// Minimum engagements
    pub min_engagements: Option<i32>,
    /// Filter by platform type
    pub platform_type: Option<PlatformType>,
}

/// Engagement query filter
#[derive(Debug, Clone, InputObject, Default)]
pub struct EngagementFilter {
    /// Filter by hit/miss
    pub hit: Option<bool>,
    /// Filter by weapon type
    pub weapon_type: Option<WeaponType>,
    /// Filter by time range
    pub time_range: Option<TimeRangeInput>,
    /// Filter by damage assessment
    pub damage_assessment: Option<DamageAssessment>,
}

/// Drone query filter
#[derive(Debug, Clone, InputObject, Default)]
pub struct DroneFilter {
    /// Filter by status
    pub status: Option<DroneStatus>,
    /// Filter by platform type
    pub platform_type: Option<PlatformType>,
    /// Minimum fuel percentage
    pub min_fuel_pct: Option<f64>,
}
