//! # Drone Convoy Tracking System - Domain Model
//!
//! Core domain entities, value objects, and enums for military drone
//! convoy operations. These types are the single source of truth across
//! all layers: persistence, API, and frontend.
//!
//! ## Classification: UNCLASSIFIED // FOR OFFICIAL USE ONLY

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// =============================================================================
// VALUE OBJECTS
// =============================================================================

/// Geographic coordinates with full flight vector data
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Coordinates {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_m: f64,
    pub heading_deg: f32,
    pub speed_mps: f32,
}

impl Coordinates {
    pub fn new(lat: f64, lon: f64, alt: f64) -> Self {
        Self {
            latitude: lat,
            longitude: lon,
            altitude_m: alt,
            heading_deg: 0.0,
            speed_mps: 0.0,
        }
    }

    /// Calculate great-circle distance to another point (Haversine formula)
    #[must_use]
    pub fn distance_to_km(&self, other: &Coordinates) -> f64 {
        const EARTH_RADIUS_KM: f64 = 6371.0;

        let lat1 = self.latitude.to_radians();
        let lat2 = other.latitude.to_radians();
        let delta_lat = (other.latitude - self.latitude).to_radians();
        let delta_lon = (other.longitude - self.longitude).to_radians();

        let a = (delta_lat / 2.0).sin().powi(2)
            + lat1.cos() * lat2.cos() * (delta_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().asin();

        EARTH_RADIUS_KM * c
    }
}

impl Default for Coordinates {
    fn default() -> Self {
        // Default to Kabul, Afghanistan
        Self {
            latitude: 34.5553,
            longitude: 69.2075,
            altitude_m: 1800.0,
            heading_deg: 0.0,
            speed_mps: 0.0,
        }
    }
}

// =============================================================================
// ENUMS
// =============================================================================

/// Drone platform types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlatformType {
    Mq9Reaper,
    Mq1cGrayEagle,
    Rq4GlobalHawk,
    Mq25Stingray,
}

impl PlatformType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Mq9Reaper => "MQ-9_REAPER",
            Self::Mq1cGrayEagle => "MQ-1C_GRAY_EAGLE",
            Self::Rq4GlobalHawk => "RQ-4_GLOBAL_HAWK",
            Self::Mq25Stingray => "MQ-25_STINGRAY",
        }
    }
}

/// Drone operational status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DroneStatus {
    Preflight,
    Airborne,
    Loiter,
    Ingress,
    Egress,
    Rtb, // Return to Base
    Landed,
    Maintenance,
}

/// Convoy mission status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConvoyStatus {
    Planning,
    Active,
    Rtb,
    Complete,
    Abort,
}

/// Mission types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MissionType {
    Isr, // Intelligence, Surveillance, Reconnaissance
    Strike,
    Escort,
    Resupply,
    Sar, // Search and Rescue
}

/// Waypoint types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WaypointType {
    Nav,
    Loiter,
    Strike,
    Refuel,
    Rendezvous,
    Checkpoint,
}

/// Waypoint status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WaypointStatus {
    Pending,
    Active,
    Complete,
    Skipped,
}

/// Weapon types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WeaponType {
    Agm114Hellfire,
    Gbu12Paveway,
    Aim9xSidewinder,
    Gbu38Jdam,
    Agm176Griffin,
}

impl WeaponType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Agm114Hellfire => "AGM-114_HELLFIRE",
            Self::Gbu12Paveway => "GBU-12_PAVEWAY",
            Self::Aim9xSidewinder => "AIM-9X_SIDEWINDER",
            Self::Gbu38Jdam => "GBU-38_JDAM",
            Self::Agm176Griffin => "AGM-176_GRIFFIN",
        }
    }
}

/// Weapon status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WeaponState {
    Armed,
    Safe,
    Jammed,
    Expended,
}

/// Target types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TargetType {
    Vehicle,
    Structure,
    Personnel,
    Radar,
    AirDefense,
    Supply,
}

/// Threat level classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThreatLevel {
    High,
    Medium,
    Low,
    Unknown,
}

