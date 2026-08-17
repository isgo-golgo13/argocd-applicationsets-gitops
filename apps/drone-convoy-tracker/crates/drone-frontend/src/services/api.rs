//! # API Client
//!
//! GraphQL HTTP client for queries and mutations.

use crate::state::{Coordinates, DroneState, DroneStatus, EngagementEvent, LeaderboardEntry};
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const API_URL: &str = "http://localhost:8080/graphql";

#[derive(Serialize)]
struct GraphQLRequest<V: Serialize> {
    query: &'static str,
    variables: V,
}

#[derive(Deserialize, Debug)]
struct GraphQLResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Deserialize, Debug)]
struct GraphQLError {
    message: String,
}

/// Fetch leaderboard for a convoy
pub async fn fetch_leaderboard(
    convoy_id: Uuid,
    limit: u32,
) -> Result<Vec<LeaderboardEntry>, String> {
    // The query declares $convoyId. Without the rename this serialises as
    // "convoy_id" and the server rejects the variables on every single poll.
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Variables {
        convoy_id: String,
        limit: u32,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct LeaderboardResponse {
        leaderboard: LeaderboardData,
    }

    #[derive(Deserialize)]
    struct LeaderboardData {
        entries: Vec<LeaderboardEntryData>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct LeaderboardEntryData {
        drone_id: String,
        callsign: String,
        platform_type: String,
        rank: u32,
        accuracy_pct: f32,
        total_engagements: u32,
        successful_hits: u32,
        current_streak: i32,
        best_streak: i32,
    }

    let request = GraphQLRequest {
        query: r#"
            query GetLeaderboard($convoyId: ID!, $limit: Int!) {
                leaderboard(convoyId: $convoyId, limit: $limit) {
                    entries {
                        droneId
                        callsign
                        platformType
                        rank
                        accuracyPct
                        totalEngagements
                        successfulHits
                        currentStreak
                        bestStreak
                    }
                }
            }
        "#,
        variables: Variables {
            convoy_id: convoy_id.to_string(),
            limit,
        },
    };

    let response = Request::post(API_URL)
        .header("Content-Type", "application/json")
        .json(&request)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let result: GraphQLResponse<LeaderboardResponse> = response
        .json()
        .await
        .map_err(|e| e.to_string())?;

    if let Some(errors) = result.errors {
        return Err(errors.into_iter().map(|e| e.message).collect::<Vec<_>>().join(", "));
    }

    let data = result.data.ok_or("No data in response")?;
    
    Ok(data.leaderboard.entries.into_iter().map(|e| LeaderboardEntry {
        drone_id: Uuid::parse_str(&e.drone_id).unwrap_or_default(),
        callsign: e.callsign,
        platform_type: e.platform_type,
        rank: e.rank,
        accuracy_pct: e.accuracy_pct,
        total_engagements: e.total_engagements,
        successful_hits: e.successful_hits,
        current_streak: e.current_streak,
        best_streak: e.best_streak,
        rank_change: 0,
    }).collect())
}

/// Record an engagement
pub async fn record_engagement(
    convoy_id: Uuid,
    drone_id: Uuid,
    hit: bool,
    weapon_type: &str,
) -> Result<RecordEngagementResult, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Variables {
        input: RecordEngagementInput,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct RecordEngagementInput {
        convoy_id: String,
        drone_id: String,
        hit: bool,
        weapon_type: String,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Response {
        record_engagement: RecordEngagementResult,
    }

    let request = GraphQLRequest {
        query: r#"
            mutation RecordEngagement($input: RecordEngagementInput!) {
                recordEngagement(input: $input) {
                    success
                    newRank
                    rankChange
                    newAccuracyPct
                }
            }
        "#,
        variables: Variables {
            input: RecordEngagementInput {
                convoy_id: convoy_id.to_string(),
                drone_id: drone_id.to_string(),
                hit,
                weapon_type: weapon_type.to_string(),
            },
        },
    };

    let response = Request::post(API_URL)
        .header("Content-Type", "application/json")
        .json(&request)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let result: GraphQLResponse<Response> = response
        .json()
        .await
        .map_err(|e| e.to_string())?;

    if let Some(errors) = result.errors {
        return Err(errors.into_iter().map(|e| e.message).collect::<Vec<_>>().join(", "));
    }

    result.data.map(|d| d.record_engagement).ok_or("No data".to_string())
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RecordEngagementResult {
    pub success: bool,
    pub new_rank: i32,
    pub rank_change: i32,
    pub new_accuracy_pct: f32,
}

/// Fetch active convoys
pub async fn fetch_active_convoys() -> Result<Vec<ConvoySummary>, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Response {
        active_convoys: Vec<ConvoySummary>,
    }

    let request = GraphQLRequest {
        query: r#"
            query GetActiveConvoys {
                activeConvoys {
                    convoyId
                    callsign
                    missionType
                    status
                    droneCount
                }
            }
        "#,
        variables: (),
    };

    let response = Request::post(API_URL)
        .header("Content-Type", "application/json")
        .json(&request)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let result: GraphQLResponse<Response> = response
        .json()
        .await
        .map_err(|e| e.to_string())?;

    // Same lesson as everywhere else: GraphQL rejections arrive in a 200 OK
    // body. Skipping this check turns a schema error into a silent "No data".
    if let Some(errors) = result.errors {
        return Err(errors.into_iter().map(|e| e.message).collect::<Vec<_>>().join(", "));
    }

    result.data.map(|d| d.active_convoys).ok_or("No data".to_string())
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ConvoySummary {
    pub convoy_id: String,
    pub callsign: String,
    pub mission_type: String,
    pub status: String,
    pub drone_count: u32,
}

/// Fetch all drones in a convoy, mapped straight onto dashboard state.
///
/// The `status` field deserialises through `DroneStatus`'s
/// SCREAMING_SNAKE_CASE serde rename, so the GraphQL enum wire form
/// ("AIRBORNE", "RTB", ...) lands without a manual match.
pub async fn fetch_drones(convoy_id: Uuid) -> Result<Vec<DroneState>, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Variables {
        convoy_id: String,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Response {
        drones: DronesConnection,
    }

    #[derive(Deserialize)]
    struct DronesConnection {
        items: Vec<DroneData>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct DroneData {
        drone_id: String,
        convoy_id: String,
        tail_number: String,
        callsign: String,
        platform_type: String,
        status: DroneStatus,
        current_position: PositionData,
        fuel_remaining_pct: f32,
        accuracy_pct: f32,
        current_waypoint: u32,
        total_waypoints: u32,
        updated_at: chrono::DateTime<chrono::Utc>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PositionData {
        latitude: f64,
        longitude: f64,
        altitude_m: f64,
        heading_deg: f32,
        speed_mps: f32,
    }

    let request = GraphQLRequest {
        query: r#"
            query GetDrones($convoyId: ID!) {
                drones(convoyId: $convoyId, pagination: { limit: 50, offset: 0 }) {
                    items {
                        droneId
                        convoyId
                        tailNumber
                        callsign
                        platformType
                        status
                        currentPosition {
                            latitude
                            longitude
                            altitudeM
                            headingDeg
                            speedMps
                        }
                        fuelRemainingPct
                        accuracyPct
                        currentWaypoint
                        totalWaypoints
                        updatedAt
                    }
                }
            }
        "#,
        variables: Variables {
            convoy_id: convoy_id.to_string(),
        },
    };

    let response = Request::post(API_URL)
        .header("Content-Type", "application/json")
        .json(&request)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let result: GraphQLResponse<Response> = response
        .json()
        .await
        .map_err(|e| e.to_string())?;

    if let Some(errors) = result.errors {
        return Err(errors.into_iter().map(|e| e.message).collect::<Vec<_>>().join(", "));
    }

    let data = result.data.ok_or("No data in response")?;

    Ok(data
        .drones
        .items
        .into_iter()
        .map(|d| DroneState {
            drone_id: Uuid::parse_str(&d.drone_id).unwrap_or_default(),
            convoy_id: Uuid::parse_str(&d.convoy_id).unwrap_or_default(),
            callsign: d.callsign,
            tail_number: d.tail_number,
            platform_type: d.platform_type,
            status: d.status,
            position: Coordinates {
                latitude: d.current_position.latitude,
                longitude: d.current_position.longitude,
                altitude_m: d.current_position.altitude_m,
                heading_deg: d.current_position.heading_deg,
                speed_mps: d.current_position.speed_mps,
            },
            fuel_pct: d.fuel_remaining_pct,
            accuracy_pct: d.accuracy_pct,
            current_waypoint: d.current_waypoint,
            total_waypoints: d.total_waypoints,
            updated_at: d.updated_at,
        })
        .collect())
}

/// Fetch recent engagements for a convoy, newest first.
///
/// `new_accuracy_pct` is not part of the engagement record — the caller fills
/// it from the leaderboard so the feed shows the shooter's accuracy after the
/// shot, same as the subscription event would.
pub async fn fetch_engagements(convoy_id: Uuid, limit: u32) -> Result<Vec<EngagementEvent>, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Variables {
        convoy_id: String,
        limit: u32,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Response {
        engagements: EngagementsConnection,
    }

    #[derive(Deserialize)]
    struct EngagementsConnection {
        items: Vec<EngagementData>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct EngagementData {
        engagement_id: String,
        drone_id: String,
        drone_callsign: String,
        hit: bool,
        weapon_type: String,
        engaged_at: chrono::DateTime<chrono::Utc>,
    }

    let request = GraphQLRequest {
        query: r#"
            query GetEngagements($convoyId: ID!, $limit: Int!) {
                engagements(convoyId: $convoyId, pagination: { limit: $limit, offset: 0 }) {
                    items {
                        engagementId
                        droneId
                        droneCallsign
                        hit
                        weaponType
                        engagedAt
                    }
                }
            }
        "#,
        variables: Variables {
            convoy_id: convoy_id.to_string(),
            limit,
        },
    };

    let response = Request::post(API_URL)
        .header("Content-Type", "application/json")
        .json(&request)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let result: GraphQLResponse<Response> = response
        .json()
        .await
        .map_err(|e| e.to_string())?;

    if let Some(errors) = result.errors {
        return Err(errors.into_iter().map(|e| e.message).collect::<Vec<_>>().join(", "));
    }

    let data = result.data.ok_or("No data in response")?;

    Ok(data
        .engagements
        .items
        .into_iter()
        .map(|e| EngagementEvent {
            id: Uuid::parse_str(&e.engagement_id).unwrap_or_default(),
            drone_id: Uuid::parse_str(&e.drone_id).unwrap_or_default(),
            callsign: e.drone_callsign,
            hit: e.hit,
            weapon_type: e.weapon_type,
            new_accuracy_pct: 0.0,
            timestamp: e.engaged_at,
        })
        .collect())
}

/// Issue a tasking order: retask the convoy to a theater.
///
/// The dashboard's THEATER selector calls this. The convoy record becomes
/// the system of record; the simulator (or a live ground station) obeys it.
pub async fn retask_convoy(convoy_id: Uuid, theater_slug: &str) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Variables {
        convoy_id: String,
        theater: String,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Response {
        retask_convoy: RetaskResult,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RetaskResult {
        aor_name: String,
    }

    let request = GraphQLRequest {
        query: r#"
            mutation Retask($convoyId: String!, $theater: String!) {
                retaskConvoy(input: { convoyId: $convoyId, theater: $theater }) { aorName }
            }
        "#,
        variables: Variables { convoy_id: convoy_id.to_string(), theater: theater_slug.to_string() },
    };

    let response = Request::post(API_URL)
        .header("Content-Type", "application/json")
        .json(&request)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let result: GraphQLResponse<Response> = response.json().await.map_err(|e| e.to_string())?;
    if let Some(errors) = result.errors {
        return Err(errors.into_iter().map(|e| e.message).collect::<Vec<_>>().join(", "));
    }
    let data = result.data.ok_or("No data in response")?;
    log::info!("tasking order accepted: convoy now assigned to {}", data.retask_convoy.aor_name);
    Ok(())
}
