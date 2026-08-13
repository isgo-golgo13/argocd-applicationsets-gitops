//! # GraphQL Enum Types
//!
//! Enum definitions for the GraphQL schema.

use async_graphql::Enum;
use drone_domain as domain;

/// Drone platform type
///
/// Wire names are pinned explicitly. async-graphql's SCREAMING_SNAKE_CASE
/// rename runs through Inflector, whose `char_is_uppercase` is
/// `c == c.to_ascii_uppercase()` — TRUE for digits — so `Mq9Reaper` would
/// serialize as `MQ_9_REAPER`, not the `MQ9_REAPER` every caller (simulator,
/// frontend match arms, stored rows) expects. Pinning removes the rename
/// engine from the contract entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Enum)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum PlatformType {
    /// MQ-9 Reaper - Primary strike/ISR platform
    #[graphql(name = "MQ9_REAPER")]
    Mq9Reaper,
    /// MQ-1C Gray Eagle - Army tactical UAS
    #[graphql(name = "MQ1C_GRAY_EAGLE")]
    Mq1cGrayEagle,
    /// RQ-4 Global Hawk - High-altitude ISR
    #[graphql(name = "RQ4_GLOBAL_HAWK")]
    Rq4GlobalHawk,
    /// MQ-25 Stingray - Carrier-based refueling
    #[graphql(name = "MQ25_STINGRAY")]
    Mq25Stingray,
}

impl From<domain::PlatformType> for PlatformType {
    fn from(p: domain::PlatformType) -> Self {
        match p {
            domain::PlatformType::Mq9Reaper => Self::Mq9Reaper,
            domain::PlatformType::Mq1cGrayEagle => Self::Mq1cGrayEagle,
            domain::PlatformType::Rq4GlobalHawk => Self::Rq4GlobalHawk,
            domain::PlatformType::Mq25Stingray => Self::Mq25Stingray,
        }
    }
}

impl From<PlatformType> for domain::PlatformType {
    fn from(p: PlatformType) -> Self {
        match p {
            PlatformType::Mq9Reaper => Self::Mq9Reaper,
            PlatformType::Mq1cGrayEagle => Self::Mq1cGrayEagle,
            PlatformType::Rq4GlobalHawk => Self::Rq4GlobalHawk,
            PlatformType::Mq25Stingray => Self::Mq25Stingray,
        }
    }
}

/// Drone operational status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Enum)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum DroneStatus {
    /// Pre-flight checks in progress
    Preflight,
    /// Airborne and operational
    Airborne,
    /// Holding pattern / surveillance orbit
    Loiter,
    /// Inbound to target area
    Ingress,
    /// Exiting target area
    Egress,
    /// Returning to base
    Rtb,
    /// On ground at base
    Landed,
    /// Undergoing maintenance
    Maintenance,
}

impl From<domain::DroneStatus> for DroneStatus {
    fn from(s: domain::DroneStatus) -> Self {
        match s {
            domain::DroneStatus::Preflight => Self::Preflight,
            domain::DroneStatus::Airborne => Self::Airborne,
            domain::DroneStatus::Loiter => Self::Loiter,
            domain::DroneStatus::Ingress => Self::Ingress,
            domain::DroneStatus::Egress => Self::Egress,
            domain::DroneStatus::Rtb => Self::Rtb,
            domain::DroneStatus::Landed => Self::Landed,
            domain::DroneStatus::Maintenance => Self::Maintenance,
        }
    }
}

/// Mission/convoy status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Enum)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum ConvoyStatus {
    /// Mission planning phase
    Planning,
    /// Mission actively executing
    Active,
    /// All assets returning to base
    Rtb,
    /// Mission completed successfully
    Complete,
    /// Mission aborted
    Abort,
}

impl From<domain::ConvoyStatus> for ConvoyStatus {
    fn from(s: domain::ConvoyStatus) -> Self {
        match s {
            domain::ConvoyStatus::Planning => Self::Planning,
            domain::ConvoyStatus::Active => Self::Active,
            domain::ConvoyStatus::Rtb => Self::Rtb,
            domain::ConvoyStatus::Complete => Self::Complete,
            domain::ConvoyStatus::Abort => Self::Abort,
        }
    }
}

/// Mission type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Enum)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum MissionType {
    /// Intelligence, Surveillance, Reconnaissance
    Isr,
    /// Kinetic strike mission
    Strike,
    /// Escort/protection mission
    Escort,
    /// Resupply/logistics
    Resupply,
    /// Search and Rescue
    Sar,
}

