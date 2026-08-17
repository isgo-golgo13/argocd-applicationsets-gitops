//! Drone Convoy Simulator CLI
//!
//! Simulates drone convoy operations and posts EVERYTHING to the GraphQL API:
//! convoy registration, waypoint routes, per-tick drone state, telemetry, and
//! engagements. If a dashboard panel exists, this binary feeds it.
//!
//! Startup sequence (bootstrap):
//!   1. `createConvoy`     — with the well-known demo convoy id
//!   2. `createWaypoints`  — the full planned route, per drone
//!   3. `updateDroneState` — registers each drone (callsign, platform, route)
//!
//! Per tick:
//!   `recordTelemetry` + `updateDroneState` per drone, `recordEngagement` as
//!   they occur, and `updateConvoyStatus` on status transitions.

use anyhow::Result;
use clap::Parser;
use drone_simulator::convoy::ConvoyStatus;
use drone_simulator::ConvoySimulator;
use reqwest::Client;
use serde_json::{json, Value};
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

    /// Tactical theater to fly (also DRONE_THEATER): afghanistan | syria |
    /// libya | pakistan | iran | iraq. The convoy follows that theater's
    /// published route — the same points the dashboard draws as pins — so
    /// posted positions land exactly on the track. Default: afghanistan.
    #[arg(long, env = "DRONE_THEATER", default_value = "afghanistan")]
    theater: String,

    /// API endpoint (also DRONE_API_URL). Point it at the Gateway when the
    /// stack runs in Kubernetes: https://drone.localtest.me/graphql
    #[arg(long, env = "DRONE_API_URL", default_value = "http://localhost:8080/graphql")]
    api_url: String,

    /// Run as a long-lived SERVICE (also DRONE_SERVICE=1): fly sorties back to
    /// back and obey tasking orders from the dashboard. Each sortie the
    /// convoy record's aor_name is read; if the dashboard retasked it (the
    /// THEATER selector -> retaskConvoy mutation), the NEXT sortie flies the
    /// new theater. Without this flag: one sortie in --theater, then exit
    /// (the original behavior, kept for dev).
    #[arg(long, env = "DRONE_SERVICE", default_value_t = false,
          value_parser = parse_bool_env, num_args = 0..=1, default_missing_value = "true")]
    service: bool,

    /// In service mode, how often (ticks) to check the tasking order MID-
    /// sortie. A change mid-sortie retasks immediately (the convoy is
    /// re-flown from the new theater's IP) rather than waiting for the end.
    #[arg(long, env = "DRONE_TASKING_POLL_TICKS", default_value = "3")]
    tasking_poll_ticks: u32,

    /// Accept self-signed TLS (also DRONE_INSECURE_TLS=1). For KinD, where the
    /// Gateway certificate comes from a self-signed ClusterIssuer. Never in prod.
    #[arg(long, env = "DRONE_INSECURE_TLS", default_value_t = false,
          value_parser = parse_bool_env, num_args = 0..=1, default_missing_value = "true")]
    insecure_tls: bool,

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

    let requested = drone_domain::TheaterId::from_slug(&args.theater).unwrap_or_else(|| {
        let valid: Vec<&str> = drone_domain::TheaterId::ALL.iter().map(|t| t.slug()).collect();
        warn!("unknown theater '{}' — valid: {} — flying afghanistan", args.theater, valid.join(", "));
        drone_domain::TheaterId::Afghanistan
    });
    let client = Client::builder()
        .danger_accept_invalid_certs(args.insecure_tls)
        .build()
        .expect("reqwest client");
    if args.insecure_tls {
        warn!("TLS certificate verification DISABLED (--insecure-tls) -- local KinD only");
    }
    info!("API: {}", args.api_url);
    info!("Tick: {}ms, Duration: {} ticks, Mode: {}", args.tick_ms, args.duration,
          if args.service { "SERVICE (obeys dashboard tasking)" } else { "single sortie" });

    if !args.dry_run {
        wait_for_api(&client, &args.api_url).await;
    }

    // Initial theater: SEED from --theater/DRONE_THEATER, deliberately NOT
    // from a stale record. A record left over from a previous run is not a
    // live order -- the operator's opening dashboard view is, and it issues
    // that order (retaskConvoy) as soon as it loads. Starting where the
    // dashboard defaults means the two agree on boot; any disagreement is
    // reconciled by that first order within a few ticks. Live tasking is
    // still read from the record from the first sortie onward.
    let mut theater = requested;

    loop {
        // Boot rule: the FIRST sortie seeds from --theater and ignores whatever
        // the record says -- a record left by yesterday's run is not a live
        // order. The dashboard's opening view IS a live order: it is issued on
        // page load and re-issued the moment drones appear, and the mid-sortie
        // tasking poll picks it up within a few ticks. So on boot the two
        // agree by default (both Afghanistan) and any disagreement heals in
        // seconds -- without a stale record ever winning. From the second
        // sortie on, the record is authoritative between sorties as before.
        info!("Theater: {} ({} waypoints)", theater.theater().label, theater.theater().route.len());
        let mut convoy = ConvoySimulator::new(&args.callsign, &args.mission, args.drones, theater);
        info!("Convoy ID: {}", convoy.convoy_id);

        if !args.dry_run {
            bootstrap(&client, &args, &convoy).await;
        }

        // Immediately after bootstrap, catch an order the dashboard issued
        // while we were starting up (its opening view). Zero-tick latency for
        // the common "make serve, open dashboard" flow.
        if args.service && !args.dry_run {
            if let Some(t) = read_tasking(&client, &args.api_url, convoy.convoy_id).await {
                if t != theater {
                    info!("Order on record at bootstrap: {} -> {} — re-flying", theater.theater().label, t.theater().label);
                    theater = t;
                    continue;
                }
            }
        }

        let outcome = fly_sortie(&client, &args, &mut convoy).await;

        match outcome {
            SortieOutcome::Complete => {
                if !args.service { break; }
                // Between sorties: obey whatever the dashboard ordered.
                if !args.dry_run {
                    if let Some(t) = read_tasking(&client, &args.api_url, convoy.convoy_id).await {
                        if t != theater { info!("RETASKED for next sortie: {} -> {}", theater.theater().label, t.theater().label); }
                        theater = t;
                    }
                }
                info!("Sortie complete — next sortie in {} theater", theater.theater().label);
                sleep(Duration::from_secs(2)).await;
            }
            SortieOutcome::Retasked(t) => {
                info!("RETASKED mid-sortie: {} -> {} — re-flying from the new IP", theater.theater().label, t.theater().label);
                theater = t;
            }
        }
    }

    Ok(())
}