/// Battle damage assessment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DamageAssessment {
    Destroyed,
    Damaged,
    Missed,
    PendingBda,
}

/// Collateral risk level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CollateralRisk {
    None,
    Minimal,
    Moderate,
    High,
}

/// Sensor types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SensorType {
    EoIr, // Electro-Optical / Infrared
    Sar,  // Synthetic Aperture Radar
    Sigint,
    Lidar,
}

/// Communication link types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LinkType {
    Satcom,
    Los, // Line of Sight
    Mesh,
    Backup,
}

/// Alert severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AlertSeverity {
    Critical,
    Warning,
    Info,
}

// =============================================================================
// NESTED VALUE OBJECTS
// =============================================================================

/// Weapon system status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeaponStatus {
    pub weapon_type: WeaponType,
    pub rounds_remaining: i16,
    pub status: WeaponState,
}

/// Target information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetInfo {
    pub target_id: Uuid,
    pub target_type: TargetType,
    pub coordinates: Coordinates,
    pub confidence: f32,
    pub threat_level: ThreatLevel,
}

/// Engagement result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngagementResult {
    pub impact_time: DateTime<Utc>,
    pub impact_coords: Coordinates,
    pub damage_assessment: DamageAssessment,
    pub collateral_risk: CollateralRisk,
}

/// Sensor payload status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensorStatus {
    pub sensor_type: SensorType,
    pub operational: bool,
    pub mode: String,
}

/// Communication link status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommLink {
    pub link_type: LinkType,
    pub signal_strength_dbm: f32,
    pub latency_ms: i32,
    pub encryption: String,
}

// =============================================================================
// ENTITY TYPES
// =============================================================================

/// Convoy entity - mission-level grouping of drones
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Convoy {
    pub convoy_id: Uuid,
    pub convoy_callsign: String,
    pub mission_id: Uuid,
    pub mission_type: MissionType,
    pub status: ConvoyStatus,

    // Temporal
    pub created_at: DateTime<Utc>,
    pub mission_start: Option<DateTime<Utc>>,
    pub mission_end: Option<DateTime<Utc>>,

    // AOR (Area of Responsibility)
    pub aor_name: String,
    pub aor_center: Coordinates,
    pub aor_radius_km: f32,

    // Command
    pub commanding_unit: String,
    pub authorization_level: String,
    pub roe_profile: String,

    // Drone roster
    pub drone_ids: Vec<Uuid>,
    pub drone_count: i16,
}

/// Drone entity - individual drone platform
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Drone {
    pub convoy_id: Uuid,
    pub drone_id: Uuid,

    // Platform identification
    pub tail_number: String,
    pub callsign: String,
    pub platform_type: PlatformType,
    pub serial_number: String,

    // Current state
    pub status: DroneStatus,
    pub current_position: Coordinates,
    pub fuel_remaining_pct: f32,
    pub flight_time_hrs: f32,

    // Loadout
    pub weapons: Vec<WeaponStatus>,
    pub sensors: Vec<SensorStatus>,

    // Communications
    pub primary_link: Option<CommLink>,
    pub backup_link: Option<CommLink>,
    pub mesh_neighbors: Vec<Uuid>,

    // Performance metrics
    pub total_engagements: i32,
    pub successful_hits: i32,
    pub accuracy_pct: f32,

    // Metadata
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Drone {
    /// Recalculate accuracy percentage
    pub fn calculate_accuracy(&mut self) {
        if self.total_engagements > 0 {
            self.accuracy_pct =
                (self.successful_hits as f32 / self.total_engagements as f32) * 100.0;
        } else {
            self.accuracy_pct = 0.0;
        }
    }
}

/// Waypoint entity - route waypoint
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Waypoint {
    pub drone_id: Uuid,
    pub sequence_number: i16,

    pub waypoint_id: Uuid,
    pub waypoint_name: String,
    pub waypoint_type: WaypointType,

    pub coordinates: Coordinates,

    pub planned_arrival: Option<DateTime<Utc>>,
    pub actual_arrival: Option<DateTime<Utc>>,
    pub planned_departure: Option<DateTime<Utc>>,
    pub actual_departure: Option<DateTime<Utc>>,

    pub loiter_duration_min: Option<i32>,
    pub authorized_actions: Vec<String>,

    pub status: WaypointStatus,
}

/// Telemetry entity - time-series position/sensor data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Telemetry {
    pub drone_id: Uuid,
    pub time_bucket: String,
    pub recorded_at: DateTime<Utc>,

    // Position & movement
    pub position: Coordinates,
    pub velocity_mps: f32,
    pub acceleration_mps2: f32,
    pub bank_angle_deg: f32,
    pub pitch_angle_deg: f32,

    // Waypoint context
    pub current_waypoint: i16,
    pub distance_to_next_km: f32,
    pub eta_next_waypoint: Option<DateTime<Utc>>,

    // Systems status
    pub fuel_remaining_pct: f32,
    pub engine_rpm: i32,
    pub engine_temp_c: f32,
    pub battery_voltage: f32,

    // Environment
    pub wind_speed_mps: f32,
    pub wind_direction_deg: f32,
    pub temperature_c: f32,
    pub visibility_km: f32,

    // Comms health
    pub link_status: Option<CommLink>,
    pub mesh_connectivity: f32,
}

