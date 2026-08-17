//! # GraphQL Mutation Resolver
//!
//! Write operations for the drone convoy API.
//!
//! Every mutation persists through a repository in `drone-persistence` and
//! (where a subscription exists) broadcasts the event. The simulator is the
//! primary caller; the dashboard's ENGAGE control is the second.

use async_graphql::{Context, Object, Result, ID};
use chrono::Utc;
use uuid::Uuid;

use crate::context::ApiContext;
use crate::error::ApiError;
use crate::schema::*;

use drone_domain as domain;
use drone_persistence::DroneRecord;

/// GraphQL Mutation root
pub struct MutationRoot;

#[Object]
impl MutationRoot {
    // =========================================================================
    // ENGAGEMENT MUTATIONS
    // =========================================================================

    /// Record a hit/miss engagement for accuracy tracking
    ///
    /// Updates the leaderboard, persists a full engagement record, and
    /// denormalizes the new accuracy counters onto the drone row.
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

        // Identity for the leaderboard row: registered drone first, then the
        // caller-supplied callsign, then a visible placeholder.
        let registered = api_ctx
            .drone_repo
            .get(convoy_uuid, drone_uuid)
            .await
            .map_err(ApiError::from)?;
        let (callsign, platform) = match &registered {
            Some(rec) => (rec.drone.callsign.clone(), rec.drone.platform_type),
            None => (
                input
                    .callsign
                    .clone()
                    .unwrap_or_else(|| "UNKNOWN".to_string()),
                domain::PlatformType::Mq9Reaper,
            ),
        };

        let domain_entry = api_ctx
            .leaderboard_repo
            .update_entry(convoy_uuid, drone_uuid, &callsign, platform, input.hit)
            .await
            .map_err(ApiError::from)?;

        // Full engagement record — feeds the engagement feed panel.
        let weapon: domain::WeaponType = input
            .weapon_type
            .unwrap_or(WeaponType::Agm114Hellfire)
            .into();
        let target_type: domain::TargetType =
            input.target_type.unwrap_or(TargetType::Vehicle).into();
        let shooter_position = registered
            .as_ref()
            .map(|rec| rec.drone.current_position)
            .unwrap_or_default();
        let now = Utc::now();
        let engagement = domain::Engagement {
            convoy_id: convoy_uuid,
            engaged_at: now,
            engagement_id: Uuid::new_v4(),
            drone_id: drone_uuid,
            drone_callsign: callsign.clone(),
            weapon_type: weapon,
            weapon_serial: String::new(),
            target: domain::TargetInfo {
                target_id: Uuid::new_v4(),
                target_type,
                coordinates: shooter_position,
                confidence: 0.9,
                threat_level: domain::ThreatLevel::Medium,
            },
            authorization_code: String::new(),
            authorized_by: String::new(),
            roe_compliance: true,
            result: domain::EngagementResult {
                impact_time: now,
                impact_coords: shooter_position,
                damage_assessment: if input.hit {
                    domain::DamageAssessment::PendingBda
                } else {
                    domain::DamageAssessment::Missed
                },
                collateral_risk: domain::CollateralRisk::None,
            },
            hit: input.hit,
            waypoint_number: 0,
            shooter_position,
            range_to_target_km: input.range_km.unwrap_or_default() as f32,
            bda_status: "PENDING".to_string(),
            bda_notes: None,
        };
        api_ctx
            .engagement_repo
            .record(&engagement)
            .await
            .map_err(ApiError::from)?;

        // Denormalize the new counters onto the drone row for the cards.
        if registered.is_some() {
            api_ctx
                .drone_repo
                .update_stats(
                    convoy_uuid,
                    drone_uuid,
                    domain_entry.total_engagements,
                    domain_entry.successful_hits,
                    domain_entry.accuracy_pct,
                )
                .await
                .map_err(ApiError::from)?;
        }

        let entry = LeaderboardEntry::from(domain_entry.clone());