/// Waypoint type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Enum)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum WaypointType {
    /// Navigation waypoint
    Nav,
    /// Loiter/orbit point
    Loiter,
    /// Strike/engagement point
    Strike,
    /// Aerial refueling point
    Refuel,
    /// Formation rendezvous
    Rendezvous,
    /// Mission checkpoint
    Checkpoint,
}

impl From<domain::WaypointType> for WaypointType {
    fn from(w: domain::WaypointType) -> Self {
        match w {
            domain::WaypointType::Nav => Self::Nav,
            domain::WaypointType::Loiter => Self::Loiter,
            domain::WaypointType::Strike => Self::Strike,
            domain::WaypointType::Refuel => Self::Refuel,
            domain::WaypointType::Rendezvous => Self::Rendezvous,
            domain::WaypointType::Checkpoint => Self::Checkpoint,
        }
    }
}

/// Waypoint completion status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Enum)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum WaypointStatus {
    /// Not yet reached
    Pending,
    /// Currently active/approaching
    Active,
    /// Successfully completed
    Complete,
    /// Skipped (replanning)
    Skipped,
}

impl From<domain::WaypointStatus> for WaypointStatus {
    fn from(s: domain::WaypointStatus) -> Self {
        match s {
            domain::WaypointStatus::Pending => Self::Pending,
            domain::WaypointStatus::Active => Self::Active,
            domain::WaypointStatus::Complete => Self::Complete,
            domain::WaypointStatus::Skipped => Self::Skipped,
        }
    }
}

/// Weapon type
///
/// Wire names pinned for the same reason as [`PlatformType`]: Inflector
/// treats digits as uppercase, so `Agm114Hellfire` would otherwise serialize
/// as `AGM_114_HELLFIRE` and every simulator post would be rejected with
/// "enumeration type WeaponType does not contain the value" — which is
/// precisely what happened on the first live run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Enum)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum WeaponType {
    /// AGM-114 Hellfire missile
    #[graphql(name = "AGM114_HELLFIRE")]
    Agm114Hellfire,
    /// GBU-12 Paveway II laser-guided bomb
    #[graphql(name = "GBU12_PAVEWAY")]
    Gbu12Paveway,
    /// AIM-9X Sidewinder air-to-air
    #[graphql(name = "AIM9X_SIDEWINDER")]
    Aim9xSidewinder,
    /// GBU-38 JDAM GPS-guided bomb
    #[graphql(name = "GBU38_JDAM")]
    Gbu38Jdam,
    /// AGM-176 Griffin small tactical munition
    #[graphql(name = "AGM176_GRIFFIN")]
    Agm176Griffin,
}

impl From<domain::WeaponType> for WeaponType {
    fn from(w: domain::WeaponType) -> Self {
        match w {
            domain::WeaponType::Agm114Hellfire => Self::Agm114Hellfire,
            domain::WeaponType::Gbu12Paveway => Self::Gbu12Paveway,
            domain::WeaponType::Aim9xSidewinder => Self::Aim9xSidewinder,
            domain::WeaponType::Gbu38Jdam => Self::Gbu38Jdam,
            domain::WeaponType::Agm176Griffin => Self::Agm176Griffin,
        }
    }
}

/// Battle damage assessment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Enum)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum DamageAssessment {
    /// Target confirmed destroyed
    Destroyed,
    /// Target damaged but not destroyed
    Damaged,
    /// Weapon missed target
    Missed,
    /// Awaiting BDA confirmation
    PendingBda,
}

impl From<domain::DamageAssessment> for DamageAssessment {
    fn from(d: domain::DamageAssessment) -> Self {
        match d {
            domain::DamageAssessment::Destroyed => Self::Destroyed,
            domain::DamageAssessment::Damaged => Self::Damaged,
            domain::DamageAssessment::Missed => Self::Missed,
            domain::DamageAssessment::PendingBda => Self::PendingBda,
        }
    }
}

/// Target type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Enum)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum TargetType {
    /// Ground vehicle
    Vehicle,
    /// Building/structure
    Structure,
    /// Personnel
    Personnel,
    /// Radar installation
    Radar,
    /// Air defense system
    AirDefense,
    /// Supply depot/cache
    Supply,
}

/// Threat level classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Enum)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum ThreatLevel {
    /// High threat - immediate danger
    High,
    /// Medium threat - caution advised
    Medium,
    /// Low threat - minimal risk
    Low,
    /// Unknown threat level
    Unknown,
}

/// Alert severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Enum)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum AlertSeverity {
    /// Critical - immediate action required
    Critical,
    /// Warning - attention needed
    Warning,
    /// Informational
    Info,
}

