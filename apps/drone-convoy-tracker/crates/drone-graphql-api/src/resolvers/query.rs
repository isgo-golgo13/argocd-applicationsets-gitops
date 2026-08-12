//! # GraphQL Query Resolver
//!
//! Read operations for the drone convoy API.

use async_graphql::{Context, Object, Result, ID};
use chrono::Utc;
use uuid::Uuid;

use crate::context::ApiContext;
use crate::error::ApiError;
use crate::schema::*;

/// GraphQL Query root
pub struct QueryRoot;

#[Object]
impl QueryRoot {
    // =========================================================================
    // LEADERBOARD QUERIES
    // =========================================================================

    /// Get the accuracy leaderboard for a convoy
    ///
    /// Returns drones ranked by missile-to-target hit accuracy.
    /// Default limit is 10, maximum is 100.
    #[graphql(name = "leaderboard")]
    async fn get_leaderboard(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Convoy ID to get leaderboard for")]
        convoy_id: ID,
        #[graphql(default = 10, validator(maximum = 100), desc = "Maximum entries to return (default: 10, max: 100)")]
        limit: i32,
        #[graphql(desc = "Optional filter criteria")]
        filter: Option<LeaderboardFilter>,
    ) -> Result<Leaderboard> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let convoy_uuid = Uuid::parse_str(&convoy_id).map_err(ApiError::from)?;

        tracing::debug!(
            convoy_id = %convoy_uuid,
            limit = limit,
            "Fetching leaderboard"
        );

        let entries = api_ctx
            .leaderboard_repo
            .get_leaderboard(convoy_uuid, limit)
            .await
            .map_err(ApiError::from)?
            .into_iter()
            .map(LeaderboardEntry::from)
            .filter(|e| {
                // Apply filters if provided
                if let Some(ref f) = filter {
                    if let Some(min_acc) = f.min_accuracy {
                        if e.accuracy_pct < min_acc as f32 {
                            return false;
                        }
                    }
                    if let Some(min_eng) = f.min_engagements {
                        if e.total_engagements < min_eng {
                            return false;
                        }
                    }
                    if let Some(pt) = f.platform_type {
                        if e.platform_type != pt {
                            return false;
                        }
                    }
                }
                true
            })
            .collect();

        // TODO: Fetch actual convoy callsign from convoy repo
        let convoy_callsign = format!("CONVOY-{}", &convoy_id.as_str()[..8]);