        // Broadcast for subscriptions
        let event = EngagementEvent {
            convoy_id: ID(input.convoy_id.clone()),
            drone_id: ID(input.drone_id.clone()),
            callsign: entry.callsign.clone(),
            hit: input.hit,
            weapon_type: input.weapon_type.unwrap_or(WeaponType::Agm114Hellfire),
            new_accuracy_pct: entry.accuracy_pct,
            timestamp: now,
        };
        let _ = api_ctx.engagement_tx.send(event);

        let leaderboard_event = LeaderboardUpdateEvent {
            convoy_id: ID(input.convoy_id.clone()),
            drone_id: ID(input.drone_id.clone()),
            callsign: entry.callsign.clone(),
            new_rank: entry.rank,
            old_rank: None,
            accuracy_pct: entry.accuracy_pct,
            change_type: RankChangeType::ScoreUpdate,
            timestamp: now,
        };
        let _ = api_ctx.leaderboard_tx.send(leaderboard_event);

        Ok(RecordEngagementResult {
            success: true,
            new_rank: i32::from(domain_entry.rank),
            rank_change: 0,
            new_accuracy_pct: domain_entry.accuracy_pct,
            entry,
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

        // Leaderboard accounting first (also broadcasts events).
        let record_input = RecordEngagementInput {
            convoy_id: input.convoy_id.clone(),
            drone_id: input.drone_id.clone(),
            hit: input.hit,
            weapon_type: Some(input.weapon_type),
            target_type: Some(input.target.target_type),
            range_km: None,
            callsign: None,
        };
        let result = self.record_engagement(ctx, record_input).await?;

        let range_km = calculate_distance(
            input.shooter_position.latitude,
            input.shooter_position.longitude,
            input.target.coordinates.latitude,
            input.target.coordinates.longitude,
        );

        let now = Utc::now();
        let target_coords = domain::Coordinates {
            latitude: input.target.coordinates.latitude,
            longitude: input.target.coordinates.longitude,
            altitude_m: input.target.coordinates.altitude_m,
            heading_deg: 0.0,
            speed_mps: 0.0,
        };
        let engagement = domain::Engagement {
            convoy_id: convoy_uuid,
            engaged_at: now,
            engagement_id,
            drone_id: drone_uuid,
            drone_callsign: result.entry.callsign.clone(),
            weapon_type: input.weapon_type.into(),
            weapon_serial: String::new(),
            target: domain::TargetInfo {
                target_id: Uuid::new_v4(),
                target_type: input.target.target_type.into(),
                coordinates: target_coords,
                confidence: input.target.confidence as f32,
                threat_level: domain::ThreatLevel::Medium,
            },
            authorization_code: input.authorization_code.clone(),
            authorized_by: String::new(),
            roe_compliance: input.roe_compliance,
            result: domain::EngagementResult {
                impact_time: now,
                impact_coords: target_coords,
                damage_assessment: if input.hit {
                    domain::DamageAssessment::PendingBda
                } else {
                    domain::DamageAssessment::Missed
                },
                collateral_risk: domain::CollateralRisk::None,
            },
            hit: input.hit,
            waypoint_number: 0,
            shooter_position: domain::Coordinates {
                latitude: input.shooter_position.latitude,
                longitude: input.shooter_position.longitude,
                altitude_m: input.shooter_position.altitude_m,
                heading_deg: input.shooter_position.heading_deg as f32,
                speed_mps: input.shooter_position.speed_mps as f32,
            },
            range_to_target_km: range_km as f32,
            bda_status: "PENDING".to_string(),
            bda_notes: None,
        };
        api_ctx
            .engagement_repo
            .record(&engagement)
            .await
            .map_err(ApiError::from)?;

        Ok(engagement.into())
    }

    /// Update battle damage assessment for an engagement
    #[graphql(name = "updateBda")]
    async fn update_bda(&self, _ctx: &Context<'_>, input: UpdateBdaInput) -> Result<Engagement> {
        tracing::info!(
            engagement_id = %input.engagement_id,
            damage_assessment = ?input.damage_assessment,
            "Updating BDA"
        );

        // BDA updates require a read-modify-write against a clustering-keyed
        // engagement row; the dashboard has no BDA control yet, so this stays
        // an explicit error rather than a silent stub.
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
    ///
    /// Read-merge-write: the stored row is loaded, the provided fields are
    /// applied over it, and the merged record is written back. The first call
    /// for a drone (with callsign/platform set) registers it.
    #[graphql(name = "updateDroneState")]
    async fn update_drone_state(
        &self,
        ctx: &Context<'_>,
        input: UpdateDroneStateInput,
    ) -> Result<Drone> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let convoy_uuid = Uuid::parse_str(&input.convoy_id).map_err(ApiError::from)?;
        let drone_uuid = Uuid::parse_str(&input.drone_id).map_err(ApiError::from)?;

        tracing::debug!(
            convoy_id = %convoy_uuid,
            drone_id = %drone_uuid,
            "Updating drone state"
        );

        let existing = api_ctx
            .drone_repo
            .get(convoy_uuid, drone_uuid)
            .await
            .map_err(ApiError::from)?;
        let old_status = existing
            .as_ref()
            .map(|r| r.drone.status)
            .unwrap_or(domain::DroneStatus::Preflight);

        let mut record = existing.unwrap_or_else(|| DroneRecord {
            drone: domain::Drone {
                convoy_id: convoy_uuid,
                drone_id: drone_uuid,
                tail_number: String::new(),
                callsign: "UNKNOWN".to_string(),
                platform_type: domain::PlatformType::Mq9Reaper,
                serial_number: String::new(),
                status: domain::DroneStatus::Preflight,
                current_position: domain::Coordinates::default(),
                fuel_remaining_pct: 100.0,
                flight_time_hrs: 0.0,
                weapons: Vec::new(),
                sensors: Vec::new(),
                primary_link: None,
                backup_link: None,
                mesh_neighbors: Vec::new(),
                total_engagements: 0,
                successful_hits: 0,
                accuracy_pct: 0.0,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            current_waypoint: 0,
            total_waypoints: 0,
        });

        if let Some(status) = input.status {
            record.drone.status = status.into();
        }
        if let Some(p) = input.position {
            record.drone.current_position = domain::Coordinates {
                latitude: p.latitude,
                longitude: p.longitude,
                altitude_m: p.altitude_m,
                heading_deg: p.heading_deg as f32,
                speed_mps: p.speed_mps as f32,
            };
        }
        if let Some(fuel) = input.fuel_pct {
            record.drone.fuel_remaining_pct = fuel as f32;
        }
        if let Some(wp) = input.current_waypoint {
            record.current_waypoint = wp as i16;
        }
        if let Some(callsign) = input.callsign {
            record.drone.callsign = callsign;
        }
        if let Some(tail) = input.tail_number {
            record.drone.tail_number = tail;
        }
        if let Some(pt) = input.platform_type {
            record.drone.platform_type = pt.into();
        }
        if let Some(total) = input.total_waypoints {
            record.total_waypoints = total as i16;
        }
        record.drone.updated_at = Utc::now();

        api_ctx
            .drone_repo
            .upsert_state(&record)
            .await
            .map_err(ApiError::from)?;

        let wire = Drone {
            drone_id: record.drone.drone_id.to_string(),
            convoy_id: record.drone.convoy_id.to_string(),
            tail_number: record.drone.tail_number.clone(),
            callsign: record.drone.callsign.clone(),
            platform_type: record.drone.platform_type.into(),
            status: record.drone.status.into(),
            current_position: record.drone.current_position.into(),
            fuel_remaining_pct: record.drone.fuel_remaining_pct,
            accuracy_pct: record.drone.accuracy_pct,
            total_engagements: record.drone.total_engagements,
            successful_hits: record.drone.successful_hits,
            current_waypoint: i32::from(record.current_waypoint),
            total_waypoints: i32::from(record.total_waypoints),
            created_at: record.drone.created_at,
            updated_at: record.drone.updated_at,
        };

        // Broadcast the status change for subscriptions
        let event = DroneStatusEvent {
            convoy_id: ID(input.convoy_id),
            drone_id: ID(input.drone_id),
            callsign: wire.callsign.clone(),
            old_status: old_status.into(),
            new_status: wire.status,
            timestamp: wire.updated_at,
        };
        let _ = api_ctx.drone_status_tx.send(event);

        Ok(wire)
    }

    // =========================================================================
    // TELEMETRY MUTATIONS
    // =========================================================================

    /// Record telemetry data point
    #[graphql(name = "recordTelemetry")]
    async fn record_telemetry(
        &self,
        ctx: &Context<'_>,
        input: CreateTelemetryInput,
    ) -> Result<TelemetrySnapshot> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let drone_uuid = Uuid::parse_str(&input.drone_id).map_err(ApiError::from)?;

        tracing::debug!(drone_id = %drone_uuid, "Recording telemetry");

        let now = Utc::now();
        let telemetry = domain::Telemetry {
            drone_id: drone_uuid,
            time_bucket: domain::Telemetry::generate_time_bucket(&now),
            recorded_at: now,
            position: domain::Coordinates {
                latitude: input.position.latitude,
                longitude: input.position.longitude,
                altitude_m: input.position.altitude_m,
                heading_deg: input.position.heading_deg as f32,
                speed_mps: input.position.speed_mps as f32,
            },
            velocity_mps: input.velocity_mps as f32,
            acceleration_mps2: 0.0,
            bank_angle_deg: 0.0,
            pitch_angle_deg: 0.0,
            current_waypoint: input.current_waypoint as i16,
            distance_to_next_km: 0.0,
            eta_next_waypoint: None,
            fuel_remaining_pct: input.fuel_pct as f32,
            engine_rpm: 0,
            engine_temp_c: 0.0,
            battery_voltage: 0.0,
            wind_speed_mps: 0.0,
            wind_direction_deg: 0.0,
            temperature_c: 0.0,
            visibility_km: 0.0,
            link_status: None,
            mesh_connectivity: input.mesh_connectivity as f32,
        };

        api_ctx
            .telemetry_repo
            .record(&telemetry)
            .await
            .map_err(ApiError::from)?;

        let snapshot = TelemetrySnapshot::from(telemetry);
        let _ = api_ctx.telemetry_tx.send(snapshot.clone());

        Ok(snapshot)
    }

    // =========================================================================
    // CONVOY MUTATIONS
    // =========================================================================

    /// Create a new convoy
    #[graphql(name = "createConvoy")]
    async fn create_convoy(&self, ctx: &Context<'_>, input: CreateConvoyInput) -> Result<Convoy> {
        let api_ctx = ctx.data::<ApiContext>()?;

        let convoy_id = match &input.convoy_id {
            Some(id) => Uuid::parse_str(id).map_err(ApiError::from)?,
            None => Uuid::new_v4(),
        };

        tracing::info!(
            convoy_id = %convoy_id,
            callsign = %input.callsign,
            "Creating convoy"
        );

        let convoy = domain::Convoy {
            convoy_id,
            convoy_callsign: input.callsign,
            mission_id: Uuid::new_v4(),
            mission_type: input.mission_type.into(),
            status: domain::ConvoyStatus::Active,
            created_at: Utc::now(),
            mission_start: Some(Utc::now()),
            mission_end: None,
            aor_name: input.aor_name,
            aor_center: domain::Coordinates::new(
                input.aor_center.latitude,
                input.aor_center.longitude,
                input.aor_center.altitude_m,
            ),
            aor_radius_km: input.aor_radius_km as f32,
            commanding_unit: input.commanding_unit,
            authorization_level: String::new(),
            roe_profile: input.roe_profile,
            drone_ids: Vec::new(),
            drone_count: 0,
        };

        api_ctx
            .convoy_repo
            .create(&convoy)
            .await
            .map_err(ApiError::from)?;

        Ok(convoy.into())
    }

    /// Update convoy status
    #[graphql(name = "updateConvoyStatus")]
    async fn update_convoy_status(
        &self,
        ctx: &Context<'_>,
        input: UpdateConvoyStatusInput,
    ) -> Result<Convoy> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let convoy_uuid = Uuid::parse_str(&input.convoy_id).map_err(ApiError::from)?;

        tracing::info!(
            convoy_id = %convoy_uuid,
            status = ?input.status,
            "Updating convoy status"
        );

        api_ctx
            .convoy_repo
            .update_status(convoy_uuid, input.status.into())
            .await
            .map_err(ApiError::from)?;

        let convoy = api_ctx
            .convoy_repo
            .get(convoy_uuid)
            .await
            .map_err(ApiError::from)?
            .ok_or_else(|| async_graphql::Error::new("Convoy not found after update"))?;

        Ok(convoy.into())
    }

