//! # Application State
//!
//! Reactive state management for the drone convoy HUD.

use chrono::{DateTime, Utc};
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Global application state
#[derive(Clone, Debug)]
pub struct AppState {
    pub selected_convoy: RwSignal<Option<Uuid>>,
    pub selected_drone: RwSignal<Option<Uuid>>,
    pub leaderboard: RwSignal<Vec<LeaderboardEntry>>,
    pub drones: RwSignal<HashMap<Uuid, DroneState>>,
    pub engagements: RwSignal<Vec<EngagementEvent>>,
    pub ws_connected: RwSignal<bool>,
    pub mission_start: RwSignal<Option<DateTime<Utc>>>,
    pub alerts: RwSignal<Vec<Alert>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            selected_convoy: RwSignal::new(None),
            selected_drone: RwSignal::new(None),
            leaderboard: RwSignal::new(Vec::new()),
            drones: RwSignal::new(HashMap::new()),
            engagements: RwSignal::new(Vec::new()),
            ws_connected: RwSignal::new(false),
            mission_start: RwSignal::new(None),
            alerts: RwSignal::new(Vec::new()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LeaderboardEntry {
    pub drone_id: Uuid,
    pub callsign: String,
    pub platform_type: String,
    pub rank: u32,
    pub accuracy_pct: f32,
    pub total_engagements: u32,
    pub successful_hits: u32,
    pub current_streak: i32,
    pub best_streak: i32,
    pub rank_change: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DroneState {
    pub drone_id: Uuid,
    pub convoy_id: Uuid,
    pub callsign: String,
    pub tail_number: String,
    pub platform_type: String,
    pub status: DroneStatus,
    pub position: Coordinates,
    pub fuel_pct: f32,
    pub accuracy_pct: f32,
    pub current_waypoint: u32,
    pub total_waypoints: u32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DroneStatus {
    Preflight,
    Airborne,
    Loiter,
    Ingress,
    Egress,
    Rtb,
    Landed,
    Maintenance,
}

impl DroneStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Preflight => "PREFLIGHT",
            Self::Airborne => "AIRBORNE",
            Self::Loiter => "LOITER",
            Self::Ingress => "INGRESS",
            Self::Egress => "EGRESS",
            Self::Rtb => "RTB",
            Self::Landed => "LANDED",
            Self::Maintenance => "MAINT",
        }
    }

    pub fn status_class(&self) -> &'static str {
        match self {
            Self::Airborne | Self::Loiter | Self::Ingress | Self::Egress => "nominal",
            Self::Rtb | Self::Preflight => "warning",
            Self::Landed | Self::Maintenance => "offline",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Coordinates {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_m: f64,
    pub heading_deg: f32,
    pub speed_mps: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EngagementEvent {
    pub id: Uuid,
    pub drone_id: Uuid,
    pub callsign: String,
    pub hit: bool,
    pub weapon_type: String,
    pub new_accuracy_pct: f32,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Alert {
    pub id: Uuid,
    pub severity: AlertSeverity,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

impl AlertSeverity {
    pub fn class(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Waypoint {
    pub id: Uuid,
    pub sequence: u32,
    pub name: String,
    pub coordinates: Coordinates,
    pub status: WaypointStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum WaypointStatus {
    Pending,
    Active,
    Complete,
    Skipped,
}

pub fn provide_app_state() {
    let state = AppState::new();
    provide_context(state);
}

pub fn use_app_state() -> AppState {
    expect_context::<AppState>()
}