/// How a sortie ended.
enum SortieOutcome {
    /// Flew to 100% (or single-sortie mode finished).
    Complete,
    /// The dashboard retasked the convoy mid-sortie; carries the new theater.
    Retasked(drone_domain::TheaterId),
}

/// Fly one sortie to completion, or until a tasking change is observed
/// (service mode). All per-tick posting lives here — unchanged from the
/// original mission loop, just factored so it can run repeatedly.
async fn fly_sortie(client: &Client, args: &Args, convoy: &mut ConvoySimulator) -> SortieOutcome {
    let progress_per_tick = 1.0 / args.duration as f64;
    let mut last_status = convoy.status;

    for tick in 0..args.duration {
        // Advance mission
        convoy.advance(progress_per_tick);
        let state = convoy.state();

        // Convoy status transitions (Active -> Rtb -> Complete)
        if !args.dry_run && state.status != last_status {
            post_convoy_status(client, &args.api_url, convoy, state.status).await;
            last_status = state.status;
        }

        // Telemetry for every drone — recorded AND mirrored onto drone state,
        // so the cards, the map and the chart all move together.
        let telemetry = convoy.generate_telemetry();
        info!(
            "Tick {}/{} | Progress: {:.1}% | Status: {:?} | Telemetry: {} snapshots",
            tick + 1,
            args.duration,
            state.progress_pct,
            state.status,
            telemetry.len()
        );

        if !args.dry_run {
            for snap in &telemetry {
                post_telemetry(client, &args.api_url, snap).await;
                post_drone_state(client, &args.api_url, convoy, snap, state.progress_pct).await;
            }
        }

        // Simulate engagements
        let engagements = convoy.simulate_engagements();
        for e in &engagements {
            let result = if e.hit { "HIT" } else { "MISS" };
            info!(
                "  {} {} | {} | {} @ {:.1}km",
                e.callsign, result, e.weapon_type.as_str(), e.target_type.as_str(), e.range_km
            );

            if !args.dry_run {
                post_engagement(client, &args.api_url, e).await;
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

        // Service mode: obey a mid-sortie tasking change promptly.
        if args.service && !args.dry_run && args.tasking_poll_ticks > 0
            && tick % args.tasking_poll_ticks == 0
        {
            if let Some(t) = read_tasking(client, &args.api_url, convoy.convoy_id).await {
                if t != convoy.theater {
                    return SortieOutcome::Retasked(t);
                }
            }
        }

        sleep(Duration::from_millis(args.tick_ms)).await;
    }

    info!("Mission complete!");

    if !args.dry_run {
        post_convoy_status(client, &args.api_url, convoy, ConvoyStatus::Complete).await;
    }

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

    SortieOutcome::Complete
}

/// Read the tasking order: the convoy record's `aorName`, parsed as a theater
/// slug. `None` if the convoy doesn't exist yet or the name isn't a known
/// theater (e.g. the legacy "Kandahar Province"), in which case the caller
/// keeps flying what it has. This is the ONLY input the dashboard has on the
/// simulator -- and it's the same input a live ground station would read.
async fn read_tasking(client: &Client, api_url: &str, convoy_id: uuid::Uuid) -> Option<drone_domain::TheaterId> {
    let body = json!({
        "query": "query Tasking($id: ID!) { convoy(convoyId: $id) { aorName } }",
        "variables": { "id": convoy_id.to_string() }
    });
    let resp = client.post(api_url).json(&body).send().await.ok()?;
    let v: Value = resp.json().await.ok()?;
    let name = v.pointer("/data/convoy/aorName")?.as_str()?;
    drone_domain::TheaterId::from_slug(name)
}

/// Lenient boolean for env-backed flags: true/false, 1/0, yes/no, on/off.
/// clap's default bool parser accepts only "true"/"false", which is a
/// surprising trap for `DRONE_SERVICE=1` in a Makefile or shell.
fn parse_bool_env(s: &str) -> std::result::Result<bool, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" | "" => Ok(false),
        other => Err(format!("expected true/false (or 1/0, yes/no, on/off), got '{other}'")),
    }
}

