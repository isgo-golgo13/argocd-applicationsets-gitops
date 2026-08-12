//! # GraphQL Mutation Resolver
//!
//! Write operations for the drone convoy API.

use async_graphql::{Context, Object, Result, ID};
use chrono::Utc;
use uuid::Uuid;

use crate::context::ApiContext;
use crate::error::ApiError;
use crate::schema::*;

/// GraphQL Mutation root
pub struct MutationRoot;

#[Object]
impl MutationRoot {
    // =========================================================================
    // ENGAGEMENT MUTATIONS
    // =========================================================================

    /// Record a hit/miss engagement for accuracy tracking
    ///
    /// Updates the drone's accuracy counters and recalculates leaderboard position.
    /// This is the primary mutation for leaderboard updates.
    #[graphql(name = "recordEngagement")]
    async fn record_engagement(
        &self,
        ctx: &Context<'_>,
        input: RecordEngagementInput,
    ) -> Result<RecordEngagementResult> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let convoy_uuid = Uuid::parse_str(&input.convoy_id).map_err(ApiError::from)?;
        let drone_uuid = Uuid::parse_str(&input.drone_id).map_err(ApiError::from)?;

        tracing::info!(
            convoy_id = %convoy_uuid,
            drone_id = %drone_uuid,
            hit = input.hit,
            "Recording engagement"
        );

        // Use update_entry which handles incrementing counters internally
        let domain_entry = api_ctx
            .leaderboard_repo
            .update_entry(
                convoy_uuid,
                drone_uuid,
                "UNKNOWN", // TODO: Fetch callsign from drone repo
                drone_domain::PlatformType::Mq9Reaper,
                input.hit,
            )
            .await
            .map_err(ApiError::from)?;

        // Build GraphQL leaderboard entry from domain entry
        let entry = LeaderboardEntry::from(domain_entry.clone());

        // Broadcast event for subscriptions
        let event = EngagementEvent {
            convoy_id: ID(input.convoy_id.clone()),
            drone_id: ID(input.drone_id.clone()),
            callsign: entry.callsign.clone(),
            hit: input.hit,
            weapon_type: input.weapon_type.unwrap_or(WeaponType::Agm114Hellfire),
            new_accuracy_pct: entry.accuracy_pct,
            timestamp: Utc::now(),
        };
        let _ = api_ctx.engagement_tx.send(event);

        // Broadcast leaderboard update
        let leaderboard_event = LeaderboardUpdateEvent {
            convoy_id: ID(input.convoy_id.clone()),
            drone_id: ID(input.drone_id.clone()),
            callsign: entry.callsign.clone(),
            new_rank: entry.rank,
            old_rank: None, // Simplified - not tracking old rank
            accuracy_pct: entry.accuracy_pct,
            change_type: RankChangeType::ScoreUpdate,
            timestamp: Utc::now(),
        };
        let _ = api_ctx.leaderboard_tx.send(leaderboard_event);

