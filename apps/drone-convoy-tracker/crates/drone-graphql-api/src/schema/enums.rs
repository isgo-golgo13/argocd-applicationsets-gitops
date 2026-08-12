//! # GraphQL Enum Types
//!
//! Enum definitions for the GraphQL schema.

use async_graphql::Enum;
use drone_domain as domain;

/// Drone platform type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Enum)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum PlatformType {
    /// MQ-9 Reaper - Primary strike/ISR platform
    Mq9Reaper,
    /// MQ-1C Gray Eagle - Army tactical UAS
    Mq1cGrayEagle,
    /// RQ-4 Global Hawk - High-altitude ISR
    Rq4GlobalHawk,
    /// MQ-25 Stingray - Carrier-based refueling
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Enum)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum WeaponType {
    /// AGM-114 Hellfire missile
    Agm114Hellfire,
    /// GBU-12 Paveway II laser-guided bomb
    Gbu12Paveway,
    /// AIM-9X Sidewinder air-to-air
    Aim9xSidewinder,
    /// GBU-38 JDAM GPS-guided bomb
    Gbu38Jdam,
    /// AGM-176 Griffin small tactical munition
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