// =============================================================================
// BOOTSTRAP
// =============================================================================

/// Block until the GraphQL endpoint answers a trivial query, up to ~3 minutes.
///
/// Probes the same URL the mutations use (no URL surgery, no separate health
/// path to misconfigure). On timeout the simulator proceeds anyway — the
/// per-tick posts self-heal identity — but says so loudly.
async fn wait_for_api(client: &Client, api_url: &str) {
    const ATTEMPTS: u32 = 180;
    for attempt in 1..=ATTEMPTS {
        let probe = client
            .post(api_url)
            .json(&json!({ "query": "{ health }" }))
            .send()
            .await;
        if let Ok(resp) = probe {
            if resp.status().is_success() {
                if attempt > 1 {
                    info!("API ready after {attempt}s");
                }
                return;
            }
        }
        if attempt % 5 == 1 {
            info!("waiting for API at {api_url} ({attempt}/{ATTEMPTS})...");
        }
        sleep(Duration::from_secs(1)).await;
    }
    warn!("API not reachable after {ATTEMPTS}s — starting anyway; bootstrap posts may be lost");
}

/// Register convoy, waypoint routes and drone identities with the API.
///
/// Failures are logged and skipped — the simulator keeps flying so a partial
/// registration still produces a partially-live dashboard rather than nothing.
async fn bootstrap(client: &Client, args: &Args, convoy: &ConvoySimulator) {
    // 1. Convoy, pinned to the well-known id so the dashboard's queries, the
    //    simulator's writes and the leaderboard reads all agree.
    let query = r#"
        mutation CreateConvoy($input: CreateConvoyInput!) {
            createConvoy(input: $input) { convoyId }
        }
    "#;
    let variables = json!({
        "input": {
            "convoyId": convoy.convoy_id.to_string(),
            "callsign": format!("{}-CONVOY", args.callsign),
            "missionType": args.mission.to_uppercase(),
            // The theater slug IS the tasking vocabulary (see read_tasking):
            // the dashboard writes it via retaskConvoy, we read it back.
            "aorName": convoy.theater.slug(),
            "aorCenter": {
                "latitude": convoy.theater.theater().center.0,
                "longitude": convoy.theater.theater().center.1,
                "altitudeM": 1000.0
            },
            "aorRadiusKm": convoy.theater.theater().aor_radius_m / 1000.0,
            "commandingUnit": "432nd Wing",
            "roeProfile": "STANDARD"
        }
    });
    post_graphql(client, &args.api_url, "createConvoy", query, variables).await;

    // 2. Waypoint routes + 3. drone registration
    for (idx, drone) in convoy.drones.values().enumerate() {
        let waypoints: Vec<Value> = drone
            .waypoints
            .iter()
            .map(|w| {
                json!({
                    "sequenceNumber": w.sequence as i32,
                    "name": w.name,
                    "waypointType": waypoint_type_wire(w.waypoint_type),
                    "coordinates": {
                        "latitude": w.coordinates.latitude,
                        "longitude": w.coordinates.longitude,
                        "altitudeM": w.coordinates.altitude_m,
                        "headingDeg": f64::from(w.coordinates.heading_deg),
                        "speedMps": f64::from(w.coordinates.speed_mps)
                    }
                })
            })
            .collect();

        let query = r#"
            mutation CreateWaypoints($input: CreateWaypointsInput!) {
                createWaypoints(input: $input) { sequenceNumber }
            }
        "#;
        let variables = json!({
            "input": { "droneId": drone.drone_id.to_string(), "waypoints": waypoints }
        });
        post_graphql(client, &args.api_url, "createWaypoints", query, variables).await;

        let first = drone.waypoints.first();
        let query = r#"
            mutation RegisterDrone($input: UpdateDroneStateInput!) {
                updateDroneState(input: $input) { droneId }
            }
        "#;
        let variables = json!({
            "input": {
                "convoyId": convoy.convoy_id.to_string(),
                "droneId": drone.drone_id.to_string(),
                "callsign": drone.callsign,
                "tailNumber": format!("AF-{:03}", idx + 1),
                "platformType": drone.platform_type,
                "status": "AIRBORNE",
                "fuelPct": 100.0,
                "currentWaypoint": 0,
                "totalWaypoints": drone.waypoints.len() as i32,
                "position": first.map(|w| json!({
                    "latitude": w.coordinates.latitude,
                    "longitude": w.coordinates.longitude,
                    "altitudeM": w.coordinates.altitude_m
                }))
            }
        });
        post_graphql(client, &args.api_url, "updateDroneState", query, variables).await;
    }

    info!(
        "Bootstrap complete: convoy + {} drone routes registered",
        convoy.drones.len()
    );
}