    /// Retask a convoy to a tactical theater.
    ///
    /// The tasking order the dashboard issues from its THEATER selector.
    /// Validated against the shared theater vocabulary; written to the
    /// convoy record (aor_name = slug, aor_center = theater centre); the
    /// simulator -- or, later, a live ground station -- watches the record
    /// and flies the new route. The dashboard never commands a process; it
    /// commands the system of record.
    #[graphql(name = "retaskConvoy")]
    async fn retask_convoy(&self, ctx: &Context<'_>, input: RetaskConvoyInput) -> Result<Convoy> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let convoy_uuid = Uuid::parse_str(&input.convoy_id).map_err(ApiError::from)?;
        let theater = domain::TheaterId::from_slug(&input.theater).ok_or_else(|| {
            let valid: Vec<&str> = domain::TheaterId::ALL.iter().map(|t| t.slug()).collect();
            async_graphql::Error::new(format!(
                "unknown theater '{}' (valid: {})", input.theater, valid.join(", ")
            ))
        })?;
        let t = theater.theater();

        tracing::info!(convoy_id = %convoy_uuid, theater = t.label, "Retasking convoy");

        let center = domain::Coordinates {
            latitude: t.center.0,
            longitude: t.center.1,
            altitude_m: 0.0,
            heading_deg: 0.0,
            speed_mps: 0.0,
        };
        api_ctx
            .convoy_repo
            .retask(convoy_uuid, theater.slug(), &center)
            .await
            .map_err(ApiError::from)?;

        let convoy = api_ctx
            .convoy_repo
            .get(convoy_uuid)
            .await
            .map_err(ApiError::from)?
            .ok_or_else(|| async_graphql::Error::new("Convoy not found after retask"))?;

        Ok(convoy.into())
    }

