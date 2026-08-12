//! Drone Convoy Simulator CLI
//!
//! Simulates drone telemetry and engagements, posting to GraphQL API.

use anyhow::Result;
use clap::Parser;
use drone_simulator::ConvoySimulator;
use reqwest::Client;
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "drone-simulator")]
#[command(about = "Simulate drone convoy operations")]
struct Args {
    /// Convoy callsign
    #[arg(short, long, default_value = "ALPHA")]
    callsign: String,

    /// Mission type
    #[arg(short, long, default_value = "STRIKE")]
    mission: String,

    /// Number of drones
    #[arg(short, long, default_value = "4")]
    drones: usize,

    /// API endpoint
    #[arg(long, default_value = "http://localhost:8080/graphql")]
    api_url: String,

    /// Tick interval in milliseconds
    #[arg(long, default_value = "1000")]
    tick_ms: u64,

    /// Total mission duration in ticks
    #[arg(long, default_value = "300")]
    duration: u32,

    /// Dry run (don't post to API)
    #[arg(long)]
    dry_run: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("drone_simulator=info".parse()?))
        .init();

    let args = Args::parse();

    info!(
        "Starting convoy simulation: {} ({} drones, {} mission)",
        args.callsign, args.drones, args.mission
    );

    let mut convoy = ConvoySimulator::new(&args.callsign, &args.mission, args.drones);
    let client = Client::new();
    let progress_per_tick = 1.0 / args.duration as f64;

    info!("Convoy ID: {}", convoy.convoy_id);
    info!("API: {}", args.api_url);
    info!("Tick: {}ms, Duration: {} ticks", args.tick_ms, args.duration);

    for tick in 0..args.duration {
        // Advance mission
        convoy.advance(progress_per_tick);
        let state = convoy.state();

        // Generate telemetry
        let telemetry = convoy.generate_telemetry();
        info!(
            "Tick {}/{} | Progress: {:.1}% | Status: {:?} | Telemetry: {} snapshots",
            tick + 1,
            args.duration,
            state.progress_pct,
            state.status,
            telemetry.len()
        );

        // Simulate engagements
        let engagements = convoy.simulate_engagements();
        if !engagements.is_empty() {
            for e in &engagements {
                let result = if e.hit { "HIT" } else { "MISS" };
                info!(
                    "  {} {} | {} | {} @ {:.1}km",
                    e.callsign, result, e.weapon_type.as_str(), e.target_type.as_str(), e.range_km
                );

                // Post engagement to API
                if !args.dry_run {
                    if let Err(err) = post_engagement(&client, &args.api_url, e).await {
                        warn!("Failed to post engagement: {}", err);
                    }
                }
            }
        }

        // Show leaderboard periodically
        if tick % 30 == 0 && tick > 0 {
            let leaderboard = convoy.leaderboard();
            info!("--- LEADERBOARD ---");
            for entry in leaderboard.iter().take(5) {
                info!(
                    "  #{} {} - {:.1}% ({}/{})",
                    entry.rank, entry.callsign, entry.accuracy_pct,
                    entry.successful_hits, entry.total_engagements
                );
            }
        }

        sleep(Duration::from_millis(args.tick_ms)).await;
    }

    info!("Mission complete!");

    // Final leaderboard
    let leaderboard = convoy.leaderboard();
    info!("=== FINAL LEADERBOARD ===");
    for entry in &leaderboard {
        info!(
            "#{} {} ({}) - {:.1}% ({}/{} engagements)",
            entry.rank,
            entry.callsign,
            entry.platform_type,
            entry.accuracy_pct,
            entry.successful_hits,
            entry.total_engagements
        );
    }

    Ok(())
}

/// Post engagement to GraphQL API.
async fn post_engagement(
    client: &Client,
    api_url: &str,
    engagement: &drone_simulator::engagement::SimulatedEngagement,
) -> Result<()> {
    let query = r#"
        mutation RecordEngagement($input: RecordEngagementInput!) {
            recordEngagement(input: $input) {
                success
                newRank
                rankChange
                newAccuracyPct
            }
        }
    "#;

    let variables = json!({
        "input": {
            "convoyId": engagement.convoy_id.to_string(),
            "droneId": engagement.drone_id.to_string(),
            "hit": engagement.hit,
            "weaponType": engagement.weapon_type.as_str(),
            "targetType": engagement.target_type.as_str(),
            "rangeKm": engagement.range_km
        }
    });

    let response = client
        .post(api_url)
        .json(&json!({
            "query": query,
            "variables": variables
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        warn!("API returned status: {}", response.status());
    }

    Ok(())
}
