//! # GraphQL Query Resolver
//!
//! Read operations for the drone convoy API.
//!
//! Every resolver here reads from a repository in `drone-persistence`.
//! There are NO hardcoded structs in this file — if a panel is empty, the
//! table is empty, and the GraphQL playground will tell you the truth.

use async_graphql::{Context, Object, Result, ID};
use chrono::Utc;
use uuid::Uuid;

use crate::context::ApiContext;
use crate::error::ApiError;
use crate::schema::*;

use drone_persistence::DroneRecord;

/// Map a persistence [`DroneRecord`] onto the wire type.
///
/// Lives here rather than in `objects.rs` because `DroneRecord` is a
/// persistence type; `objects.rs` converts only from `drone-domain`.
fn drone_from_record(rec: DroneRecord) -> Drone {
    let DroneRecord {
        drone,
        current_waypoint,
        total_waypoints,
    } = rec;
    Drone {
        drone_id: drone.drone_id.to_string(),
        convoy_id: drone.convoy_id.to_string(),
        tail_number: drone.tail_number,
        callsign: drone.callsign,
        platform_type: drone.platform_type.into(),
        status: drone.status.into(),
        current_position: drone.current_position.into(),
        fuel_remaining_pct: drone.fuel_remaining_pct,
        accuracy_pct: drone.accuracy_pct,
        total_engagements: drone.total_engagements,
        successful_hits: drone.successful_hits,
        current_waypoint: i32::from(current_waypoint),
        total_waypoints: i32::from(total_waypoints),
        created_at: drone.created_at,
        updated_at: drone.updated_at,
    }
}