// =============================================================================
// PER-TICK POSTS
// =============================================================================

/// Post one telemetry snapshot.
async fn post_telemetry(
    client: &Client,
    api_url: &str,
    snap: &drone_simulator::telemetry::TelemetrySnapshot,
) {
    let query = r#"
        mutation RecordTelemetry($input: CreateTelemetryInput!) {
            recordTelemetry(input: $input) { droneId }
        }
    "#;
    let mesh = (f64::from(snap.signal_strength_dbm) + 100.0) / 50.0;
    let variables = json!({
        "input": {
            "droneId": snap.drone_id.to_string(),
            "position": {
                "latitude": snap.position.latitude,
                "longitude": snap.position.longitude,
                "altitudeM": snap.position.altitude_m,
                "headingDeg": f64::from(snap.position.heading_deg),
                "speedMps": f64::from(snap.position.speed_mps)
            },
            "fuelPct": f64::from(snap.fuel_remaining_pct),
            "currentWaypoint": snap.current_waypoint as i32,
            "velocityMps": f64::from(snap.ground_speed_mps),
            "meshConnectivity": mesh.clamp(0.0, 1.0)
        }
    });
    post_graphql(client, api_url, "recordTelemetry", query, variables).await;
}

/// Mirror the snapshot onto the drone row, with a status derived from
/// mission progress: AIRBORNE → INGRESS → EGRESS → RTB.
///
/// Identity (callsign, platform, route length) rides on EVERY tick, not just
/// bootstrap. `updateDroneState` is a read-merge-write upsert server-side, so
/// this is idempotent — and it means a drone whose registration was lost
/// (API race, restart, packet into the void) heals within one tick instead
/// of flying as UNKNOWN forever.
async fn post_drone_state(
    client: &Client,
    api_url: &str,
    convoy: &ConvoySimulator,
    snap: &drone_simulator::telemetry::TelemetrySnapshot,
    progress_pct: f32,
) {
    let status = match progress_pct {
        p if p < 10.0 => "AIRBORNE",
        p if p < 50.0 => "INGRESS",
        p if p < 90.0 => "EGRESS",
        _ => "RTB",
    };

    let identity = convoy.drones.get(&snap.drone_id);

    let query = r#"
        mutation UpdateDroneState($input: UpdateDroneStateInput!) {
            updateDroneState(input: $input) { droneId }
        }
    "#;
    let variables = json!({
        "input": {
            "convoyId": convoy.convoy_id.to_string(),
            "droneId": snap.drone_id.to_string(),
            "status": status,
            "fuelPct": f64::from(snap.fuel_remaining_pct),
            "currentWaypoint": snap.current_waypoint as i32,
            "callsign": identity.map(|d| d.callsign.clone()),
            "platformType": identity.map(|d| d.platform_type.clone()),
            "totalWaypoints": identity.map(|d| d.waypoints.len() as i32),
            "position": {
                "latitude": snap.position.latitude,
                "longitude": snap.position.longitude,
                "altitudeM": snap.position.altitude_m,
                "headingDeg": f64::from(snap.position.heading_deg),
                "speedMps": f64::from(snap.position.speed_mps)
            }
        }
    });
    post_graphql(client, api_url, "updateDroneState", query, variables).await;
}

