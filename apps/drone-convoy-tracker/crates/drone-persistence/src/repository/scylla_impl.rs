//! ScyllaDB repository implementation.
//!
//! Provides repository pattern access to ScyllaDB for drone convoy entities.

use chrono::Utc;
use scylla::{Session, SessionBuilder};
use std::sync::Arc;
use uuid::Uuid;

use crate::cache::SharedCacheClient;
use crate::error::Result;
use crate::strategy::{ReadStrategy, WriteStrategy};
use drone_domain::{
    Convoy, ConvoyStatus, Engagement, LeaderboardEntry,
    MissionType, PlatformType, TargetType, Telemetry, Waypoint,
};

// =============================================================================
// SCYLLA CONFIGURATION
// =============================================================================

/// ScyllaDB connection configuration.
#[derive(Debug, Clone)]
pub struct ScyllaConfig {
    pub hosts: Vec<String>,
    pub keyspace: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl Default for ScyllaConfig {
    fn default() -> Self {
        Self {
            hosts: vec!["localhost:9042".to_string()],
            keyspace: "drone_ops".to_string(),
            username: None,
            password: None,
        }
    }
}

// =============================================================================
// SCYLLA CLIENT
// =============================================================================

/// ScyllaDB client wrapper.
pub struct ScyllaClient {
    session: Arc<Session>,
    pub config: ScyllaConfig,
}

impl ScyllaClient {
    /// Create a new ScyllaDB client.
    pub async fn new(config: ScyllaConfig) -> Result<Self> {
        let mut builder = SessionBuilder::new()
            .known_nodes(&config.hosts);

        if let (Some(user), Some(pass)) = (&config.username, &config.password) {
            builder = builder.user(user, pass);
        }

        let session = builder.build().await?;

        // Use keyspace
        session
            .query_unpaged(format!("USE {}", config.keyspace), ())
            .await?;

        Ok(Self {
            session: Arc::new(session),
            config,
        })
    }

    /// Get session reference.
    pub fn session(&self) -> &Session {
        &self.session
    }
}

// =============================================================================
// LEADERBOARD REPOSITORY
// =============================================================================

/// Repository for leaderboard operations.
pub struct ScyllaLeaderboardRepository {
    client: Arc<ScyllaClient>,
    cache: Option<SharedCacheClient>,
    read_strategy: ReadStrategy,
    write_strategy: WriteStrategy,
}

impl ScyllaLeaderboardRepository {
    /// Create a new leaderboard repository with default strategies.
    pub fn new(client: Arc<ScyllaClient>, cache: Option<SharedCacheClient>) -> Self {
        Self {
            client,
            cache,
            read_strategy: ReadStrategy::CacheFirst,
            write_strategy: WriteStrategy::WriteThrough,
        }
    }

    /// Create with custom strategies.
    pub fn with_strategies(
        client: Arc<ScyllaClient>,
        cache: Option<SharedCacheClient>,
        read_strategy: ReadStrategy,
        write_strategy: WriteStrategy,
    ) -> Self {
        Self {
            client,
            cache,
            read_strategy,
            write_strategy,
        }
    }

    /// Set read strategy.
    pub fn set_read_strategy(&mut self, strategy: ReadStrategy) {
        self.read_strategy = strategy;
    }

    /// Set write strategy.
    pub fn set_write_strategy(&mut self, strategy: WriteStrategy) {
        self.write_strategy = strategy;
    }