        Ok(RecordEngagementResult {
            success: true,
            entry,
            new_rank: domain_entry.rank as i32,
            rank_change: 0, // Simplified
            new_accuracy_pct: domain_entry.accuracy_pct,
        })
    }

    /// Create a full engagement record with target details
    #[graphql(name = "createEngagement")]
    async fn create_engagement(
        &self,
        ctx: &Context<'_>,
        input: CreateEngagementInput,
    ) -> Result<Engagement> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let convoy_uuid = Uuid::parse_str(&input.convoy_id).map_err(ApiError::from)?;
        let drone_uuid = Uuid::parse_str(&input.drone_id).map_err(ApiError::from)?;
        let engagement_id = Uuid::new_v4();

        tracing::info!(
            engagement_id = %engagement_id,
            convoy_id = %convoy_uuid,
            drone_id = %drone_uuid,
            weapon = ?input.weapon_type,
            hit = input.hit,
            "Creating engagement record"
        );

        // Record the hit/miss for accuracy tracking
        let record_input = RecordEngagementInput {
            convoy_id: input.convoy_id.clone(),
            drone_id: input.drone_id.clone(),
            hit: input.hit,
            weapon_type: Some(input.weapon_type),
            target_type: Some(input.target.target_type),
            range_km: None,
        };
        let _ = self.record_engagement(ctx, record_input).await?;

        // Calculate range
        let range_km = calculate_distance(
            input.shooter_position.latitude,
            input.shooter_position.longitude,
            input.target.coordinates.latitude,
            input.target.coordinates.longitude,
        );

        // TODO: Persist to engagement repository

        Ok(Engagement {
            engagement_id: ID(engagement_id.to_string()),
            convoy_id: ID(input.convoy_id),
            drone_id: ID(input.drone_id),
            drone_callsign: "UNKNOWN".to_string(),
            engaged_at: Utc::now(),
            weapon_type: input.weapon_type,
            target_type: input.target.target_type,
            target_coordinates: Coordinates {
                latitude: input.target.coordinates.latitude,
                longitude: input.target.coordinates.longitude,
                altitude_m: input.target.coordinates.altitude_m,
                heading_deg: 0.0,
                speed_mps: 0.0,
            },
            shooter_position: Coordinates {
                latitude: input.shooter_position.latitude,
                longitude: input.shooter_position.longitude,
                altitude_m: input.shooter_position.altitude_m,
                heading_deg: input.shooter_position.heading_deg as f32,
                speed_mps: input.shooter_position.speed_mps as f32,
            },
            range_km: range_km as f32,
            hit: input.hit,
            damage_assessment: if input.hit {
                DamageAssessment::PendingBda
            } else {
                DamageAssessment::Missed
            },
            authorization_code: input.authorization_code,
            roe_compliant: input.roe_compliance,
        })
    }

    /// Update battle damage assessment for an engagement
    #[graphql(name = "updateBda")]
    async fn update_bda(&self, _ctx: &Context<'_>, input: UpdateBdaInput) -> Result<Engagement> {
        tracing::info!(
            engagement_id = %input.engagement_id,
            damage_assessment = ?input.damage_assessment,
            "Updating BDA"
        );

        // TODO: Implement with engagement repository

        Err(async_graphql::Error::new("Not implemented"))
    }

    // =========================================================================
    // LEADERBOARD MUTATIONS
    // =========================================================================

    /// Force rebuild of leaderboard cache from source data
    #[graphql(name = "rebuildLeaderboard")]
    async fn rebuild_leaderboard(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Convoy ID")]
        convoy_id: ID,
    ) -> Result<RebuildLeaderboardResult> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let convoy_uuid = Uuid::parse_str(&convoy_id).map_err(ApiError::from)?;

        tracing::info!(convoy_id = %convoy_uuid, "Rebuilding leaderboard");

        let start = std::time::Instant::now();

        // Get current entries (rebuild would recalculate from engagements)
        let entries = api_ctx
            .leaderboard_repo
            .get_leaderboard(convoy_uuid, 100)
            .await
            .map_err(ApiError::from)?;

        let duration_ms = start.elapsed().as_millis() as i64;

        Ok(RebuildLeaderboardResult {
            success: true,
            entries_processed: entries.len() as i32,
            duration_ms,
        })
    }

    // =========================================================================
    // DRONE MUTATIONS
    // =========================================================================

    /// Update drone state
    #[graphql(name = "updateDroneState")]
    async fn update_drone_state(
        &self,
        ctx: &Context<'_>,
        input: UpdateDroneStateInput,
    ) -> Result<Drone> {
        let _api_ctx = ctx.data::<ApiContext>()?;

        tracing::info!(
            convoy_id = %input.convoy_id,
            drone_id = %input.drone_id,
            "Updating drone state"
        );

        // TODO: Implement with drone repository

        Ok(Drone {
            drone_id: input.drone_id.clone(),
            convoy_id: input.convoy_id.clone(),
            tail_number: "AF-001".to_string(),
            callsign: "REAPER-01".to_string(),
            platform_type: PlatformType::Mq9Reaper,
            status: input.status.unwrap_or(DroneStatus::Airborne),
            current_position: input.position.map(|p| Coordinates {
                latitude: p.latitude,
                longitude: p.longitude,
                altitude_m: p.altitude_m,
                heading_deg: p.heading_deg as f32,
                speed_mps: p.speed_mps as f32,
            }).unwrap_or(Coordinates {
                latitude: 34.5553,
                longitude: 69.2075,
                altitude_m: 5000.0,
                heading_deg: 45.0,
                speed_mps: 80.0,
            }),
            fuel_remaining_pct: input.fuel_pct.unwrap_or(75.0) as f32,
            accuracy_pct: 92.3,
            total_engagements: 13,
            successful_hits: 12,
            current_waypoint: input.current_waypoint.unwrap_or(15),
            total_waypoints: 25,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    // =========================================================================
    // TELEMETRY MUTATIONS
    // =========================================================================

    /// Record telemetry data point
    #[graphql(name = "recordTelemetry")]
    async fn record_telemetry(
        &self,
        _ctx: &Context<'_>,
        input: CreateTelemetryInput,
    ) -> Result<TelemetrySnapshot> {
        tracing::debug!(drone_id = %input.drone_id, "Recording telemetry");

        // TODO: Implement with telemetry repository

        Ok(TelemetrySnapshot {
            drone_id: ID(input.drone_id),
            recorded_at: Utc::now(),
            position: Coordinates {
                latitude: input.position.latitude,
                longitude: input.position.longitude,
                altitude_m: input.position.altitude_m,
                heading_deg: input.position.heading_deg as f32,
                speed_mps: input.position.speed_mps as f32,
            },
            fuel_remaining_pct: input.fuel_pct as f32,
            current_waypoint: input.current_waypoint,
            velocity_mps: input.velocity_mps as f32,
            mesh_connectivity: input.mesh_connectivity as f32,
            distance_to_next_km: 0.0,
        })
    }

    // =========================================================================
    // CONVOY MUTATIONS
    // =========================================================================

    /// Create a new convoy
    #[graphql(name = "createConvoy")]
    async fn create_convoy(&self, _ctx: &Context<'_>, input: CreateConvoyInput) -> Result<Convoy> {
        let convoy_id = Uuid::new_v4();

        tracing::info!(
            convoy_id = %convoy_id,
            callsign = %input.callsign,
            "Creating convoy"
        );

        // TODO: Implement with convoy repository

        Ok(Convoy {
            convoy_id: ID(convoy_id.to_string()),
            callsign: input.callsign,
            mission_type: input.mission_type,
            status: ConvoyStatus::Planning,
            aor_name: input.aor_name,
            aor_center: Coordinates {
                latitude: input.aor_center.latitude,
                longitude: input.aor_center.longitude,
                altitude_m: input.aor_center.altitude_m,
                heading_deg: 0.0,
                speed_mps: 0.0,
            },
            aor_radius_km: input.aor_radius_km as f32,
            drone_count: 0,
            commanding_unit: input.commanding_unit,
            mission_start: None,
            mission_end: None,
            created_at: Utc::now(),
        })
    }

    /// Update convoy status
    #[graphql(name = "updateConvoyStatus")]
    async fn update_convoy_status(
        &self,
        _ctx: &Context<'_>,
        input: UpdateConvoyStatusInput,
    ) -> Result<Convoy> {
        tracing::info!(
            convoy_id = %input.convoy_id,
            status = ?input.status,
            "Updating convoy status"
        );

        // TODO: Implement with convoy repository

        Err(async_graphql::Error::new("Not implemented"))
    }

    // =========================================================================
    // WAYPOINT MUTATIONS
    // =========================================================================

    /// Create waypoints for a drone
    #[graphql(name = "createWaypoints")]
    async fn create_waypoints(
        &self,
        _ctx: &Context<'_>,
        input: CreateWaypointsInput,
    ) -> Result<Vec<Waypoint>> {
        tracing::info!(
            drone_id = %input.drone_id,
            count = input.waypoints.len(),
            "Creating waypoints"
        );

        // TODO: Implement with waypoint repository

        let waypoints: Vec<Waypoint> = input
            .waypoints
            .into_iter()
            .map(|w| Waypoint {
                waypoint_id: ID(Uuid::new_v4().to_string()),
                drone_id: ID(input.drone_id.clone()),
                sequence_number: w.sequence_number,
                name: w.name,
                waypoint_type: w.waypoint_type,
                coordinates: Coordinates {
                    latitude: w.coordinates.latitude,
                    longitude: w.coordinates.longitude,
                    altitude_m: w.coordinates.altitude_m,
                    heading_deg: w.coordinates.heading_deg as f32,
                    speed_mps: 0.0,
                },
                status: WaypointStatus::Pending,
                planned_arrival: None,
                actual_arrival: None,
                planned_departure: None,
                actual_departure: None,
                loiter_duration_min: None,
            })
            .collect();

        Ok(waypoints)
    }
}

/// Calculate great-circle distance between two points (Haversine)
fn calculate_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const EARTH_RADIUS_KM: f64 = 6371.0;

    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();
    let delta_lat = (lat2 - lat1).to_radians();
    let delta_lon = (lon2 - lon1).to_radians();

    let a = (delta_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();

    EARTH_RADIUS_KM * c
}
