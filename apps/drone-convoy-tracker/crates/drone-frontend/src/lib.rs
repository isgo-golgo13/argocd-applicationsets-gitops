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
use leptos::task::spawn_local;
use uuid::Uuid;

use components::*;
use wasm_bindgen::JsCast;
use state::*;

/// Leaderboard poll interval. The GraphQL subscription route is not yet mounted
/// server-side, so the dashboard pulls rather than being pushed to. Swap this
/// for `use_websocket` once `/graphql/ws` is enabled.
const POLL_INTERVAL_MS: i32 = 2_000;

#[component]
pub fn App() -> impl IntoView {
    provide_app_state();
    // Positions and the engagement feed still come from a seed: the resolvers
    // behind them are stubs (see start_live_feed).
    seed_static_panels();
    // The leaderboard is real: simulator -> recordEngagement -> ScyllaDB -> here.
    start_live_feed();

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

/// Poll the real leaderboard and push it into state.
///
/// This is the one path that is live end to end today. `recordEngagement`
/// writes through `ScyllaLeaderboardRepository`, and the `leaderboard` query
/// reads back from it -- no stub in between.
///
/// `ws_connected` now reflects whether the last poll actually succeeded,
/// instead of being set to true unconditionally.
fn start_live_feed() {
    let state = use_app_state();

    spawn_local(async move {
        match services::fetch_active_convoys().await {
            Ok(convoys) => {
                if let Some(first) = convoys.first() {
                    if let Ok(id) = Uuid::parse_str(&first.convoy_id) {
                        state.selected_convoy.set(Some(id));
                        log::info!("tracking convoy {} ({})", first.callsign, id);
                    }
                }
            }
            Err(err) => {
                log::error!("could not list convoys: {err}");
                state.ws_connected.set(false);
            }
        }

        poll_leaderboard(state);
    });
}

fn poll_leaderboard(state: AppState) {
    let tick = move || {
        spawn_local(async move {
            let Some(convoy_id) = state.selected_convoy.get_untracked() else {
                return;
            };
            match services::fetch_leaderboard(convoy_id, 10).await {
                Ok(entries) => {
                    state.ws_connected.set(true);
                    state.leaderboard.set(entries);
                }
                Err(err) => {
                    state.ws_connected.set(false);
                    log::warn!("leaderboard poll failed: {err}");
                }
            }
        });
    };

    tick();

    let closure = wasm_bindgen::closure::Closure::wrap(Box::new(tick) as Box<dyn Fn()>);
    if let Some(window) = web_sys::window() {
        let _ = window.set_interval_with_callback_and_timeout_and_arguments_0(
            closure.as_ref().unchecked_ref(),
            POLL_INTERVAL_MS,
        );
    }
    // Deliberately leaked: the interval outlives this scope for the life of the
    // page. Dropping the closure here would invalidate the callback.
    closure.forget();
}

/// Seed for the panels whose resolvers are still stubs: drone positions,
/// telemetry and the engagement feed. Everything here is placeholder data and
/// is replaced the moment `getDrones` / `engagements` are wired to their
/// repositories -- which already exist in drone-persistence.
fn seed_static_panels() {
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
}

pub fn main() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Debug);
    log::info!("Drone Convoy Tracker v{}", env!("CARGO_PKG_VERSION"));
    leptos::mount::mount_to_body(App);
}