    /// Get leaderboard for a convoy.
    pub async fn get_leaderboard(
        &self,
        convoy_id: Uuid,
        limit: i32,
    ) -> Result<Vec<LeaderboardEntry>> {
        // Try cache first if available
        if let Some(ref cache) = self.cache {
            if let Ok(cached) = cache.get_leaderboard(convoy_id, limit as usize).await {
                // Cache returns Vec<(Uuid, f64)> - would need to hydrate full entries
                let _ = cached;
            }
        }

        let query = r#"
            SELECT convoy_id, drone_id, callsign, platform_type, 
                   total_engagements, successful_hits, accuracy_pct, 
                   current_streak, best_streak, rank, updated_at
            FROM leaderboard
            WHERE convoy_id = ?
            LIMIT ?
        "#;

        let result = self.client.session
            .query_unpaged(query, (convoy_id, limit))
            .await?;

        let mut entries = Vec::new();
        
        // Get rows result and iterate
        if let Ok(rows_result) = result.into_rows_result() {
            if let Ok(rows) = rows_result.rows::<(
                Uuid, Uuid, String, String, i32, i32, f32, i32, i32, i16
            )>() {
                for row_result in rows {
                    if let Ok((cid, did, callsign, platform, total, hits, acc, streak, best, rank)) = row_result {
                        entries.push(LeaderboardEntry {
                            convoy_id: cid,
                            drone_id: did,
                            callsign,
                            platform_type: parse_platform_type(&platform),
                            total_engagements: total,
                            successful_hits: hits,
                            accuracy_pct: acc,
                            current_streak: streak,
                            best_streak: best,
                            rank,
                            updated_at: Utc::now(),
                        });
                    }
                }
            }
        }

        Ok(entries)
    }

    /// Update leaderboard entry after engagement.
    pub async fn update_entry(
        &self,
        convoy_id: Uuid,
        drone_id: Uuid,
        callsign: &str,
        platform: PlatformType,
        hit: bool,
    ) -> Result<LeaderboardEntry> {
        // Get current stats or defaults
        let current = self.get_drone_entry(convoy_id, drone_id).await?;
        
        let (total, hits, streak, best) = match current {
            Some(e) => {
                let new_streak = if hit { e.current_streak + 1 } else { 0 };
                let new_best = new_streak.max(e.best_streak);
                (
                    e.total_engagements + 1,
                    if hit { e.successful_hits + 1 } else { e.successful_hits },
                    new_streak,
                    new_best,
                )
            }
            None => (1, if hit { 1 } else { 0 }, if hit { 1 } else { 0 }, if hit { 1 } else { 0 }),
        };

        let accuracy = if total > 0 {
            (hits as f32 / total as f32) * 100.0
        } else {
            0.0
        };

        let update = r#"
            UPDATE leaderboard
            SET callsign = ?,
                platform_type = ?,
                total_engagements = ?, 
                successful_hits = ?, 
                accuracy_pct = ?,
                current_streak = ?,
                best_streak = ?,
                updated_at = toTimestamp(now())
            WHERE convoy_id = ? AND drone_id = ?
        "#;

        self.client.session
            .query_unpaged(update, (
                callsign,
                platform.as_str(),
                total,
                hits,
                accuracy,
                streak,
                best,
                convoy_id,
                drone_id,
            ))
            .await?;

        // Invalidate cache
        if let Some(ref cache) = self.cache {
            let _ = cache.invalidate_drone(drone_id).await;
        }

        Ok(LeaderboardEntry {
            convoy_id,
            drone_id,
            callsign: callsign.to_string(),
            platform_type: platform,
            total_engagements: total,
            successful_hits: hits,
            accuracy_pct: accuracy,
            current_streak: streak,
            best_streak: best,
            rank: 0, // Will be recalculated
            updated_at: Utc::now(),
        })
    }

    /// Get single drone entry.
    async fn get_drone_entry(
        &self,
        convoy_id: Uuid,
        drone_id: Uuid,
    ) -> Result<Option<LeaderboardEntry>> {
        let query = r#"
            SELECT convoy_id, drone_id, callsign, platform_type,
                   total_engagements, successful_hits, accuracy_pct,
                   current_streak, best_streak, rank
            FROM leaderboard
            WHERE convoy_id = ? AND drone_id = ?
        "#;

        let result = self.client.session
            .query_unpaged(query, (convoy_id, drone_id))
            .await?;

        if let Ok(rows_result) = result.into_rows_result() {
            if let Ok(rows) = rows_result.rows::<(
                Uuid, Uuid, String, String, i32, i32, f32, i32, i32, i16
            )>() {
                for row_result in rows {
                    if let Ok((cid, did, callsign, platform, total, hits, acc, streak, best, rank)) = row_result {
                        return Ok(Some(LeaderboardEntry {
                            convoy_id: cid,
                            drone_id: did,
                            callsign,
                            platform_type: parse_platform_type(&platform),
                            total_engagements: total,
                            successful_hits: hits,
                            accuracy_pct: acc,
                            current_streak: streak,
                            best_streak: best,
                            rank,
                            updated_at: Utc::now(),
                        }));
                    }
                }
            }
        }

        Ok(None)
    }
}

// =============================================================================
// ENGAGEMENT REPOSITORY
// =============================================================================

/// Repository for engagement operations.
pub struct ScyllaEngagementRepository {
    client: Arc<ScyllaClient>,
}

impl ScyllaEngagementRepository {
    /// Create a new engagement repository.
    pub fn new(client: Arc<ScyllaClient>) -> Self {
        Self { client }
    }