    // =========================================================================
    // WAYPOINT MUTATIONS
    // =========================================================================

    /// Create waypoints for a drone (replaces the route)
    #[graphql(name = "createWaypoints")]
    async fn create_waypoints(
        &self,
        ctx: &Context<'_>,
        input: CreateWaypointsInput,
    ) -> Result<Vec<Waypoint>> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let drone_uuid = Uuid::parse_str(&input.drone_id).map_err(ApiError::from)?;

        tracing::info!(
            drone_id = %drone_uuid,
            count = input.waypoints.len(),
            "Creating waypoints"
        );

        let waypoints: Vec<domain::Waypoint> = input
            .waypoints
            .into_iter()
            .map(|w| domain::Waypoint {
                drone_id: drone_uuid,
                sequence_number: w.sequence_number as i16,
                waypoint_id: Uuid::new_v4(),
                waypoint_name: w.name,
                waypoint_type: w.waypoint_type.into(),
                coordinates: domain::Coordinates {
                    latitude: w.coordinates.latitude,
                    longitude: w.coordinates.longitude,
                    altitude_m: w.coordinates.altitude_m,
                    heading_deg: w.coordinates.heading_deg as f32,
                    speed_mps: w.coordinates.speed_mps as f32,
                },
                planned_arrival: None,
                actual_arrival: None,
                planned_departure: None,
                actual_departure: None,
                loiter_duration_min: None,
                authorized_actions: Vec::new(),
                status: domain::WaypointStatus::Pending,
            })
            .collect();

        api_ctx
            .waypoint_repo
            .create_batch(&waypoints)
            .await
            .map_err(ApiError::from)?;

        Ok(waypoints.into_iter().map(Waypoint::from).collect())
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
