//! # Header Component
//!
//! Top navigation bar with logo, mission clock, and status.

use chrono::{DateTime, Timelike, Utc};
use leptos::prelude::*;

use crate::state::use_app_state;
use crate::components::{regions::TheaterId, tactical_select::{SelectOption, SelectTone, TacticalSelect}};


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
        // Derived from the ticking `time` signal, not Utc::now(): mission_start
        // is set once and never changes, so a closure depending on it alone
        // renders once and freezes at 00:00:00. Reading `time` re-runs this
        // every second alongside the ZULU clock.
        state.mission_start.get().map(|start| {
            let duration = time.get() - start;
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

    // Built outside the view! macro: its parser takes an attribute value as a
    // single expression, not a multi-line method chain.
    let theater_options: Vec<SelectOption<TheaterId>> = TheaterId::ALL
        .iter()
        .map(|t| SelectOption { key: *t, label: t.theater().label })
        .collect();

    // THE SELECTOR IS THE COMMANDER. Every value it holds -- including the
    // one it opens with -- is a tasking order to the convoy record; the
    // simulator (or a live ground station) obeys it. The opening view is
    // the standing order: if the record disagrees (a previous run flew
    // elsewhere), this reconciles it within a few ticks; if it already
    // matches, the write is a harmless no-op.
    //
    // Two triggers, one Effect: (a) the theater changes; (b) the convoy
    // FIRST becomes visible (drones 0 -> 1). (b) exists for a fresh DB: the
    // service's bootstrap creates the convoy a few seconds after the page
    // loads, so an order issued at load can find "no convoy" -- re-issuing
    // once drones exist makes the opening view stick regardless of timing.
    {
        let last_sent: StoredValue<Option<(TheaterId, bool)>> = StoredValue::new(None);
        Effect::new(move |_| {
            let theater = state.selected_theater.get();
            let have_drones = !state.drones.get().is_empty();
            let key = (theater, have_drones);
            // Skip pure repeats (poll ticks re-firing state.drones with drones
            // still present); fire on theater change or on the 0 -> 1 edge.
            if last_sent.get_value() == Some(key) { return; }
            last_sent.set_value(Some(key));
            let Some(convoy_id) = state.selected_convoy.get_untracked() else {
                log::warn!("retask -> {}: no convoy selected yet, order not sent", theater.slug());
                return;
            };
            log::info!("tasking order: convoy {} -> {}", convoy_id, theater.slug());
            state.retasking.set(Some(theater));
            state.retask_error.set(None);
            leptos::task::spawn_local(async move {
                match crate::services::retask_convoy(convoy_id, theater.slug()).await {
                    Ok(()) => log::info!("tasking order accepted -> {}", theater.slug()),
                    Err(e) => {
                        // "not found" on a fresh DB is expected: the service's
                        // bootstrap hasn't created the convoy yet. The order is
                        // re-issued automatically once drones appear, so don't
                        // paint a rejection for it. Anything else is real.
                        if e.to_lowercase().contains("not found") {
                            log::info!("tasking order deferred: convoy not registered yet");
                        } else {
                            log::error!("tasking order REJECTED: {e}");
                            state.retasking.set(None);
                            state.retask_error.set(Some(e));
                        }
                    }
                }
            });
        });
    }

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
                // Mission selector: which tactical theater the map shows.
                // Sits immediately left of the link status pill. Writes only
                // the theater signal; the map owns everything that follows.
                <TacticalSelect
                    label="THEATER"
                    options=theater_options
                    value=state.selected_theater
                    tone=SelectTone::Danger
                />
                <div class="status-badge" class:nominal=move || ws_status().0 == "nominal" class:critical=move || ws_status().0 == "critical">
                    <span class="status-dot" class:nominal=move || ws_status().0 == "nominal" class:critical=move || ws_status().0 == "critical"></span>
                    {move || ws_status().1}
                </div>
            </div>
        </header>
    }
}
