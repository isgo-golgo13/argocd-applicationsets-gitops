//! # API Client
//!
//! GraphQL HTTP client for queries and mutations.

use crate::state::LeaderboardEntry;
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
    #[derive(Serialize)]
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