        Ok(Leaderboard {
            convoy_id: convoy_id.to_string(),
            convoy_callsign,
            entries,
            generated_at: Utc::now(),
        })
    }

    /// Get a specific drone's rank and stats in the leaderboard
    #[graphql(name = "droneRank")]
    async fn get_drone_rank(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Convoy ID")]
        convoy_id: ID,
        #[graphql(desc = "Drone ID")]
        drone_id: ID,
    ) -> Result<Option<LeaderboardEntry>> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let convoy_uuid = Uuid::parse_str(&convoy_id).map_err(ApiError::from)?;
        let drone_uuid = Uuid::parse_str(&drone_id).map_err(ApiError::from)?;

        let entries = api_ctx
            .leaderboard_repo
            .get_leaderboard(convoy_uuid, 100)
            .await
            .map_err(ApiError::from)?;

        let entry = entries
            .into_iter()
            .find(|e| e.drone_id == drone_uuid)
            .map(LeaderboardEntry::from);

        Ok(entry)
    }

    // =========================================================================
    // CONVOY QUERIES
    // =========================================================================

    /// Get all active convoys
    #[graphql(name = "activeConvoys")]
    async fn get_active_convoys(&self, _ctx: &Context<'_>) -> Result<Vec<Convoy>> {
        // TODO: Implement with convoy repository
        // For now, return mock data
        Ok(vec![Convoy {
            convoy_id: ID::from("550e8400-e29b-41d4-a716-446655440000"),
            callsign: "ALPHA-CONVOY".to_string(),
            mission_type: MissionType::Strike,
            status: ConvoyStatus::Active,
            aor_name: "Kandahar Province".to_string(),
            aor_center: Coordinates {
                latitude: 31.6289,
                longitude: 65.7372,
                altitude_m: 1000.0,
                heading_deg: 0.0,
                speed_mps: 0.0,
            },
            aor_radius_km: 150.0,
            drone_count: 12,
            commanding_unit: "432nd Wing".to_string(),
            mission_start: Some(Utc::now()),
            mission_end: None,
            created_at: Utc::now(),
        }])
    }

    /// Get convoy details by ID
    #[graphql(name = "convoy")]
    async fn get_convoy(
        &self,
        _ctx: &Context<'_>,
        #[graphql(desc = "Convoy ID")]
        convoy_id: ID,
    ) -> Result<Option<Convoy>> {
        // TODO: Implement with convoy repository
        Ok(Some(Convoy {
            convoy_id: convoy_id.clone(),
            callsign: "ALPHA-CONVOY".to_string(),
            mission_type: MissionType::Strike,
            status: ConvoyStatus::Active,
            aor_name: "Kandahar Province".to_string(),
            aor_center: Coordinates {
                latitude: 31.6289,
                longitude: 65.7372,
                altitude_m: 1000.0,
                heading_deg: 0.0,
                speed_mps: 0.0,
            },
            aor_radius_km: 150.0,
            drone_count: 12,
            commanding_unit: "432nd Wing".to_string(),
            mission_start: Some(Utc::now()),
            mission_end: None,
            created_at: Utc::now(),
        }))
    }

    /// Get convoy statistics
    #[graphql(name = "convoyStats")]
    async fn get_convoy_stats(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Convoy ID")]
        convoy_id: ID,
    ) -> Result<ConvoyStats> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let convoy_uuid = Uuid::parse_str(&convoy_id).map_err(ApiError::from)?;

        // Calculate stats from leaderboard data
        let entries = api_ctx
            .leaderboard_repo
            .get_leaderboard(convoy_uuid, 100)
            .await
            .map_err(ApiError::from)?;

        let total_engagements: i32 = entries.iter().map(|e| e.total_engagements).sum();
        let total_hits: i32 = entries.iter().map(|e| e.successful_hits).sum();
        let avg_accuracy = if !entries.is_empty() {
            entries.iter().map(|e| e.accuracy_pct).sum::<f32>() / entries.len() as f32
        } else {
            0.0
        };

        Ok(ConvoyStats {
            convoy_id,
            drone_count: entries.len() as i32,
            airborne_count: entries.len() as i32, // TODO: Get from drone repo
            total_engagements,
            total_hits,
            average_accuracy_pct: avg_accuracy,
            average_fuel_pct: 75.0, // TODO: Get from drone repo
            timestamp: Utc::now(),
        })
    }

    // =========================================================================
    // DRONE QUERIES
    // =========================================================================

    /// Get drone details by ID
    #[graphql(name = "drone")]
    async fn get_drone(
        &self,
        _ctx: &Context<'_>,
        #[graphql(desc = "Convoy ID")]
        convoy_id: ID,
        #[graphql(desc = "Drone ID")]
        drone_id: ID,
    ) -> Result<Option<Drone>> {
        // TODO: Implement with drone repository
        Ok(Some(Drone {
            drone_id: drone_id.to_string(),
            convoy_id: convoy_id.to_string(),
            tail_number: "AF-001".to_string(),
            callsign: "REAPER-01".to_string(),
            platform_type: PlatformType::Mq9Reaper,
            status: DroneStatus::Airborne,
            current_position: Coordinates {
                latitude: 34.5553,
                longitude: 69.2075,
                altitude_m: 5000.0,
                heading_deg: 45.0,
                speed_mps: 80.0,
            },
            fuel_remaining_pct: 75.5,
            accuracy_pct: 92.3,
            total_engagements: 13,
            successful_hits: 12,
            current_waypoint: 15,
            total_waypoints: 25,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }))
    }

    /// Get all drones in a convoy
    #[graphql(name = "drones")]
    async fn get_drones(
        &self,
        _ctx: &Context<'_>,
        #[graphql(desc = "Convoy ID")]
        convoy_id: ID,
        #[graphql(desc = "Optional filter")]
        filter: Option<DroneFilter>,
        #[graphql(default, desc = "Pagination")]
        pagination: PaginationInput,
    ) -> Result<Connection<Drone>> {
        // TODO: Implement with drone repository
        let drones = vec![Drone {
            drone_id: Uuid::new_v4().to_string(),
            convoy_id: convoy_id.to_string(),
            tail_number: "AF-001".to_string(),
            callsign: "REAPER-01".to_string(),
            platform_type: PlatformType::Mq9Reaper,
            status: DroneStatus::Airborne,
            current_position: Coordinates {
                latitude: 34.5553,
                longitude: 69.2075,
                altitude_m: 5000.0,
                heading_deg: 45.0,
                speed_mps: 80.0,
            },
            fuel_remaining_pct: 75.5,
            accuracy_pct: 92.3,
            total_engagements: 13,
            successful_hits: 12,
            current_waypoint: 15,
            total_waypoints: 25,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }];

        Ok(Connection {
            items: drones,
            total_count: 1,
            has_next_page: false,
            has_previous_page: false,
        })
    }

    // =========================================================================
    // WAYPOINT QUERIES
    // =========================================================================

    /// Get all waypoints for a drone
    #[graphql(name = "waypoints")]
    async fn get_waypoints(
        &self,
        _ctx: &Context<'_>,
        #[graphql(desc = "Drone ID")]
        drone_id: ID,
    ) -> Result<Vec<Waypoint>> {
        // TODO: Implement with waypoint repository
        // Generate 25 waypoints for demo
        let waypoints: Vec<Waypoint> = (1..=25)
            .map(|i| {
                let status = if i < 15 {
                    WaypointStatus::Complete
                } else if i == 15 {
                    WaypointStatus::Active
                } else {
                    WaypointStatus::Pending
                };

                Waypoint {
                    waypoint_id: ID::from(Uuid::new_v4().to_string()),
                    drone_id: drone_id.clone(),
                    sequence_number: i,
                    name: format!("WP-{:02}", i),
                    waypoint_type: if i % 5 == 0 {
                        WaypointType::Loiter
                    } else {
                        WaypointType::Nav
                    },
                    coordinates: Coordinates {
                        latitude: 34.0 + (i as f64 * 0.15),
                        longitude: 68.5 + (i as f64 * 0.12),
                        altitude_m: 5000.0,
                        heading_deg: (i as f32 * 15.0) % 360.0,
                        speed_mps: 0.0,
                    },
                    status,
                    planned_arrival: Some(Utc::now()),
                    actual_arrival: if i < 15 { Some(Utc::now()) } else { None },
                    planned_departure: Some(Utc::now()),
                    actual_departure: if i < 15 { Some(Utc::now()) } else { None },
                    loiter_duration_min: if i % 5 == 0 { Some(10) } else { None },
                }
            })
            .collect();

        Ok(waypoints)
    }

    // =========================================================================
    // ENGAGEMENT QUERIES
    // =========================================================================

    /// Get engagements for a convoy
    #[graphql(name = "engagements")]
    async fn get_engagements(
        &self,
        _ctx: &Context<'_>,
        #[graphql(desc = "Convoy ID")]
        convoy_id: ID,
        #[graphql(desc = "Optional filter")]
        filter: Option<EngagementFilter>,
        #[graphql(default, desc = "Pagination")]
        pagination: PaginationInput,
    ) -> Result<Connection<Engagement>> {
        // TODO: Implement with engagement repository
        Ok(Connection {
            items: vec![],
            total_count: 0,
            has_next_page: false,
            has_previous_page: false,
        })
    }

    /// Get engagements for a specific drone
    #[graphql(name = "droneEngagements")]
    async fn get_drone_engagements(
        &self,
        _ctx: &Context<'_>,
        #[graphql(desc = "Drone ID")]
        drone_id: ID,
        #[graphql(desc = "Optional filter")]
        filter: Option<EngagementFilter>,
        #[graphql(default, desc = "Pagination")]
        pagination: PaginationInput,
    ) -> Result<Connection<Engagement>> {
        // TODO: Implement with engagement repository
        Ok(Connection {
            items: vec![],
            total_count: 0,
            has_next_page: false,
            has_previous_page: false,
        })
    }

    // =========================================================================
    // TELEMETRY QUERIES
    // =========================================================================

    /// Get latest telemetry for a drone
    #[graphql(name = "latestTelemetry")]
    async fn get_latest_telemetry(
        &self,
        _ctx: &Context<'_>,
        #[graphql(desc = "Drone ID")]
        drone_id: ID,
    ) -> Result<Option<TelemetrySnapshot>> {
        // TODO: Implement with telemetry repository
        Ok(Some(TelemetrySnapshot {
            drone_id,
            recorded_at: Utc::now(),
            position: Coordinates {
                latitude: 34.5553,
                longitude: 69.2075,
                altitude_m: 5000.0,
                heading_deg: 45.0,
                speed_mps: 80.0,
            },
            fuel_remaining_pct: 75.5,
            current_waypoint: 15,
            velocity_mps: 80.0,
            mesh_connectivity: 0.95,
            distance_to_next_km: 12.5,
        }))
    }

    /// Get telemetry history for a drone
    #[graphql(name = "telemetryHistory")]
    async fn get_telemetry_history(
        &self,
        _ctx: &Context<'_>,
        #[graphql(desc = "Drone ID")]
        drone_id: ID,
        #[graphql(desc = "Time range")]
        time_range: TimeRangeInput,
        #[graphql(default, desc = "Pagination")]
        pagination: PaginationInput,
    ) -> Result<Connection<TelemetrySnapshot>> {
        // TODO: Implement with telemetry repository
        Ok(Connection {
            items: vec![],
            total_count: 0,
            has_next_page: false,
            has_previous_page: false,
        })
    }

    // =========================================================================
    // HEALTH CHECK
    // =========================================================================

    /// API health check
    #[graphql(name = "health")]
    async fn health(&self) -> Result<String> {
        Ok("OK".to_string())
    }

    /// API version
    #[graphql(name = "version")]
    async fn version(&self) -> Result<String> {
        Ok(env!("CARGO_PKG_VERSION").to_string())
    }
}