    /// Record a new engagement.
    pub async fn record(&self, engagement: &Engagement) -> Result<()> {
        let query = r#"
            INSERT INTO engagements (
                convoy_id, engaged_at, engagement_id, drone_id, drone_callsign,
                weapon_type, hit, range_to_target_km, bda_status
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;

        // Convert DateTime to milliseconds for CQL timestamp
        let engaged_at_ms = engagement.engaged_at.timestamp_millis();

        self.client.session
            .query_unpaged(
                query,
                (
                    engagement.convoy_id,
                    engaged_at_ms,
                    engagement.engagement_id,
                    engagement.drone_id,
                    &engagement.drone_callsign,
                    engagement.weapon_type.as_str(),
                    engagement.hit,
                    engagement.range_to_target_km,
                    &engagement.bda_status,
                ),
            )
            .await?;

        Ok(())
    }

    /// Get engagements for a drone (stub - returns empty).
    pub async fn get_by_drone(
        &self,
        _convoy_id: Uuid,
        _drone_id: Uuid,
        _limit: i32,
    ) -> Result<Vec<Engagement>> {
        // TODO: Implement full parsing of complex Engagement type
        Ok(Vec::new())
    }
}

// =============================================================================
// TELEMETRY REPOSITORY
// =============================================================================

/// Repository for telemetry operations.
pub struct ScyllaTelemetryRepository {
    client: Arc<ScyllaClient>,
}

impl ScyllaTelemetryRepository {
    /// Create a new telemetry repository.
    pub fn new(client: Arc<ScyllaClient>) -> Self {
        Self { client }
    }

    /// Record telemetry snapshot.
    pub async fn record(&self, telemetry: &Telemetry) -> Result<()> {
        let query = r#"
            INSERT INTO telemetry (
                drone_id, time_bucket, recorded_at,
                latitude, longitude, altitude_m, heading_deg, speed_mps,
                velocity_mps, fuel_remaining_pct, engine_rpm, engine_temp_c
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            USING TTL 86400
        "#;

        // Convert DateTime to milliseconds for CQL timestamp
        let recorded_at_ms = telemetry.recorded_at.timestamp_millis();

        self.client.session
            .query_unpaged(
                query,
                (
                    telemetry.drone_id,
                    &telemetry.time_bucket,
                    recorded_at_ms,
                    telemetry.position.latitude,
                    telemetry.position.longitude,
                    telemetry.position.altitude_m,
                    telemetry.position.heading_deg,
                    telemetry.position.speed_mps,
                    telemetry.velocity_mps,
                    telemetry.fuel_remaining_pct,
                    telemetry.engine_rpm,
                    telemetry.engine_temp_c,
                ),
            )
            .await?;

        Ok(())
    }

    /// Get latest telemetry for a drone (stub - returns None).
    pub async fn get_latest(&self, _drone_id: Uuid) -> Result<Option<Telemetry>> {
        // TODO: Implement full parsing of complex Telemetry type
        Ok(None)
    }
}

// =============================================================================
// CONVOY REPOSITORY
// =============================================================================

/// Repository for convoy operations.
pub struct ScyllaConvoyRepository {
    client: Arc<ScyllaClient>,
}

impl ScyllaConvoyRepository {
    /// Create a new convoy repository.
    pub fn new(client: Arc<ScyllaClient>) -> Self {
        Self { client }
    }

