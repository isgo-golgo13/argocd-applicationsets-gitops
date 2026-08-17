//! # Footer Component
//!
//! Status bar with system information.

use leptos::prelude::*;

/// Dissemination marking shown in the footer. FOUO ("For Official Use Only")
/// is the legacy pre-2020 DoD marking; the current equivalent is CUI
/// ("Controlled Unclassified Information"). Kept as one constant so the
/// demo can wear whichever reads right for the room.
pub const CLASSIFICATION_MARKING: &str = "UNCLASSIFIED // CUI";

use crate::state::use_app_state;

/// Footer status bar
#[component]
pub fn Footer() -> impl IntoView {
    let state = use_app_state();

    let connection_status = move || {
        if state.ws_connected.get() {
            ("nominal", "CONNECTED")
        } else {
            ("critical", "DISCONNECTED")
        }
    };

    let drone_count = move || state.drones.get().len();
    let alert_count = move || state.alerts.get().len();

    view! {
        <footer class="hud-footer">
            <div class="flex items-center gap-lg">
                <span class="text-muted">"DRONE OPS v0.1.0"</span>
                <span class="text-muted">"|"</span>
                <span>
                    <span class="text-muted">"ASSETS: "</span>
                    <span class="text-accent">{drone_count}</span>
                </span>
            </div>

            <div class="flex items-center gap-lg">
                {move || {
                    let count = alert_count();
                    if count > 0 {
                        Some(view! {
                            <span class="status-badge warning">
                                {count}" ALERTS"
                            </span>
                        })
                    } else {
                        None
                    }
                }}

                <span class="flex items-center gap-xs">
                    <span class=move || format!("status-dot {}", connection_status().0)></span>
                    <span class="text-sm">{move || connection_status().1}</span>
                </span>

                <span class="text-muted">{format!("CLASSIFICATION: {CLASSIFICATION_MARKING}")}</span>
            </div>
        </footer>
    }
}
