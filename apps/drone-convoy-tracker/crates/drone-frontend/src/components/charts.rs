//! # Telemetry Chart Component
//!
//! Real-time charts using Charming (ECharts wrapper).

use charming::{
    component::{Axis, Grid, Legend},
    element::{AreaStyle, AxisType, LineStyle, Tooltip, Trigger},
    series::Line,
    Chart, WasmRenderer,
};
use leptos::prelude::*;

use crate::state::use_app_state;

/// Telemetry chart panel
///
/// Renders the rolling convoy-average series from `state.telemetry_series`.
/// Reading the signal inside the Effect makes the render reactive: every
/// poll tick that appends a point re-renders the chart. Charming's WASM
/// renderer redraws in place on the same element id.
#[component]
pub fn TelemetryChartPanel() -> impl IntoView {
    let state = use_app_state();
    let chart_id = "telemetry-chart";

    // The Echarts handle from the first render. Subsequent series changes go
    // through WasmRenderer::update — calling render() again would echarts-init
    // the same DOM node repeatedly and stack instances on every poll tick.
    let echarts: std::rc::Rc<std::cell::RefCell<Option<charming::Echarts>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));

    // Rebuild chart whenever the series changes
    Effect::new(move |_| {
        let series = state.telemetry_series.get();
        if series.len() < 2 {
            // Nothing meaningful to plot yet; leave the container empty
            // rather than rendering a two-axis chart with no lines.
            return;
        }

        let labels: Vec<String> = series.iter().map(|p| p.label.clone()).collect();
        let altitude_data: Vec<f64> = series.iter().map(|p| p.avg_altitude_m).collect();
        let fuel_data: Vec<f64> = series.iter().map(|p| p.avg_fuel_pct).collect();

        // No in-chart title: the panel header already reads FLIGHT TELEMETRY.
        // Two labels for one panel looked odd, and the freed strip lets the
        // plot breathe. Chart title stays out; grid top tightened to match.
        let chart = Chart::new()
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
                    .top("8%")       // title strip gone -> reclaim it for the plot
                    .bottom("20%"),
            )
            .x_axis(
                Axis::new()
                    .type_(AxisType::Category)
                    .data(labels)
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
                    .data(altitude_data)
                    .smooth(true)
                    .line_style(LineStyle::new().color("#00ff41").width(2))
                    .area_style(AreaStyle::new().color("rgba(0, 255, 65, 0.1)")),
            )
            .series(
                Line::new()
                    .name("Fuel (%)")
                    .data(fuel_data)
                    .smooth(true)
                    .line_style(LineStyle::new().color("#ffaa00").width(2))
                    .area_style(AreaStyle::new().color("rgba(255, 170, 0, 0.1)")),
            );

        let mut handle = echarts.borrow_mut();
        if let Some(instance) = handle.as_ref() {
            WasmRenderer::update(instance, &chart);
        } else {
            match WasmRenderer::new(400, 200).render(chart_id, &chart) {
                Ok(instance) => *handle = Some(instance),
                Err(e) => log::error!("Chart render error: {:?}", e),
            }
        }
    });

    view! {
        <div class="panel">
            <div class="panel-header">
                <span class="panel-title">"FLIGHT TELEMETRY"</span>
                {move || (state.telemetry_series.get().len() >= 2).then(|| view! {
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
        let airborne = drones.values().filter(|d| d.status.is_airborne()).count();
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