impl Telemetry {
    /// Generate time bucket string from timestamp (hourly buckets)
    #[must_use]
    pub fn generate_time_bucket(ts: &DateTime<Utc>) -> String {
        ts.format("%Y%m%d%H").to_string()
    }
}

/// Engagement entity - weapon employment record
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Engagement {
    pub convoy_id: Uuid,
    pub engaged_at: DateTime<Utc>,
    pub engagement_id: Uuid,

    // Shooter
    pub drone_id: Uuid,
    pub drone_callsign: String,

    // Weapon
    pub weapon_type: WeaponType,
    pub weapon_serial: String,

    // Target
    pub target: TargetInfo,

    // Authorization
    pub authorization_code: String,
    pub authorized_by: String,
    pub roe_compliance: bool,

    // Result
    pub result: EngagementResult,
    pub hit: bool,

    // Context
    pub waypoint_number: i16,
    pub shooter_position: Coordinates,
    pub range_to_target_km: f32,

    // BDA
    pub bda_status: String,
    pub bda_notes: Option<String>,
}

// =============================================================================
// LEADERBOARD TYPES
// =============================================================================

/// Leaderboard entry - pre-computed for fast queries
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub convoy_id: Uuid,
    pub drone_id: Uuid,
    pub callsign: String,
    pub platform_type: PlatformType,
    pub accuracy_pct: f32,
    pub total_engagements: i32,
    pub successful_hits: i32,
    pub current_streak: i32,
    pub best_streak: i32,
    pub rank: i16,
    pub updated_at: DateTime<Utc>,
}

/// Accuracy statistics
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AccuracyStats {
    pub total_engagements: i64,
    pub successful_hits: i64,
    pub current_streak: i32,
    pub best_streak: i32,
}

impl AccuracyStats {
    #[must_use]
    pub fn accuracy_pct(&self) -> f32 {
        if self.total_engagements > 0 {
            (self.successful_hits as f32 / self.total_engagements as f32) * 100.0
        } else {
            0.0
        }
    }
}

// =============================================================================
// QUERY/FILTER TYPES
// =============================================================================

/// Time range filter for queries
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// Pagination parameters
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pagination {
    pub limit: i32,
    pub offset: i32,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            limit: 50,
            offset: 0,
        }
    }
}

// =============================================================================
// ERRORS
// =============================================================================

/// Domain-level errors
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("Entity not found: {entity_type} with id {id}")]
    NotFound { entity_type: String, id: String },

    #[error("Invalid coordinates: lat={lat}, lon={lon}")]
    InvalidCoordinates { lat: f64, lon: f64 },

    #[error("Invalid waypoint sequence: {0}")]
    InvalidWaypointSequence(String),

    #[error("Engagement validation failed: {0}")]
    EngagementValidation(String),
}
