//! # Engagement Feed Component
//!
//! Real-time feed of weapon engagements.

use leptos::prelude::*;

use crate::state::{use_app_state, EngagementEvent};

/// Engagement feed panel
#[component]
pub fn EngagementFeedPanel() -> impl IntoView {
    let state = use_app_state();

    let events = move || state.engagements.get();
    let hit_count = move || events().iter().filter(|e| e.hit).count();
    let total_count = move || events().len();

    view! {
        <div class="panel">
            <div class="panel-header">
                <span class="panel-title">"ENGAGEMENT FEED"</span>
                <span class="panel-badge">{hit_count}"/"{ total_count}</span>
            </div>
            <div class="panel-body no-padding">
                <div class="engagement-feed">
                    <For
                        each=events
                        key=|event| event.id
                        children=move |event| view! { <EngagementItem event=event /> }
                    />
                    {move || {
                        if events().is_empty() {
                            Some(view! {
                                <div style="padding: 24px; text-align: center; color: var(--text-muted);">
                                    "Awaiting engagement data..."
                                </div>
                            })
                        } else {
                            None
                        }
                    }}
                </div>
            </div>
        </div>
    }
}

/// Single engagement item
#[component]
fn EngagementItem(event: EngagementEvent) -> impl IntoView {
    let hit_class = if event.hit { "hit" } else { "miss" };
    let result_text = if event.hit { "HIT" } else { "MISS" };
    let result_color = if event.hit { "var(--status-nominal)" } else { "var(--status-critical)" };

    let weapon_short = match event.weapon_type.as_str() {
        "AGM114_HELLFIRE" => "AGM-114",
        "GBU12_PAVEWAY" => "GBU-12",
        "AIM9X_SIDEWINDER" => "AIM-9X",
        "GBU38_JDAM" => "GBU-38",
        "AGM176_GRIFFIN" => "AGM-176",
        _ => &event.weapon_type,
    };

    let time_str = event.timestamp.format("%H:%M:%S").to_string();

    view! {
        <div class=format!("engagement-item {}", hit_class)>
            <span class="status-dot" class:nominal=event.hit class:critical=!event.hit></span>
            <div class="engagement-info">
                <div class="engagement-callsign">
                    {event.callsign.clone()}
                    " "
                    <span style=format!("color: {};", result_color)>{result_text}</span>
                </div>
                <div class="engagement-weapon">
                    {weapon_short.to_string()}" → "{format!("{:.1}%", event.new_accuracy_pct)}
                </div>
            </div>
            <div class="engagement-time">{time_str}"Z"</div>
        </div>
    }
}
