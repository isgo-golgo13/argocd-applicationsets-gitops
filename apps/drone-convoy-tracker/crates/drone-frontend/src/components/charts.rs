//! # Telemetry Chart Component
//!
//! Real-time charts using Charming (ECharts wrapper).

use charming::{
    component::{Axis, Grid, Legend, Title},
    element::{AreaStyle, AxisType, LineStyle, Tooltip, Trigger},
    series::Line,
    Chart, WasmRenderer,
};
use leptos::prelude::*;

use crate::state::use_app_state;

/// Telemetry chart panel
#[component]
pub fn TelemetryChartPanel() -> impl IntoView {
    let state = use_app_state();
    let chart_id = "telemetry-chart";

    // Sample data for demonstration
    let altitude_data: Vec<f64> = vec![
        4800.0, 4900.0, 5000.0, 5100.0, 5050.0, 5000.0, 4950.0, 5000.0,
        5100.0, 5200.0, 5150.0, 5100.0, 5000.0, 4900.0, 5000.0, 5100.0,
    ];

    let fuel_data: Vec<f64> = vec![
        95.0, 93.0, 91.0, 89.0, 87.0, 85.0, 83.0, 81.0,
        79.0, 77.0, 75.0, 73.0, 71.0, 69.0, 67.0, 65.0,
    ];

    let waypoints: Vec<String> = (1..=16).map(|i| format!("WP{}", i)).collect();

    // Build chart on mount
    Effect::new(move |_| {
        let chart = Chart::new()
            .title(
                Title::new()
                    .text("FLIGHT TELEMETRY")
                    .text_style(charming::element::TextStyle::new().color("#00ff41").font_size(12))
                    .left("center"),
            )
            .tooltip(Tooltip::new().trigger(Trigger::Axis))
            .legend(
                Legend::new()
                    .data(vec!["Altitude (m)", "Fuel (%)"])
                    .text_style(charming::element::TextStyle::new().color("#99cc99"))
                    .bottom(0),
            )
            .grid(
                Grid::new()
                    .left("10%")
                    .right("10%")
                    .top("15%")
                    .bottom("20%"),
            )
            .x_axis(
                Axis::new()
                    .type_(AxisType::Category)
                    .data(waypoints.clone())
                    .axis_line(charming::element::AxisLine::new().line_style((1.0, "#557755")))
                    .axis_label(charming::element::AxisLabel::new().color("#557755")),
            )
            .y_axis(
                Axis::new()
                    .type_(AxisType::Value)
                    .name("Altitude (m)")
                    .axis_line(charming::element::AxisLine::new().line_style((1.0, "#557755")))
                    .axis_label(charming::element::AxisLabel::new().color("#557755"))
                    .split_line(charming::element::SplitLine::new().line_style(LineStyle::new().color("#1a2a1a"))),
            )
            .series(
                Line::new()
                    .name("Altitude (m)")
                    .data(altitude_data.clone())
                    .smooth(true)
                    .line_style(LineStyle::new().color("#00ff41").width(2))
                    .area_style(AreaStyle::new().color("rgba(0, 255, 65, 0.1)")),
            )
            .series(
                Line::new()
                    .name("Fuel (%)")
                    .data(fuel_data.clone())
                    .smooth(true)
                    .line_style(LineStyle::new().color("#ffaa00").width(2))
                    .area_style(AreaStyle::new().color("rgba(255, 170, 0, 0.1)")),
            );

        let renderer = WasmRenderer::new(400, 200);
        if let Err(e) = renderer.render(chart_id, &chart) {
            log::error!("Chart render error: {:?}", e);
        }
    });

    view! {
        <div class="panel">
            <div class="panel-header">
                <span class="panel-title">"TELEMETRY"</span>
                {move || state.selected_drone.get().map(|_| view! {
                    <span class="panel-badge">"LIVE"</span>
                })}
            </div>
            <div class="panel-body no-padding">
                <div id=chart_id class="chart-container"></div>
            </div>
        </div>
    }
}

/// Stats summary panel
#[component]
pub fn ConvoyStatsPanel() -> impl IntoView {
    let state = use_app_state();

    let stats = move || {
        let drones = state.drones.get();
        let leaderboard = state.leaderboard.get();

        let total = drones.len();
        let airborne = drones.values().filter(|d| d.status.status_class() == "nominal").count();
        let avg_fuel: f32 = if total > 0 {
            drones.values().map(|d| d.fuel_pct).sum::<f32>() / total as f32
        } else {
            0.0
        };
        let avg_accuracy: f32 = if !leaderboard.is_empty() {
            leaderboard.iter().map(|e| e.accuracy_pct).sum::<f32>() / leaderboard.len() as f32
        } else {
            0.0
        };
        let total_engagements: u32 = leaderboard.iter().map(|e| e.total_engagements).sum();
        let total_hits: u32 = leaderboard.iter().map(|e| e.successful_hits).sum();

        (total, airborne, avg_fuel, avg_accuracy, total_engagements, total_hits)
    };

    view! {
        <div class="panel">
            <div class="panel-header">
                <span class="panel-title">"CONVOY STATUS"</span>
            </div>
            <div class="panel-body">
                <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 16px;">
                    <div>
                        <div class="text-xs text-muted uppercase tracking-wide">"ASSETS"</div>
                        <div class="text-xl font-bold text-accent">
                            {move || stats().1}"/"{ move || stats().0}
                        </div>
                        <div class="text-xs text-muted">"airborne"</div>
                    </div>
                    <div>
                        <div class="text-xs text-muted uppercase tracking-wide">"AVG FUEL"</div>
                        <div class="text-xl font-bold" class:text-warning=move || stats().2 < 40.0>
                            {move || format!("{:.0}%", stats().2)}
                        </div>
                        <div class="text-xs text-muted">"remaining"</div>
                    </div>
                    <div>
                        <div class="text-xs text-muted uppercase tracking-wide">"ACCURACY"</div>
                        <div class="text-xl font-bold text-accent">
                            {move || format!("{:.1}%", stats().3)}
                        </div>
                        <div class="text-xs text-muted">"convoy avg"</div>
                    </div>
                    <div>
                        <div class="text-xs text-muted uppercase tracking-wide">"ENGAGEMENTS"</div>
                        <div class="text-xl font-bold">
                            {move || stats().5}"/"{ move || stats().4}
                        </div>
                        <div class="text-xs text-muted">"hits/total"</div>
                    </div>
                </div>
            </div>
        </div>
    }
}
