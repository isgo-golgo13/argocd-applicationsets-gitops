//! # Drone Convoy Tracker Frontend
//!
//! Tactical HUD for military drone convoy tracking and leaderboard display.

#![forbid(unsafe_code)]
#![warn(clippy::all)]

pub mod components;
pub mod services;
pub mod state;

use chrono::Utc;
use leptos::prelude::*;
use uuid::Uuid;

use components::*;
use state::*;

#[component]
pub fn App() -> impl IntoView {
    provide_app_state();
    load_mock_data();

    view! {
        <div class="scanlines"></div>
        <div class="hud-container">
            <Header />
            <div class="hud-left-panel">
                <LeaderboardPanel />
                <DroneListPanel />
            </div>
            <div class="hud-main">
                <MapPanel />
            </div>
            <div class="hud-right-panel">
                <ConvoyStatsPanel />
                <TelemetryChartPanel />
                <EngagementFeedPanel />
            </div>
            <Footer />
        </div>
        <ToastContainer />
    }
}

#[component]
fn ToastContainer() -> impl IntoView {
    let state = use_app_state();

    view! {
        <div class="toast-container">
            <For
                each=move || state.alerts.get()
                key=|alert| alert.id
                children=move |alert| {
                    let id = alert.id;
                    let on_dismiss = move |_| {
                        state.alerts.update(|alerts| alerts.retain(|a| a.id != id));
                    };
                    view! {
                        <div class="toast">
                            <div class="flex justify-between items-center gap-md">
                                <div class="flex items-center gap-sm">
                                    <span class=format!("status-dot {}", alert.severity.class())></span>
                                    <span>{alert.message.clone()}</span>
                                </div>
                                <button class="btn btn-sm" on:click=on_dismiss>"×"</button>
                            </div>
                        </div>
                    }
                }
            />
        </div>
    }
}

fn load_mock_data() {
    let state = use_app_state();
    state.mission_start.set(Some(Utc::now() - chrono::Duration::hours(2)));

    let convoy_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    state.selected_convoy.set(Some(convoy_id));

    let leaderboard = vec![
        LeaderboardEntry {
            drone_id: Uuid::new_v4(),
            callsign: "REAPER-01".into(),
            platform_type: "MQ9_REAPER".into(),
            rank: 1, accuracy_pct: 94.5, total_engagements: 18, successful_hits: 17,
            current_streak: 8, best_streak: 12, rank_change: 0,
        },
        LeaderboardEntry {
            drone_id: Uuid::new_v4(),
            callsign: "HAWK-07".into(),
            platform_type: "MQ1C_GRAY_EAGLE".into(),
            rank: 2, accuracy_pct: 91.2, total_engagements: 23, successful_hits: 21,
            current_streak: 5, best_streak: 9, rank_change: 1,
        },
        LeaderboardEntry {
            drone_id: Uuid::new_v4(),
            callsign: "SHADOW-12".into(),
            platform_type: "MQ9_REAPER".into(),
            rank: 3, accuracy_pct: 88.9, total_engagements: 9, successful_hits: 8,
            current_streak: 3, best_streak: 6, rank_change: -1,
        },
        LeaderboardEntry {
            drone_id: Uuid::new_v4(),
            callsign: "VIPER-03".into(),
            platform_type: "MQ9_REAPER".into(),
            rank: 4, accuracy_pct: 85.7, total_engagements: 14, successful_hits: 12,
            current_streak: 0, best_streak: 4, rank_change: 0,
        },
        LeaderboardEntry {
            drone_id: Uuid::new_v4(),
            callsign: "EAGLE-09".into(),
            platform_type: "RQ4_GLOBAL_HAWK".into(),
            rank: 5, accuracy_pct: 80.0, total_engagements: 5, successful_hits: 4,
            current_streak: 2, best_streak: 3, rank_change: 2,
        },
    ];
    state.leaderboard.set(leaderboard);

    let drones = vec![
        DroneState {
            drone_id: Uuid::new_v4(), convoy_id,
            callsign: "REAPER-01".into(), tail_number: "AF-001".into(),
            platform_type: "MQ9_REAPER".into(), status: DroneStatus::Airborne,
            position: Coordinates { latitude: 31.6289, longitude: 65.7372, altitude_m: 5000.0, heading_deg: 45.0, speed_mps: 80.0 },
            fuel_pct: 72.5, accuracy_pct: 94.5, current_waypoint: 15, total_waypoints: 25, updated_at: Utc::now(),
        },
        DroneState {
            drone_id: Uuid::new_v4(), convoy_id,
            callsign: "HAWK-07".into(), tail_number: "AF-007".into(),
            platform_type: "MQ1C_GRAY_EAGLE".into(), status: DroneStatus::Loiter,
            position: Coordinates { latitude: 31.75, longitude: 65.85, altitude_m: 4500.0, heading_deg: 90.0, speed_mps: 45.0 },
            fuel_pct: 58.0, accuracy_pct: 91.2, current_waypoint: 18, total_waypoints: 25, updated_at: Utc::now(),
        },
        DroneState {
            drone_id: Uuid::new_v4(), convoy_id,
            callsign: "SHADOW-12".into(), tail_number: "AF-012".into(),
            platform_type: "MQ9_REAPER".into(), status: DroneStatus::Ingress,
            position: Coordinates { latitude: 31.5, longitude: 65.5, altitude_m: 5500.0, heading_deg: 270.0, speed_mps: 95.0 },
            fuel_pct: 35.0, accuracy_pct: 88.9, current_waypoint: 20, total_waypoints: 25, updated_at: Utc::now(),
        },
        DroneState {
            drone_id: Uuid::new_v4(), convoy_id,
            callsign: "VIPER-03".into(), tail_number: "AF-003".into(),
            platform_type: "MQ9_REAPER".into(), status: DroneStatus::Rtb,
            position: Coordinates { latitude: 31.4, longitude: 65.9, altitude_m: 3000.0, heading_deg: 180.0, speed_mps: 110.0 },
            fuel_pct: 18.0, accuracy_pct: 85.7, current_waypoint: 23, total_waypoints: 25, updated_at: Utc::now(),
        },
    ];
    state.drones.update(|map| { for d in drones { map.insert(d.drone_id, d); } });

    let engagements = vec![
        EngagementEvent { id: Uuid::new_v4(), drone_id: Uuid::new_v4(), callsign: "REAPER-01".into(), hit: true, weapon_type: "AGM114_HELLFIRE".into(), new_accuracy_pct: 94.5, timestamp: Utc::now() - chrono::Duration::minutes(5) },
        EngagementEvent { id: Uuid::new_v4(), drone_id: Uuid::new_v4(), callsign: "HAWK-07".into(), hit: true, weapon_type: "GBU12_PAVEWAY".into(), new_accuracy_pct: 91.2, timestamp: Utc::now() - chrono::Duration::minutes(12) },
        EngagementEvent { id: Uuid::new_v4(), drone_id: Uuid::new_v4(), callsign: "SHADOW-12".into(), hit: false, weapon_type: "AGM114_HELLFIRE".into(), new_accuracy_pct: 88.9, timestamp: Utc::now() - chrono::Duration::minutes(18) },
    ];
    state.engagements.set(engagements);
    state.ws_connected.set(true);
}

pub fn main() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Debug);
    log::info!("Drone Convoy Tracker v{}", env!("CARGO_PKG_VERSION"));
    leptos::mount::mount_to_body(App);
}
