//! # Drone Card Component
//!
//! Individual drone status display.

use leptos::prelude::*;

use crate::state::{use_app_state, DroneState};

/// Drone list panel
#[component]
pub fn DroneListPanel() -> impl IntoView {
    let state = use_app_state();

    let drones = move || {
        let map = state.drones.get();
        let mut list: Vec<_> = map.values().cloned().collect();
        list.sort_by(|a, b| a.callsign.cmp(&b.callsign));
        list
    };

    let total = move || drones().len();
    let airborne = move || drones().iter().filter(|d| d.status.status_class() == "nominal").count();

    view! {
        <div class="panel">
            <div class="panel-header">
                <span class="panel-title">"CONVOY ASSETS"</span>
                <span class="panel-badge">{airborne}"/"{ total}" AIRBORNE"</span>
            </div>
            <div class="panel-body" style="display: flex; flex-direction: column; gap: 8px;">
                <For
                    each=drones
                    key=|drone| drone.drone_id
                    children=move |drone| view! { <DroneCard drone=drone /> }
                />
            </div>
        </div>
    }
}

/// Single drone card
#[component]
pub fn DroneCard(drone: DroneState) -> impl IntoView {
    let state = use_app_state();
    let drone_id = drone.drone_id;

    let is_selected = move || state.selected_drone.get() == Some(drone_id);

    let on_click = move |_| {
        let current = state.selected_drone.get();
        if current == Some(drone_id) {
            state.selected_drone.set(None);
        } else {
            state.selected_drone.set(Some(drone_id));
        }
    };

    let fuel_class = if drone.fuel_pct < 20.0 {
        "critical"
    } else if drone.fuel_pct < 40.0 {
        "warning"
    } else {
        ""
    };

    let progress_pct = (drone.current_waypoint as f32 / drone.total_waypoints as f32) * 100.0;

    let platform_icon = match drone.platform_type.as_str() {
        "MQ9_REAPER" | "MQ1C_GRAY_EAGLE" => "✈",
        "RQ4_GLOBAL_HAWK" => "🛩",
        "MQ25_STINGRAY" => "⚓",
        _ => "●",
    };

    view! {
        <div
            class="drone-card"
            class:selected=is_selected
            on:click=on_click
        >
            <div class="drone-icon">
                <span style="font-size: 24px;">{platform_icon}</span>
            </div>
            <div class="drone-details">
                <div class="drone-callsign">{drone.callsign.clone()}</div>
                <div class="drone-tail">{drone.tail_number.clone()}</div>
                <div class="progress-bar" style="margin-top: 4px;">
                    <div
                        class="progress-fill"
                        style=format!("width: {}%;", progress_pct)
                    ></div>
                </div>
                <div class="text-xs text-muted" style="margin-top: 2px;">
                    "WP "{drone.current_waypoint}"/"{ drone.total_waypoints}
                </div>
            </div>
            <div class="drone-metrics">
                <div class=format!("status-badge {}", drone.status.status_class())>
                    {drone.status.as_str()}
                </div>
                <div class="metric">
                    <span class="metric-label">"FUEL"</span>
                    <span class=format!("metric-value {}", fuel_class)>
                        {format!("{:.0}%", drone.fuel_pct)}
                    </span>
                </div>
                <div class="metric">
                    <span class="metric-label">"ACC"</span>
                    <span class="metric-value text-accent">
                        {format!("{:.1}%", drone.accuracy_pct)}
                    </span>
                </div>
            </div>
        </div>
    }
}

/// Empty state for drone list
#[component]
pub fn DroneListEmpty() -> impl IntoView {
    view! {
        <div class="panel">
            <div class="panel-header">
                <span class="panel-title">"CONVOY ASSETS"</span>
            </div>
            <div class="panel-body" style="text-align: center; padding: 32px;">
                <div class="text-muted">"No drones assigned to convoy"</div>
            </div>
        </div>
    }
}