    /// Get convoy by ID (stub - returns None).
    pub async fn get(&self, _convoy_id: Uuid) -> Result<Option<Convoy>> {
        // TODO: Implement full parsing of complex Convoy type
        Ok(None)
    }

    /// Create a new convoy.
    pub async fn create(&self, convoy: &Convoy) -> Result<()> {
        // Convert DateTime to milliseconds for CQL timestamp
        let created_at_ms = convoy.created_at.timestamp_millis();
        let mission_start_ms = convoy.mission_start.map(|dt| dt.timestamp_millis());
        let mission_end_ms = convoy.mission_end.map(|dt| dt.timestamp_millis());

        let query = r#"
            INSERT INTO convoys (
                convoy_id, convoy_callsign, mission_id, mission_type, status,
                created_at, mission_start, mission_end,
                aor_name, aor_center_lat, aor_center_lon, aor_radius_km,
                commanding_unit, authorization_level, roe_profile,
                drone_count
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;

        self.client.session
            .query_unpaged(
                query,
                (
                    convoy.convoy_id,
                    &convoy.convoy_callsign,
                    convoy.mission_id,
                    mission_type_str(&convoy.mission_type),
                    convoy_status_str(&convoy.status),
                    created_at_ms,
                    mission_start_ms,
                    mission_end_ms,
                    &convoy.aor_name,
                    convoy.aor_center.latitude,
                    convoy.aor_center.longitude,
                    convoy.aor_radius_km,
                    &convoy.commanding_unit,
                    &convoy.authorization_level,
                    &convoy.roe_profile,
                    convoy.drone_count,
                ),
            )
            .await?;

        Ok(())
    }
}

// =============================================================================
// WAYPOINT REPOSITORY  
// =============================================================================

/// Repository for waypoint operations.
pub struct ScyllaWaypointRepository {
    client: Arc<ScyllaClient>,
}

impl ScyllaWaypointRepository {
    /// Create a new waypoint repository.
    pub fn new(client: Arc<ScyllaClient>) -> Self {
        Self { client }
    }

    /// Get waypoints for a drone (stub - returns empty).
    pub async fn get_waypoints(&self, _drone_id: Uuid) -> Result<Vec<Waypoint>> {
        // TODO: Implement full parsing of complex Waypoint type
        Ok(Vec::new())
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

fn parse_platform_type(s: &str) -> PlatformType {
    match s {
        "MQ-9_REAPER" | "MQ9_REAPER" => PlatformType::Mq9Reaper,
        "MQ-1C_GRAY_EAGLE" | "MQ1C_GRAY_EAGLE" => PlatformType::Mq1cGrayEagle,
        "RQ-4_GLOBAL_HAWK" | "RQ4_GLOBAL_HAWK" => PlatformType::Rq4GlobalHawk,
        "MQ-25_STINGRAY" | "MQ25_STINGRAY" => PlatformType::Mq25Stingray,
        _ => PlatformType::Mq9Reaper,
    }
}

fn target_type_str(t: &TargetType) -> &'static str {
    match t {
        TargetType::Vehicle => "VEHICLE",
        TargetType::Structure => "STRUCTURE",
        TargetType::Personnel => "PERSONNEL",
        TargetType::Radar => "RADAR",
        TargetType::AirDefense => "AIR_DEFENSE",
        TargetType::Supply => "SUPPLY",
    }
}

fn mission_type_str(t: &MissionType) -> &'static str {
    match t {
        MissionType::Isr => "ISR",
        MissionType::Strike => "STRIKE",
        MissionType::Escort => "ESCORT",
        MissionType::Resupply => "RESUPPLY",
        MissionType::Sar => "SAR",
    }
}

fn convoy_status_str(s: &ConvoyStatus) -> &'static str {
    match s {
        ConvoyStatus::Planning => "PLANNING",
        ConvoyStatus::Active => "ACTIVE",
        ConvoyStatus::Rtb => "RTB",
        ConvoyStatus::Complete => "COMPLETE",
        ConvoyStatus::Abort => "ABORT",
    }
}