/// Slice a full result set according to pagination and wrap it.
fn paginate<T: async_graphql::OutputType>(
    items: Vec<T>,
    pagination: &PaginationInput,
) -> Connection<T> {
    let total = items.len();
    let offset = pagination.offset.max(0) as usize;
    let limit = pagination.limit.max(0) as usize;
    let page: Vec<T> = items.into_iter().skip(offset).take(limit).collect();
    Connection {
        total_count: total as i32,
        has_next_page: offset + page.len() < total,
        has_previous_page: offset > 0,
        items: page,
    }
}

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

        // Real callsign when the convoy row exists; a derived placeholder
        // when it does not (leaderboard rows can precede convoy creation).
        let convoy_callsign = api_ctx
            .convoy_repo
            .get(convoy_uuid)
            .await
            .map_err(ApiError::from)?
            .map(|c| c.convoy_callsign)
            .unwrap_or_else(|| format!("CONVOY-{}", &convoy_id.as_str()[..8]));

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
    async fn get_active_convoys(&self, ctx: &Context<'_>) -> Result<Vec<Convoy>> {
        let api_ctx = ctx.data::<ApiContext>()?;

        let convoys = api_ctx
            .convoy_repo
            .list()
            .await
            .map_err(ApiError::from)?
            .into_iter()
            .filter(|c| c.status == drone_domain::ConvoyStatus::Active)
            .map(Convoy::from)
            .collect();

        Ok(convoys)
    }

    /// Get convoy details by ID
    #[graphql(name = "convoy")]
    async fn get_convoy(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Convoy ID")]
        convoy_id: ID,
    ) -> Result<Option<Convoy>> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let convoy_uuid = Uuid::parse_str(&convoy_id).map_err(ApiError::from)?;

        let convoy = api_ctx
            .convoy_repo
            .get(convoy_uuid)
            .await
            .map_err(ApiError::from)?
            .map(Convoy::from);

        Ok(convoy)
    }

    /// Get convoy statistics
    ///
    /// Engagement totals come from the leaderboard; fuel and airborne counts
    /// come from live drone state.
    #[graphql(name = "convoyStats")]
    async fn get_convoy_stats(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Convoy ID")]
        convoy_id: ID,
    ) -> Result<ConvoyStats> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let convoy_uuid = Uuid::parse_str(&convoy_id).map_err(ApiError::from)?;

        let entries = api_ctx
            .leaderboard_repo
            .get_leaderboard(convoy_uuid, 100)
            .await
            .map_err(ApiError::from)?;

        let total_engagements: i32 = entries.iter().map(|e| e.total_engagements).sum();
        let total_hits: i32 = entries.iter().map(|e| e.successful_hits).sum();
        let avg_accuracy = if entries.is_empty() {
            0.0
        } else {
            entries.iter().map(|e| e.accuracy_pct).sum::<f32>() / entries.len() as f32
        };

        let drones = api_ctx
            .drone_repo
            .list_by_convoy(convoy_uuid)
            .await
            .map_err(ApiError::from)?;

        let airborne_count = drones
            .iter()
            .filter(|d| {
                matches!(
                    d.drone.status,
                    drone_domain::DroneStatus::Airborne
                        | drone_domain::DroneStatus::Loiter
                        | drone_domain::DroneStatus::Ingress
                        | drone_domain::DroneStatus::Egress
                        | drone_domain::DroneStatus::Rtb
                )
            })
            .count() as i32;
        let avg_fuel = if drones.is_empty() {
            0.0
        } else {
            drones.iter().map(|d| d.drone.fuel_remaining_pct).sum::<f32>()
                / drones.len() as f32
        };

        Ok(ConvoyStats {
            convoy_id,
            drone_count: drones.len() as i32,
            airborne_count,
            total_engagements,
            total_hits,
            average_accuracy_pct: avg_accuracy,
            average_fuel_pct: avg_fuel,
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
        ctx: &Context<'_>,
        #[graphql(desc = "Convoy ID")]
        convoy_id: ID,
        #[graphql(desc = "Drone ID")]
        drone_id: ID,
    ) -> Result<Option<Drone>> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let convoy_uuid = Uuid::parse_str(&convoy_id).map_err(ApiError::from)?;
        let drone_uuid = Uuid::parse_str(&drone_id).map_err(ApiError::from)?;

        let drone = api_ctx
            .drone_repo
            .get(convoy_uuid, drone_uuid)
            .await
            .map_err(ApiError::from)?
            .map(drone_from_record);

        Ok(drone)
    }

    /// Get all drones in a convoy
    #[graphql(name = "drones")]
    async fn get_drones(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Convoy ID")]
        convoy_id: ID,
        #[graphql(desc = "Optional filter")]
        filter: Option<DroneFilter>,
        #[graphql(default, desc = "Pagination")]
        pagination: PaginationInput,
    ) -> Result<Connection<Drone>> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let convoy_uuid = Uuid::parse_str(&convoy_id).map_err(ApiError::from)?;

        let drones: Vec<Drone> = api_ctx
            .drone_repo
            .list_by_convoy(convoy_uuid)
            .await
            .map_err(ApiError::from)?
            .into_iter()
            .map(drone_from_record)
            .filter(|d| {
                if let Some(ref f) = filter {
                    if let Some(status) = f.status {
                        if d.status != status {
                            return false;
                        }
                    }
                    if let Some(pt) = f.platform_type {
                        if d.platform_type != pt {
                            return false;
                        }
                    }
                    if let Some(min_fuel) = f.min_fuel_pct {
                        if f64::from(d.fuel_remaining_pct) < min_fuel {
                            return false;
                        }
                    }
                }
                true
            })
            .collect();

        Ok(paginate(drones, &pagination))
    }

    // =========================================================================
    // WAYPOINT QUERIES
    // =========================================================================

    /// Get all waypoints for a drone
    #[graphql(name = "waypoints")]
    async fn get_waypoints(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Drone ID")]
        drone_id: ID,
    ) -> Result<Vec<Waypoint>> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let drone_uuid = Uuid::parse_str(&drone_id).map_err(ApiError::from)?;

        let waypoints = api_ctx
            .waypoint_repo
            .get_waypoints(drone_uuid)
            .await
            .map_err(ApiError::from)?
            .into_iter()
            .map(Waypoint::from)
            .collect();

        Ok(waypoints)
    }

    // =========================================================================
    // ENGAGEMENT QUERIES
    // =========================================================================

    /// Get engagements for a convoy (most recent first)
    #[graphql(name = "engagements")]
    async fn get_engagements(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Convoy ID")]
        convoy_id: ID,
        #[graphql(desc = "Optional filter")]
        filter: Option<EngagementFilter>,
        #[graphql(default, desc = "Pagination")]
        pagination: PaginationInput,
    ) -> Result<Connection<Engagement>> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let convoy_uuid = Uuid::parse_str(&convoy_id).map_err(ApiError::from)?;

        // Read enough rows to cover the requested page plus filtering.
        let fetch = (pagination.offset + pagination.limit).max(1) * 2;
        let engagements: Vec<Engagement> = api_ctx
            .engagement_repo
            .get_recent(convoy_uuid, fetch.min(500))
            .await
            .map_err(ApiError::from)?
            .into_iter()
            .map(Engagement::from)
            .filter(|e| filter_engagement(e, filter.as_ref()))
            .collect();

        Ok(paginate(engagements, &pagination))
    }

    /// Get engagements for a specific drone (most recent first)
    #[graphql(name = "droneEngagements")]
    async fn get_drone_engagements(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Drone ID")]
        drone_id: ID,
        #[graphql(desc = "Optional filter")]
        filter: Option<EngagementFilter>,
        #[graphql(default, desc = "Pagination")]
        pagination: PaginationInput,
    ) -> Result<Connection<Engagement>> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let drone_uuid = Uuid::parse_str(&drone_id).map_err(ApiError::from)?;

        let fetch = (pagination.offset + pagination.limit).max(1) * 2;
        let engagements: Vec<Engagement> = api_ctx
            .engagement_repo
            .get_by_drone(drone_uuid, fetch.min(500))
            .await
            .map_err(ApiError::from)?
            .into_iter()
            .map(Engagement::from)
            .filter(|e| filter_engagement(e, filter.as_ref()))
            .collect();

        Ok(paginate(engagements, &pagination))
    }

    // =========================================================================
    // TELEMETRY QUERIES
    // =========================================================================

    /// Get latest telemetry for a drone
    #[graphql(name = "latestTelemetry")]
    async fn get_latest_telemetry(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Drone ID")]
        drone_id: ID,
    ) -> Result<Option<TelemetrySnapshot>> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let drone_uuid = Uuid::parse_str(&drone_id).map_err(ApiError::from)?;

        let snapshot = api_ctx
            .telemetry_repo
            .get_latest(drone_uuid)
            .await
            .map_err(ApiError::from)?
            .map(TelemetrySnapshot::from);

        Ok(snapshot)
    }

    /// Get telemetry history for a drone (most recent first)
    #[graphql(name = "telemetryHistory")]
    async fn get_telemetry_history(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Drone ID")]
        drone_id: ID,
        #[graphql(desc = "Time range")]
        time_range: TimeRangeInput,
        #[graphql(default, desc = "Pagination")]
        pagination: PaginationInput,
    ) -> Result<Connection<TelemetrySnapshot>> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let drone_uuid = Uuid::parse_str(&drone_id).map_err(ApiError::from)?;

        let fetch = (pagination.offset + pagination.limit).max(1) * 2;
        let snapshots: Vec<TelemetrySnapshot> = api_ctx
            .telemetry_repo
            .get_history(drone_uuid, fetch.min(1000))
            .await
            .map_err(ApiError::from)?
            .into_iter()
            .map(TelemetrySnapshot::from)
            .filter(|t| t.recorded_at >= time_range.start && t.recorded_at <= time_range.end)
            .collect();

        Ok(paginate(snapshots, &pagination))
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

/// Apply an [`EngagementFilter`] to a wire-type engagement.
fn filter_engagement(e: &Engagement, filter: Option<&EngagementFilter>) -> bool {
    let Some(f) = filter else { return true };
    if let Some(hit) = f.hit {
        if e.hit != hit {
            return false;
        }
    }
    if let Some(wt) = f.weapon_type {
        if e.weapon_type != wt {
            return false;
        }
    }
    if let Some(da) = f.damage_assessment {
        if e.damage_assessment != da {
            return false;
        }
    }
    if let Some(ref tr) = f.time_range {
        if e.engaged_at < tr.start || e.engaged_at > tr.end {
            return false;
        }
    }
    true
}
