//! # Header Component
//!
//! Top navigation bar with logo, mission clock, and status.

use chrono::{DateTime, Timelike, Utc};
use leptos::prelude::*;

use crate::state::use_app_state;

/// Header component with logo and mission clock
#[component]
pub fn Header() -> impl IntoView {
    let state = use_app_state();
    let (time, set_time) = signal(Utc::now());

    // Update clock every second
    Effect::new(move |_| {
        let handle = gloo_timers::callback::Interval::new(1000, move || {
            set_time.set(Utc::now());
        });
        handle.forget();
    });

    let mission_elapsed = move || {
        state.mission_start.get().map(|start| {
            let duration = Utc::now() - start;
            let hours = duration.num_hours();
            let minutes = duration.num_minutes() % 60;
            let seconds = duration.num_seconds() % 60;
            format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
        })
    };

    let format_zulu = move |dt: DateTime<Utc>| {
        format!("{:02}:{:02}:{:02}Z", dt.hour(), dt.minute(), dt.second())
    };

    let format_date = move |dt: DateTime<Utc>| {
        dt.format("%d %b %Y").to_string().to_uppercase()
    };

    let ws_status = move || {
        if state.ws_connected.get() {
            ("nominal", "ONLINE")
        } else {
            ("critical", "OFFLINE")
        }
    };

    view! {
        <header class="hud-header">
            <div class="logo">
                <svg class="logo-icon" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M12 2L2 7v10l10 5 10-5V7L12 2zm0 2.18l6.9 3.45L12 11.09 5.1 7.63 12 4.18zM4 8.82l7 3.5v6.86l-7-3.5V8.82zm9 10.36v-6.86l7-3.5v6.86l-7 3.5z"/>
                </svg>
                <div>
                    <div class="logo-text">"CONVOY TRACKER"</div>
                    <div class="logo-subtitle">"DRONE OPS COMMAND"</div>
                </div>
            </div>

            <div class="mission-clock">
                <div class="clock-segment">
                    <div class="clock-label">"ZULU"</div>
                    <div class="clock-value">{move || format_zulu(time.get())}</div>
                </div>

                <div class="clock-segment">
                    <div class="clock-label">"DATE"</div>
                    <div class="clock-value">{move || format_date(time.get())}</div>
                </div>

                {move || mission_elapsed().map(|elapsed| view! {
                    <div class="clock-segment">
                        <div class="clock-label">"MISSION"</div>
                        <div class="clock-value">{elapsed}</div>
                    </div>
                })}
            </div>

            <div class="flex items-center gap-md">
                <div class="status-badge" class:nominal=move || ws_status().0 == "nominal" class:critical=move || ws_status().0 == "critical">
                    <span class="status-dot" class:nominal=move || ws_status().0 == "nominal" class:critical=move || ws_status().0 == "critical"></span>
                    {move || ws_status().1}
                </div>
            </div>
        </header>
    }
}