/// Post a convoy status transition.
async fn post_convoy_status(
    client: &Client,
    api_url: &str,
    convoy: &ConvoySimulator,
    status: ConvoyStatus,
) {
    let wire = match status {
        ConvoyStatus::Planning => "PLANNING",
        ConvoyStatus::Active => "ACTIVE",
        ConvoyStatus::Rtb => "RTB",
        ConvoyStatus::Complete => "COMPLETE",
        ConvoyStatus::Abort => "ABORT",
    };
    let query = r#"
        mutation UpdateConvoyStatus($input: UpdateConvoyStatusInput!) {
            updateConvoyStatus(input: $input) { convoyId status }
        }
    "#;
    let variables = json!({
        "input": { "convoyId": convoy.convoy_id.to_string(), "status": wire }
    });
    post_graphql(client, api_url, "updateConvoyStatus", query, variables).await;
}

/// Post engagement to GraphQL API.
async fn post_engagement(
    client: &Client,
    api_url: &str,
    engagement: &drone_simulator::engagement::SimulatedEngagement,
) {
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
            "rangeKm": engagement.range_km,
            "callsign": engagement.callsign
        }
    });

    post_graphql(client, api_url, "recordEngagement", query, variables).await;
}

// =============================================================================
// TRANSPORT
// =============================================================================

/// Map the simulator's waypoint taxonomy onto the schema's.
fn waypoint_type_wire(t: drone_simulator::flight::WaypointType) -> &'static str {
    use drone_simulator::flight::WaypointType as W;
    match t {
        W::Takeoff | W::Navigation | W::Rtb => "NAV",
        W::Loiter => "LOITER",
        W::Target => "STRIKE",
        W::Landing => "CHECKPOINT",
    }
}

/// POST one GraphQL operation and surface every way it can fail.
///
/// GraphQL reports errors in a 200 OK body. Checking only the HTTP status
/// means every rejected mutation looks like a success and the dashboard
/// stays silently empty — which is exactly what happened. Any new caller
/// goes through this function.
async fn post_graphql(client: &Client, api_url: &str, op: &str, query: &str, variables: Value) {
    let response = match client
        .post(api_url)
        .json(&json!({ "query": query, "variables": variables }))
        .send()
        .await
    {
        Ok(r) => r,
        Err(err) => {
            warn!("{op}: request failed: {err}");
            return;
        }
    };

    let status = response.status();
    let body: Value = match response.json().await {
        Ok(b) => b,
        Err(err) => {
            warn!("{op}: unreadable response: {err}");
            return;
        }
    };

    if !status.is_success() {
        warn!("{op}: API returned status {status}: {body}");
        return;
    }

    if let Some(errors) = body.get("errors").and_then(|e| e.as_array()) {
        for err in errors {
            let message = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown GraphQL error");
            warn!("{op} rejected: {message}");
        }
    }
}