/// Leaderboard rank change type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Enum)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum RankChangeType {
    /// Moved up in rankings
    RankUp,
    /// Moved down in rankings
    RankDown,
    /// New entry to leaderboard
    NewEntry,
    /// Score updated, rank unchanged
    ScoreUpdate,
    /// No change
    NoChange,
}

/// Sort order for queries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Enum, Default)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum SortOrder {
    /// Ascending order
    Asc,
    /// Descending order (default)
    #[default]
    Desc,
}

// =============================================================================
// SCHEMA → DOMAIN CONVERSIONS
// =============================================================================
//
// The mutations accept schema enums and persist domain enums; these reverse
// impls are the bridge. Kept exhaustive (no `_` arm) so a new variant on
// either side is a compile error here, not silent drift — enum drift between
// this schema and its callers has already burned this repo once.

impl From<DroneStatus> for domain::DroneStatus {
    fn from(s: DroneStatus) -> Self {
        match s {
            DroneStatus::Preflight => Self::Preflight,
            DroneStatus::Airborne => Self::Airborne,
            DroneStatus::Loiter => Self::Loiter,
            DroneStatus::Ingress => Self::Ingress,
            DroneStatus::Egress => Self::Egress,
            DroneStatus::Rtb => Self::Rtb,
            DroneStatus::Landed => Self::Landed,
            DroneStatus::Maintenance => Self::Maintenance,
        }
    }
}

impl From<ConvoyStatus> for domain::ConvoyStatus {
    fn from(s: ConvoyStatus) -> Self {
        match s {
            ConvoyStatus::Planning => Self::Planning,
            ConvoyStatus::Active => Self::Active,
            ConvoyStatus::Rtb => Self::Rtb,
            ConvoyStatus::Complete => Self::Complete,
            ConvoyStatus::Abort => Self::Abort,
        }
    }
}

impl From<MissionType> for domain::MissionType {
    fn from(m: MissionType) -> Self {
        match m {
            MissionType::Isr => Self::Isr,
            MissionType::Strike => Self::Strike,
            MissionType::Escort => Self::Escort,
            MissionType::Resupply => Self::Resupply,
            MissionType::Sar => Self::Sar,
        }
    }
}

impl From<domain::MissionType> for MissionType {
    fn from(m: domain::MissionType) -> Self {
        match m {
            domain::MissionType::Isr => Self::Isr,
            domain::MissionType::Strike => Self::Strike,
            domain::MissionType::Escort => Self::Escort,
            domain::MissionType::Resupply => Self::Resupply,
            domain::MissionType::Sar => Self::Sar,
        }
    }
}

impl From<WaypointType> for domain::WaypointType {
    fn from(w: WaypointType) -> Self {
        match w {
            WaypointType::Nav => Self::Nav,
            WaypointType::Loiter => Self::Loiter,
            WaypointType::Strike => Self::Strike,
            WaypointType::Refuel => Self::Refuel,
            WaypointType::Rendezvous => Self::Rendezvous,
            WaypointType::Checkpoint => Self::Checkpoint,
        }
    }
}

impl From<WeaponType> for domain::WeaponType {
    fn from(w: WeaponType) -> Self {
        match w {
            WeaponType::Agm114Hellfire => Self::Agm114Hellfire,
            WeaponType::Gbu12Paveway => Self::Gbu12Paveway,
            WeaponType::Aim9xSidewinder => Self::Aim9xSidewinder,
            WeaponType::Gbu38Jdam => Self::Gbu38Jdam,
            WeaponType::Agm176Griffin => Self::Agm176Griffin,
        }
    }
}

impl From<TargetType> for domain::TargetType {
    fn from(t: TargetType) -> Self {
        match t {
            TargetType::Vehicle => Self::Vehicle,
            TargetType::Structure => Self::Structure,
            TargetType::Personnel => Self::Personnel,
            TargetType::Radar => Self::Radar,
            TargetType::AirDefense => Self::AirDefense,
            TargetType::Supply => Self::Supply,
        }
    }
}

impl From<domain::TargetType> for TargetType {
    fn from(t: domain::TargetType) -> Self {
        match t {
            domain::TargetType::Vehicle => Self::Vehicle,
            domain::TargetType::Structure => Self::Structure,
            domain::TargetType::Personnel => Self::Personnel,
            domain::TargetType::Radar => Self::Radar,
            domain::TargetType::AirDefense => Self::AirDefense,
            domain::TargetType::Supply => Self::Supply,
        }
    }
}
