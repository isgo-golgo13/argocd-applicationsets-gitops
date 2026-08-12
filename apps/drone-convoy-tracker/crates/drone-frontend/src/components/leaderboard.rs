//! # Leaderboard Component
//!
//! Real-time accuracy rankings display.

use leptos::prelude::*;

use crate::state::{use_app_state, LeaderboardEntry};

/// Leaderboard panel component
#[component]
pub fn LeaderboardPanel() -> impl IntoView {
    let state = use_app_state();

    let entries = move || state.leaderboard.get();
    let total = move || entries().len();

    view! {
        <div class="panel">
            <div class="panel-header">
                <span class="panel-title">"ACCURACY LEADERBOARD"</span>
                <span class="panel-badge">{total}</span>
            </div>
            <div class="panel-body no-padding">
                <div class="leaderboard">
                    <For
                        each=entries
                        key=|entry| entry.drone_id
                        children=move |entry| view! { <LeaderboardRow entry=entry /> }
                    />
                </div>
            </div>
        </div>
    }
}

/// Single leaderboard row
#[component]
fn LeaderboardRow(entry: LeaderboardEntry) -> impl IntoView {
    let rank_class = match entry.rank {
        1 => "rank-1",
        2 => "rank-2",
        3 => "rank-3",
        _ => "",
    };

    let rank_change_view = move || {
        if entry.rank_change > 0 {
            Some(view! {
                <span class="rank-change up">
                    "▲" {entry.rank_change}
                </span>
            })
        } else if entry.rank_change < 0 {
            Some(view! {
                <span class="rank-change down">
                    "▼" {entry.rank_change.abs()}
                </span>
            })
        } else {
            None
        }
    };

    let platform_short = match entry.platform_type.as_str() {
        "MQ9_REAPER" => "MQ-9",
        "MQ1C_GRAY_EAGLE" => "MQ-1C",
        "RQ4_GLOBAL_HAWK" => "RQ-4",
        "MQ25_STINGRAY" => "MQ-25",
        _ => &entry.platform_type,
    };

    view! {
        <div class=format!("leaderboard-entry {}", rank_class)>
            <div class="leaderboard-rank">
                {entry.rank}
            </div>
            <div class="leaderboard-info">
                <div class="leaderboard-callsign">
                    {entry.callsign.clone()}
                    {rank_change_view}
                </div>
                <div class="leaderboard-platform">{platform_short.to_string()}</div>
            </div>
            <div class="leaderboard-stats">
                <div class="leaderboard-accuracy">
                    {format!("{:.1}%", entry.accuracy_pct)}
                </div>
                <div class="leaderboard-record">
                    {entry.successful_hits}"/"{ entry.total_engagements}" • 🔥"{entry.current_streak}
                </div>
            </div>
        </div>
    }
}

/// Loading skeleton for leaderboard
#[component]
pub fn LeaderboardSkeleton() -> impl IntoView {
    view! {
        <div class="panel">
            <div class="panel-header">
                <span class="panel-title">"ACCURACY LEADERBOARD"</span>
            </div>
            <div class="panel-body no-padding">
                <div class="leaderboard">
                    {(0..5).map(|_| view! {
                        <div class="leaderboard-entry">
                            <div class="skeleton" style="width: 32px; height: 24px;"></div>
                            <div class="leaderboard-info">
                                <div class="skeleton" style="width: 100px; height: 16px;"></div>
                                <div class="skeleton" style="width: 60px; height: 12px; margin-top: 4px;"></div>
                            </div>
                            <div class="leaderboard-stats">
                                <div class="skeleton" style="width: 50px; height: 20px;"></div>
                            </div>
                        </div>
                    }).collect::<Vec<_>>()}
                </div>
            </div>
        </div>
    }
}
