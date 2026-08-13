//! # Drone Convoy Tracker Frontend
//!
//! Tactical HUD for military drone convoy tracking and leaderboard display.
//!
//! Every panel is live. There is no seed data in this crate: the leaderboard,
//! drone cards, convoy status, engagement feed, telemetry chart and the map's
//! airframes are all driven by the 2-second poll below against the GraphQL
//! API, which reads ScyllaDB. An empty panel means an empty table — start the
//! simulator.

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

/// Poll interval for the live feed. The `/graphql/ws` subscription route is
/// mounted server-side now; swapping this poll for `use_websocket` is the
/// next step, but polling is the fallback either way.
const POLL_INTERVAL_MS: i32 = 2_000;

/// The well-known demo convoy. The simulator pins its writes to this id
/// (overridable via DRONE_CONVOY_ID), so the dashboard tracks it IMMEDIATELY
/// on load instead of waiting for a convoy-list round trip — that await was
/// worth 20+ seconds of OFFLINE on every refresh. `activeConvoys` is still
/// queried right after and overrides this if a different convoy is live.
const DEMO_CONVOY_ID: &str = "550e8400-e29b-41d4-a716-446655440000";

/// Points kept in the rolling telemetry chart (~1 minute at 2s per point).
const TELEMETRY_SERIES_CAP: usize = 30;

#[component]
pub fn App() -> impl IntoView {
    provide_app_state();
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

/// Start the live feed for every panel.
///
/// The demo convoy id is selected synchronously so the first poll fires on
/// the first tick — ONLINE within one interval of page load. The convoy list
/// is then fetched in the background purely to override the selection if a
/// different convoy is actually live.
fn start_live_feed() {
    let state = use_app_state();

    state.mission_start.set(Some(Utc::now()));
    if let Ok(id) = Uuid::parse_str(DEMO_CONVOY_ID) {
        state.selected_convoy.set(Some(id));
    }

    spawn_local(async move {
        match services::fetch_active_convoys().await {
            Ok(convoys) => {
                if let Some(first) = convoys.first() {
                    if let Ok(id) = Uuid::parse_str(&first.convoy_id) {
                        state.selected_convoy.set(Some(id));
                        log::info!("tracking convoy {} ({})", first.callsign, id);
                    }
                } else {
                    log::info!(
                        "no active convoys yet; tracking demo convoy until the simulator bootstraps"
                    );
                }
            }
            Err(err) => log::warn!("could not list convoys: {err}"),
        }
    });

    poll_live_data(state);
}

/// The 2-second tick behind every panel.
///
/// Three queries per tick — leaderboard, drones, engagements. `ws_connected`
/// reflects whether the leaderboard poll (the cheapest, always-valid one)
/// succeeded, so the ONLINE pill is an honest link indicator.
fn poll_live_data(state: AppState) {
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

            match services::fetch_drones(convoy_id).await {
                Ok(drones) => {
                    push_telemetry_point(state.telemetry_series, &drones);
                    state.drones.set(drones.into_iter().map(|d| (d.drone_id, d)).collect());
                }
                Err(err) => log::warn!("drones poll failed: {err}"),
            }

            match services::fetch_engagements(convoy_id, 20).await {
                Ok(mut engagements) => {
                    // The engagement record carries no post-shot accuracy;
                    // stamp it from the freshest leaderboard so the feed
                    // reads like the subscription event would.
                    let accuracy: std::collections::HashMap<Uuid, f32> = state
                        .leaderboard
                        .get_untracked()
                        .iter()
                        .map(|e| (e.drone_id, e.accuracy_pct))
                        .collect();
                    for e in &mut engagements {
                        if let Some(acc) = accuracy.get(&e.drone_id) {
                            e.new_accuracy_pct = *acc;
                        }
                    }
                    state.engagements.set(engagements);
                }
                Err(err) => log::warn!("engagements poll failed: {err}"),
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

/// Append one convoy-average sample to the rolling telemetry series.
///
/// Takes the signal rather than `AppState`: `RwSignal` is `Copy`, so the
/// poll closure captures a copy per Rust 2021 disjoint-field capture and
/// stays `Fn` — passing the whole (non-Copy) state would move it out and
/// demote the closure to `FnOnce`.
///
/// Averages across airborne assets keep the chart meaningful as drones join
/// and leave; the cap keeps it a sliding window rather than a full history.
fn push_telemetry_point(series_signal: RwSignal<Vec<TelemetryPoint>>, drones: &[DroneState]) {
    if drones.is_empty() {
        return;
    }
    let n = drones.len() as f64;
    // Rounded to one decimal at the source: raw f64 averages leak float dust
    // ("100.00000000046748") into the chart tooltip. Rounding the data fixes
    // the tooltip AND the axis without touching the chart library's formatter.
    let avg_altitude_m = ((drones.iter().map(|d| d.position.altitude_m).sum::<f64>() / n) * 10.0).round() / 10.0;
    let avg_fuel_pct = ((drones.iter().map(|d| f64::from(d.fuel_pct)).sum::<f64>() / n) * 10.0).round() / 10.0;
    let label = Utc::now().format("%H:%M:%S").to_string();

    series_signal.update(|series| {
        series.push(TelemetryPoint {
            label,
            avg_altitude_m,
            avg_fuel_pct,
        });
        if series.len() > TELEMETRY_SERIES_CAP {
            let excess = series.len() - TELEMETRY_SERIES_CAP;
            series.drain(0..excess);
        }
    });
}

pub fn main() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Debug);
    log::info!("Drone Convoy Tracker v{}", env!("CARGO_PKG_VERSION"));
    leptos::mount::mount_to_body(App);
}
