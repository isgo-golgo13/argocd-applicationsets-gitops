//! ScyllaDB repository implementation.
//!
//! Provides repository pattern access to ScyllaDB for drone convoy entities.
//!
//! ## Hard-won CQL facts encoded here — do not "simplify" them away
//!
//! - `leaderboard`'s PRIMARY KEY is `(convoy_id, accuracy_pct, drone_id)`.
//!   `accuracy_pct` is a CLUSTERING column: it cannot appear in a `SET` clause,
//!   and a row's accuracy change moves the row. Updates are therefore
//!   read → DELETE old row → INSERT new row (batched, same partition).
//! - The scylla 0.15 typed row API is strict: the tuple passed to
//!   `rows::<T>()` must match the SELECT column list exactly, in count and
//!   type. An 11-column SELECT read into a 10-field tuple fails the type
//!   check — and swallowing that error with `if let Ok(..)` is how this
//!   dashboard stayed empty for a week. Every error here propagates.
//! - CQL `timestamp` binds/reads as [`CqlTimestamp`] (millis), never bare
//!   `i64` — bare `i64` type-checks only against `bigint`.
//! - `telemetry.position` and friends are frozen UDTs; the mirror structs in
//!   [`udt`] must keep field names identical to the CQL type definitions.

use chrono::{DateTime, TimeZone, Utc};
use scylla::batch::{Batch, BatchType};
use scylla::frame::value::CqlTimestamp;
use scylla::{DeserializeValue, SerializeValue, Session, SessionBuilder};
use std::sync::Arc;
use uuid::Uuid;

use crate::cache::SharedCacheClient;
use crate::error::{PersistenceError, Result};
use crate::strategy::{ReadStrategy, WriteStrategy};
use drone_domain::{
    CollateralRisk, Convoy, ConvoyStatus, Coordinates, DamageAssessment, Drone, DroneStatus,
    Engagement, EngagementResult, LeaderboardEntry, MissionType, PlatformType, TargetInfo,
    TargetType, Telemetry, ThreatLevel, Waypoint, WaypointStatus, WaypointType, WeaponType,
};

// =============================================================================
// UDT MIRRORS
// =============================================================================

/// Mirrors of the CQL user-defined types.
///
/// Field names MUST match the UDT definitions in `schema/cql/001_core_schema.cql`
/// exactly — the derive macros match by name.
mod udt {
    use super::{DeserializeValue, SerializeValue};

    /// CQL `coordinates` UDT.
    #[derive(Debug, Clone, SerializeValue, DeserializeValue)]
    pub struct CoordinatesUdt {
        pub latitude: f64,
        pub longitude: f64,
        pub altitude_m: f64,
        pub heading_deg: f32,
        pub speed_mps: f32,
    }

    /// CQL `target_info` UDT.
    #[derive(Debug, Clone, SerializeValue, DeserializeValue)]
    pub struct TargetInfoUdt {
        pub target_id: uuid::Uuid,
        pub target_type: String,
        pub coordinates: CoordinatesUdt,
        pub confidence: f32,
        pub threat_level: String,
    }
}

use udt::{CoordinatesUdt, TargetInfoUdt};

impl From<Coordinates> for CoordinatesUdt {
    fn from(c: Coordinates) -> Self {
        Self {
            latitude: c.latitude,
            longitude: c.longitude,
            altitude_m: c.altitude_m,
            heading_deg: c.heading_deg,
            speed_mps: c.speed_mps,
        }
    }
}

impl From<CoordinatesUdt> for Coordinates {
    fn from(c: CoordinatesUdt) -> Self {
        Self {
            latitude: c.latitude,
            longitude: c.longitude,
            altitude_m: c.altitude_m,
            heading_deg: c.heading_deg,
            speed_mps: c.speed_mps,
        }
    }
}

impl From<&TargetInfo> for TargetInfoUdt {
    fn from(t: &TargetInfo) -> Self {
        Self {
            target_id: t.target_id,
            target_type: target_type_str(&t.target_type).to_string(),
            coordinates: t.coordinates.into(),
            confidence: t.confidence,
            threat_level: threat_level_str(&t.threat_level).to_string(),
        }
    }
}

impl From<TargetInfoUdt> for TargetInfo {
    fn from(t: TargetInfoUdt) -> Self {
        Self {
            target_id: t.target_id,
            target_type: parse_target_type(&t.target_type),
            coordinates: t.coordinates.into(),
            confidence: t.confidence,
            threat_level: parse_threat_level(&t.threat_level),
        }
    }
}

// =============================================================================
// TIMESTAMP HELPERS
// =============================================================================

/// `DateTime<Utc>` → CQL timestamp (millis since epoch).
fn ts(dt: DateTime<Utc>) -> CqlTimestamp {
    CqlTimestamp(dt.timestamp_millis())
}

/// CQL timestamp → `DateTime<Utc>`, clamping out-of-range values to epoch.
fn from_ts(t: CqlTimestamp) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(t.0)
        .single()
        .unwrap_or_else(|| Utc.timestamp_millis_opt(0).earliest().unwrap_or_default())
}

/// Map any scylla-side error into our domain error without losing the message.
fn db_err<E: std::fmt::Display>(e: E) -> PersistenceError {
    PersistenceError::Scylla(e.to_string())
}

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
        let mut builder = SessionBuilder::new().known_nodes(&config.hosts);

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

/// Typed row shape shared by the leaderboard SELECTs.
/// Order matters: it must mirror `LEADERBOARD_COLUMNS`.
type LeaderboardRow = (
    Uuid,            // convoy_id
    f32,             // accuracy_pct (clustering key — never null)
    Uuid,            // drone_id
    Option<String>,  // callsign
    Option<String>,  // platform_type
    Option<i32>,     // total_engagements
    Option<i32>,     // successful_hits
    Option<i32>,     // current_streak
    Option<i32>,     // best_streak
    Option<CqlTimestamp>, // updated_at
);

const LEADERBOARD_COLUMNS: &str = "convoy_id, accuracy_pct, drone_id, callsign, platform_type, \
     total_engagements, successful_hits, current_streak, best_streak, updated_at";

fn leaderboard_entry_from_row(row: LeaderboardRow) -> LeaderboardEntry {
    let (cid, acc, did, callsign, platform, total, hits, streak, best, updated) = row;
    LeaderboardEntry {
        convoy_id: cid,
        drone_id: did,
        callsign: callsign.unwrap_or_else(|| "UNKNOWN".to_string()),
        platform_type: parse_platform_type(platform.as_deref().unwrap_or_default()),
        total_engagements: total.unwrap_or_default(),
        successful_hits: hits.unwrap_or_default(),
        accuracy_pct: acc,
        current_streak: streak.unwrap_or_default(),
        best_streak: best.unwrap_or_default(),
        rank: 0, // assigned by the reader from clustering order
        updated_at: updated.map(from_ts).unwrap_or_else(Utc::now),
    }
}

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

    /// Get leaderboard for a convoy, best accuracy first.
    ///
    /// The table clusters on `(accuracy_pct DESC, drone_id ASC)`, so the rows
    /// arrive already sorted; rank is the row's position, assigned here rather
    /// than trusted from the (stale) stored column.
    pub async fn get_leaderboard(
        &self,
        convoy_id: Uuid,
        limit: i32,
    ) -> Result<Vec<LeaderboardEntry>> {
        let query = format!(
            "SELECT {LEADERBOARD_COLUMNS} FROM leaderboard WHERE convoy_id = ? LIMIT ?"
        );

        let result = self
            .client
            .session
            .query_unpaged(query, (convoy_id, limit))
            .await?;

        let rows_result = result.into_rows_result().map_err(db_err)?;
        let rows = rows_result.rows::<LeaderboardRow>().map_err(db_err)?;

        let mut entries = Vec::new();
        for row in rows {
            let row = row.map_err(db_err)?;
            entries.push(leaderboard_entry_from_row(row));
        }
        for (i, entry) in entries.iter_mut().enumerate() {
            entry.rank = (i + 1) as i16;
        }

        Ok(entries)
    }

    /// Update leaderboard entry after an engagement.
    ///
    /// `accuracy_pct` is part of the primary key, so the previous row is
    /// deleted and a new row inserted at the new clustering position. Both
    /// statements target the same partition and run as one logged batch.
    pub async fn update_entry(
        &self,
        convoy_id: Uuid,
        drone_id: Uuid,
        callsign: &str,
        platform: PlatformType,
        hit: bool,
    ) -> Result<LeaderboardEntry> {
        let current = self.get_drone_entry(convoy_id, drone_id).await?;

        let (old_accuracy, total, hits, streak, best) = match &current {
            Some(e) => {
                let new_streak = if hit { e.current_streak + 1 } else { 0 };
                let new_best = new_streak.max(e.best_streak);
                (
                    Some(e.accuracy_pct),
                    e.total_engagements + 1,
                    if hit { e.successful_hits + 1 } else { e.successful_hits },
                    new_streak,
                    new_best,
                )
            }
            None => {
                let first = i32::from(hit);
                (None, 1, first, first, first)
            }
        };

        let accuracy = if total > 0 {
            (hits as f32 / total as f32) * 100.0
        } else {
            0.0
        };

        let insert = "INSERT INTO leaderboard (convoy_id, accuracy_pct, drone_id, callsign, \
             platform_type, total_engagements, successful_hits, current_streak, \
             best_streak, rank, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, toTimestamp(now()))";
        let insert_values = (
            convoy_id,
            accuracy,
            drone_id,
            callsign,
            platform.as_str(),
            total,
            hits,
            streak,
            best,
            0_i16,
        );

        match old_accuracy {
            // Existing row: delete at the old clustering position and insert
            // at the new one atomically (same partition — cheap batch).
            Some(old_acc) => {
                let mut batch = Batch::new(BatchType::Logged);
                batch.append_statement(
                    "DELETE FROM leaderboard WHERE convoy_id = ? AND accuracy_pct = ? AND drone_id = ?",
                );
                batch.append_statement(insert);
                self.client
                    .session
                    .batch(&batch, ((convoy_id, old_acc, drone_id), insert_values))
                    .await?;
            }
            // First engagement for this drone: plain insert.
            None => {
                self.client
                    .session
                    .query_unpaged(insert, insert_values)
                    .await?;
            }
        }

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
            rank: 0, // recalculated on read from clustering order
            updated_at: Utc::now(),
        })
    }

    /// Get a single drone's entry.
    ///
    /// `drone_id` is the LAST clustering column, so it cannot be restricted
    /// without also restricting `accuracy_pct` — which is the very value we
    /// don't know. `ALLOW FILTERING` here scans one small partition
    /// (one row per drone in the convoy), not the table.
    pub async fn get_drone_entry(
        &self,
        convoy_id: Uuid,
        drone_id: Uuid,
    ) -> Result<Option<LeaderboardEntry>> {
        let query = format!(
            "SELECT {LEADERBOARD_COLUMNS} FROM leaderboard \
             WHERE convoy_id = ? AND drone_id = ? ALLOW FILTERING"
        );

        let result = self
            .client
            .session
            .query_unpaged(query, (convoy_id, drone_id))
            .await?;

        let rows_result = result.into_rows_result().map_err(db_err)?;
        let mut rows = rows_result.rows::<LeaderboardRow>().map_err(db_err)?;

        match rows.next() {
            Some(row) => Ok(Some(leaderboard_entry_from_row(row.map_err(db_err)?))),
            None => Ok(None),
        }
    }
}

// =============================================================================
// ENGAGEMENT REPOSITORY
// =============================================================================

/// Typed row shape for engagement SELECTs (mirrors `ENGAGEMENT_COLUMNS`).
type EngagementRow = (
    Uuid,                   // convoy_id
    CqlTimestamp,           // engaged_at (clustering — never null)
    Uuid,                   // engagement_id
    Option<Uuid>,           // drone_id
    Option<String>,         // drone_callsign
    Option<String>,         // weapon_type
    Option<TargetInfoUdt>,  // target
    Option<bool>,           // hit
    Option<i16>,            // waypoint_number
    Option<CoordinatesUdt>, // shooter_position
    Option<f32>,            // range_to_target_km
    Option<String>,         // bda_status
    Option<String>,         // authorization_code
    Option<bool>,           // roe_compliance
);

const ENGAGEMENT_COLUMNS: &str = "convoy_id, engaged_at, engagement_id, drone_id, drone_callsign, \
     weapon_type, target, hit, waypoint_number, shooter_position, \
     range_to_target_km, bda_status, authorization_code, roe_compliance";

fn engagement_from_row(row: EngagementRow) -> Engagement {
    let (
        convoy_id,
        engaged_at,
        engagement_id,
        drone_id,
        drone_callsign,
        weapon_type,
        target,
        hit,
        waypoint_number,
        shooter_position,
        range_to_target_km,
        bda_status,
        authorization_code,
        roe_compliance,
    ) = row;

    let hit = hit.unwrap_or_default();
    let target_info = target.map(TargetInfo::from).unwrap_or_else(|| TargetInfo {
        target_id: Uuid::nil(),
        target_type: TargetType::Vehicle,
        coordinates: Coordinates::default(),
        confidence: 0.0,
        threat_level: ThreatLevel::Low,
    });
    Engagement {
        convoy_id,
        engaged_at: from_ts(engaged_at),
        engagement_id,
        drone_id: drone_id.unwrap_or_default(),
        drone_callsign: drone_callsign.unwrap_or_else(|| "UNKNOWN".to_string()),
        weapon_type: parse_weapon_type(weapon_type.as_deref().unwrap_or_default()),
        weapon_serial: String::new(),
        authorization_code: authorization_code.unwrap_or_default(),
        authorized_by: String::new(),
        roe_compliance: roe_compliance.unwrap_or(true),
        result: EngagementResult {
            impact_time: from_ts(engaged_at),
            impact_coords: target_info.coordinates,
            damage_assessment: if hit {
                DamageAssessment::PendingBda
            } else {
                DamageAssessment::Missed
            },
            collateral_risk: CollateralRisk::None,
        },
        target: target_info,
        hit,
        waypoint_number: waypoint_number.unwrap_or_default(),
        shooter_position: shooter_position
            .map(Coordinates::from)
            .unwrap_or_default(),
        range_to_target_km: range_to_target_km.unwrap_or_default(),
        bda_status: bda_status.unwrap_or_else(|| "PENDING".to_string()),
        bda_notes: None,
    }
}

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
    ///
    /// Dual-writes the convoy-partitioned `engagements` table (feeds the
    /// dashboard's engagement feed) and the drone-partitioned
    /// `engagements_by_drone` table (feeds per-drone history) — the standard
    /// query-table denormalization the schema was designed around.
    pub async fn record(&self, engagement: &Engagement) -> Result<()> {
        let target_udt = TargetInfoUdt::from(&engagement.target);
        let shooter_udt = CoordinatesUdt::from(engagement.shooter_position);

        let mut batch = Batch::new(BatchType::Logged);
        batch.append_statement(
            "INSERT INTO engagements (convoy_id, engaged_at, engagement_id, drone_id, \
             drone_callsign, weapon_type, target, hit, waypoint_number, shooter_position, \
             range_to_target_km, bda_status, authorization_code, roe_compliance) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        );
        batch.append_statement(
            "INSERT INTO engagements_by_drone (drone_id, engaged_at, engagement_id, \
             convoy_id, weapon_type, target, hit, range_to_target_km) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        );

        self.client
            .session
            .batch(
                &batch,
                (
                    (
                        engagement.convoy_id,
                        ts(engagement.engaged_at),
                        engagement.engagement_id,
                        engagement.drone_id,
                        &engagement.drone_callsign,
                        engagement.weapon_type.as_str(),
                        &target_udt,
                        engagement.hit,
                        engagement.waypoint_number,
                        &shooter_udt,
                        engagement.range_to_target_km,
                        &engagement.bda_status,
                        &engagement.authorization_code,
                        engagement.roe_compliance,
                    ),
                    (
                        engagement.drone_id,
                        ts(engagement.engaged_at),
                        engagement.engagement_id,
                        engagement.convoy_id,
                        engagement.weapon_type.as_str(),
                        &target_udt,
                        engagement.hit,
                        engagement.range_to_target_km,
                    ),
                ),
            )
            .await?;

        Ok(())
    }

    /// Most recent engagements for a convoy, newest first
    /// (`engaged_at DESC` clustering order).
    pub async fn get_recent(&self, convoy_id: Uuid, limit: i32) -> Result<Vec<Engagement>> {
        let query = format!(
            "SELECT {ENGAGEMENT_COLUMNS} FROM engagements WHERE convoy_id = ? LIMIT ?"
        );

        let result = self
            .client
            .session
            .query_unpaged(query, (convoy_id, limit))
            .await?;

        let rows_result = result.into_rows_result().map_err(db_err)?;
        let rows = rows_result.rows::<EngagementRow>().map_err(db_err)?;

        let mut engagements = Vec::new();
        for row in rows {
            engagements.push(engagement_from_row(row.map_err(db_err)?));
        }
        Ok(engagements)
    }

    /// Engagement history for one drone, newest first.
    pub async fn get_by_drone(&self, drone_id: Uuid, limit: i32) -> Result<Vec<Engagement>> {
        let query = "SELECT drone_id, engaged_at, engagement_id, convoy_id, weapon_type, \
             target, hit, range_to_target_km \
             FROM engagements_by_drone WHERE drone_id = ? LIMIT ?";

        let result = self
            .client
            .session
            .query_unpaged(query, (drone_id, limit))
            .await?;

        let rows_result = result.into_rows_result().map_err(db_err)?;
        let rows = rows_result
            .rows::<(
                Uuid,
                CqlTimestamp,
                Uuid,
                Option<Uuid>,
                Option<String>,
                Option<TargetInfoUdt>,
                Option<bool>,
                Option<f32>,
            )>()
            .map_err(db_err)?;

        let mut engagements = Vec::new();
        for row in rows {
            let (did, engaged_at, eid, cid, weapon, target, hit, range) =
                row.map_err(db_err)?;
            let hit = hit.unwrap_or_default();
            let target_info = target.map(TargetInfo::from).unwrap_or_else(|| TargetInfo {
                target_id: Uuid::nil(),
                target_type: TargetType::Vehicle,
                coordinates: Coordinates::default(),
                confidence: 0.0,
                threat_level: ThreatLevel::Low,
            });
            engagements.push(Engagement {
                convoy_id: cid.unwrap_or_default(),
                engaged_at: from_ts(engaged_at),
                engagement_id: eid,
                drone_id: did,
                drone_callsign: String::new(),
                weapon_type: parse_weapon_type(weapon.as_deref().unwrap_or_default()),
                weapon_serial: String::new(),
                authorization_code: String::new(),
                authorized_by: String::new(),
                roe_compliance: true,
                result: EngagementResult {
                    impact_time: from_ts(engaged_at),
                    impact_coords: target_info.coordinates,
                    damage_assessment: if hit {
                        DamageAssessment::PendingBda
                    } else {
                        DamageAssessment::Missed
                    },
                    collateral_risk: CollateralRisk::None,
                },
                target: target_info,
                hit,
                waypoint_number: 0,
                shooter_position: Coordinates::default(),
                range_to_target_km: range.unwrap_or_default(),
                bda_status: "PENDING".to_string(),
                bda_notes: None,
            });
        }
        Ok(engagements)
    }
}

// =============================================================================
// TELEMETRY REPOSITORY
// =============================================================================

/// Typed row shape for telemetry SELECTs (mirrors `TELEMETRY_COLUMNS`).
type TelemetryRow = (
    Uuid,                   // drone_id
    String,                 // time_bucket
    CqlTimestamp,           // recorded_at (clustering — never null)
    Option<CoordinatesUdt>, // position
    Option<f32>,            // velocity_mps
    Option<i16>,            // current_waypoint
    Option<f32>,            // distance_to_next_km
    Option<f32>,            // fuel_remaining_pct
    Option<i32>,            // engine_rpm
    Option<f32>,            // engine_temp_c
    Option<f32>,            // mesh_connectivity
);

const TELEMETRY_COLUMNS: &str = "drone_id, time_bucket, recorded_at, position, velocity_mps, \
     current_waypoint, distance_to_next_km, fuel_remaining_pct, \
     engine_rpm, engine_temp_c, mesh_connectivity";

fn telemetry_from_row(row: TelemetryRow) -> Telemetry {
    let (
        drone_id,
        time_bucket,
        recorded_at,
        position,
        velocity_mps,
        current_waypoint,
        distance_to_next_km,
        fuel_remaining_pct,
        engine_rpm,
        engine_temp_c,
        mesh_connectivity,
    ) = row;

    Telemetry {
        drone_id,
        time_bucket,
        recorded_at: from_ts(recorded_at),
        position: position.map(Coordinates::from).unwrap_or_default(),
        velocity_mps: velocity_mps.unwrap_or_default(),
        acceleration_mps2: 0.0,
        bank_angle_deg: 0.0,
        pitch_angle_deg: 0.0,
        current_waypoint: current_waypoint.unwrap_or_default(),
        distance_to_next_km: distance_to_next_km.unwrap_or_default(),
        eta_next_waypoint: None,
        fuel_remaining_pct: fuel_remaining_pct.unwrap_or_default(),
        engine_rpm: engine_rpm.unwrap_or_default(),
        engine_temp_c: engine_temp_c.unwrap_or_default(),
        battery_voltage: 0.0,
        wind_speed_mps: 0.0,
        wind_direction_deg: 0.0,
        temperature_c: 0.0,
        visibility_km: 0.0,
        link_status: None,
        mesh_connectivity: mesh_connectivity.unwrap_or(1.0),
    }
}

/// Repository for telemetry operations.
pub struct ScyllaTelemetryRepository {
    client: Arc<ScyllaClient>,
}

impl ScyllaTelemetryRepository {
    /// Create a new telemetry repository.
    pub fn new(client: Arc<ScyllaClient>) -> Self {
        Self { client }
    }

    /// Record a telemetry snapshot.
    ///
    /// Column list matches the `telemetry` table as defined in the schema:
    /// `position` is a frozen `coordinates` UDT, not flattened lat/lon columns.
    pub async fn record(&self, telemetry: &Telemetry) -> Result<()> {
        let position = CoordinatesUdt::from(telemetry.position);

        let query = "INSERT INTO telemetry (drone_id, time_bucket, recorded_at, position, \
             velocity_mps, current_waypoint, distance_to_next_km, fuel_remaining_pct, \
             engine_rpm, engine_temp_c, mesh_connectivity) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) USING TTL 86400";

        self.client
            .session
            .query_unpaged(
                query,
                (
                    telemetry.drone_id,
                    &telemetry.time_bucket,
                    ts(telemetry.recorded_at),
                    &position,
                    telemetry.velocity_mps,
                    telemetry.current_waypoint,
                    telemetry.distance_to_next_km,
                    telemetry.fuel_remaining_pct,
                    telemetry.engine_rpm,
                    telemetry.engine_temp_c,
                    telemetry.mesh_connectivity,
                ),
            )
            .await?;

        Ok(())
    }

    /// Latest telemetry for a drone.
    ///
    /// The partition key is `(drone_id, time_bucket)` with hourly buckets, so
    /// the current bucket is queried first and the previous bucket is the
    /// fallback across the top of the hour.
    pub async fn get_latest(&self, drone_id: Uuid) -> Result<Option<Telemetry>> {
        for bucket in Self::recent_buckets() {
            if let Some(t) = self.latest_in_bucket(drone_id, &bucket).await? {
                return Ok(Some(t));
            }
        }
        Ok(None)
    }

    /// Recent telemetry history for a drone, newest first, spanning the
    /// current and previous hourly buckets.
    pub async fn get_history(&self, drone_id: Uuid, limit: i32) -> Result<Vec<Telemetry>> {
        let mut history = Vec::new();
        for bucket in Self::recent_buckets() {
            let remaining = limit - history.len() as i32;
            if remaining <= 0 {
                break;
            }
            let query = format!(
                "SELECT {TELEMETRY_COLUMNS} FROM telemetry \
                 WHERE drone_id = ? AND time_bucket = ? LIMIT ?"
            );
            let result = self
                .client
                .session
                .query_unpaged(query, (drone_id, &bucket, remaining))
                .await?;
            let rows_result = result.into_rows_result().map_err(db_err)?;
            let rows = rows_result.rows::<TelemetryRow>().map_err(db_err)?;
            for row in rows {
                history.push(telemetry_from_row(row.map_err(db_err)?));
            }
        }
        Ok(history)
    }

    async fn latest_in_bucket(&self, drone_id: Uuid, bucket: &str) -> Result<Option<Telemetry>> {
        let query = format!(
            "SELECT {TELEMETRY_COLUMNS} FROM telemetry \
             WHERE drone_id = ? AND time_bucket = ? LIMIT 1"
        );
        let result = self
            .client
            .session
            .query_unpaged(query, (drone_id, bucket))
            .await?;
        let rows_result = result.into_rows_result().map_err(db_err)?;
        let mut rows = rows_result.rows::<TelemetryRow>().map_err(db_err)?;
        match rows.next() {
            Some(row) => Ok(Some(telemetry_from_row(row.map_err(db_err)?))),
            None => Ok(None),
        }
    }

    /// Current and previous hourly bucket keys.
    fn recent_buckets() -> [String; 2] {
        let now = Utc::now();
        let prev = now - chrono::Duration::hours(1);
        [
            Telemetry::generate_time_bucket(&now),
            Telemetry::generate_time_bucket(&prev),
        ]
    }
}

// =============================================================================
// CONVOY REPOSITORY
// =============================================================================

/// Typed row shape for convoy SELECTs (mirrors `CONVOY_COLUMNS`).
type ConvoyRow = (
    Uuid,                   // convoy_id
    Option<String>,         // convoy_callsign
    Option<Uuid>,           // mission_id
    Option<String>,         // mission_type
    Option<String>,         // status
    Option<CqlTimestamp>,   // created_at
    Option<CqlTimestamp>,   // mission_start
    Option<CqlTimestamp>,   // mission_end
    Option<String>,         // aor_name
    Option<CoordinatesUdt>, // aor_center
    Option<f32>,            // aor_radius_km
    Option<String>,         // commanding_unit
    Option<String>,         // authorization_level
    Option<String>,         // roe_profile
    Option<i16>,            // drone_count
);

const CONVOY_COLUMNS: &str = "convoy_id, convoy_callsign, mission_id, mission_type, status, \
     created_at, mission_start, mission_end, aor_name, aor_center, \
     aor_radius_km, commanding_unit, authorization_level, roe_profile, drone_count";

fn convoy_from_row(row: ConvoyRow) -> Convoy {
    let (
        convoy_id,
        convoy_callsign,
        mission_id,
        mission_type,
        status,
        created_at,
        mission_start,
        mission_end,
        aor_name,
        aor_center,
        aor_radius_km,
        commanding_unit,
        authorization_level,
        roe_profile,
        drone_count,
    ) = row;

    Convoy {
        convoy_id,
        convoy_callsign: convoy_callsign.unwrap_or_else(|| "CONVOY".to_string()),
        mission_id: mission_id.unwrap_or_default(),
        mission_type: parse_mission_type(mission_type.as_deref().unwrap_or_default()),
        status: parse_convoy_status(status.as_deref().unwrap_or_default()),
        created_at: created_at.map(from_ts).unwrap_or_else(Utc::now),
        mission_start: mission_start.map(from_ts),
        mission_end: mission_end.map(from_ts),
        aor_name: aor_name.unwrap_or_default(),
        aor_center: aor_center.map(Coordinates::from).unwrap_or_default(),
        aor_radius_km: aor_radius_km.unwrap_or_default(),
        commanding_unit: commanding_unit.unwrap_or_default(),
        authorization_level: authorization_level.unwrap_or_default(),
        roe_profile: roe_profile.unwrap_or_default(),
        drone_ids: Vec::new(),
        drone_count: drone_count.unwrap_or_default(),
    }
}

/// Repository for convoy operations.
pub struct ScyllaConvoyRepository {
    client: Arc<ScyllaClient>,
}

impl ScyllaConvoyRepository {
    /// Create a new convoy repository.
    pub fn new(client: Arc<ScyllaClient>) -> Self {
        Self { client }
    }

    /// Get convoy by ID.
    pub async fn get(&self, convoy_id: Uuid) -> Result<Option<Convoy>> {
        let query = format!("SELECT {CONVOY_COLUMNS} FROM convoys WHERE convoy_id = ?");
        let result = self
            .client
            .session
            .query_unpaged(query, (convoy_id,))
            .await?;
        let rows_result = result.into_rows_result().map_err(db_err)?;
        let mut rows = rows_result.rows::<ConvoyRow>().map_err(db_err)?;
        match rows.next() {
            Some(row) => Ok(Some(convoy_from_row(row.map_err(db_err)?))),
            None => Ok(None),
        }
    }

    /// List all convoys.
    ///
    /// Full-table scan by design: `convoys` holds a handful of rows (one per
    /// mission), not telemetry-scale data.
    pub async fn list(&self) -> Result<Vec<Convoy>> {
        let query = format!("SELECT {CONVOY_COLUMNS} FROM convoys");
        let result = self.client.session.query_unpaged(query, ()).await?;
        let rows_result = result.into_rows_result().map_err(db_err)?;
        let rows = rows_result.rows::<ConvoyRow>().map_err(db_err)?;
        let mut convoys = Vec::new();
        for row in rows {
            convoys.push(convoy_from_row(row.map_err(db_err)?));
        }
        Ok(convoys)
    }

    /// Create (or upsert) a convoy.
    pub async fn create(&self, convoy: &Convoy) -> Result<()> {
        let query = "INSERT INTO convoys (convoy_id, convoy_callsign, mission_id, mission_type, \
             status, created_at, mission_start, mission_end, aor_name, aor_center, \
             aor_radius_km, commanding_unit, authorization_level, \
             roe_profile, drone_count) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

        let aor_center = CoordinatesUdt::from(convoy.aor_center);
        self.client
            .session
            .query_unpaged(
                query,
                (
                    convoy.convoy_id,
                    &convoy.convoy_callsign,
                    convoy.mission_id,
                    mission_type_str(&convoy.mission_type),
                    convoy_status_str(&convoy.status),
                    ts(convoy.created_at),
                    convoy.mission_start.map(ts),
                    convoy.mission_end.map(ts),
                    &convoy.aor_name,
                    &aor_center,
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

    /// Update convoy status.
    pub async fn update_status(&self, convoy_id: Uuid, status: ConvoyStatus) -> Result<()> {
        self.client
            .session
            .query_unpaged(
                "UPDATE convoys SET status = ? WHERE convoy_id = ?",
                (convoy_status_str(&status), convoy_id),
            )
            .await?;
        Ok(())
    }

    /// Retask a convoy to a new area of responsibility.
    ///
    /// This is the TASKING ORDER: the dashboard writes it, whatever flies
    /// the drones reads it. `aor_name` carries the theater slug (the shared
    /// `drone_domain::theaters` table is the vocabulary); `aor_center` is
    /// the theater centre. Both columns already exist -- no schema change.
    /// (`convoys` has no `updated_at`; watchers compare the slug, not a time.)
    pub async fn retask(&self, convoy_id: Uuid, aor_name: &str, aor_center: &Coordinates) -> Result<()> {
        self.client
            .session
            .query_unpaged(
                "UPDATE convoys SET aor_name = ?, aor_center = ? WHERE convoy_id = ?",
                (aor_name, CoordinatesUdt::from(aor_center.clone()), convoy_id),
            )
            .await?;
        Ok(())
    }
}

// =============================================================================
// DRONE REPOSITORY
// =============================================================================

/// Typed row shape for drone SELECTs (mirrors `DRONE_COLUMNS`).
type DroneRow = (
    Uuid,                   // convoy_id
    Uuid,                   // drone_id
    Option<String>,         // tail_number
    Option<String>,         // callsign
    Option<String>,         // platform_type
    Option<String>,         // serial_number
    Option<String>,         // status
    Option<CoordinatesUdt>, // current_position
    Option<f32>,            // fuel_remaining_pct
    Option<i32>,            // total_engagements
    Option<i32>,            // successful_hits
    Option<f32>,            // accuracy_pct
    Option<i16>,            // current_waypoint
    Option<i16>,            // total_waypoints
    Option<CqlTimestamp>,   // created_at
    Option<CqlTimestamp>,   // updated_at
);

const DRONE_COLUMNS: &str = "convoy_id, drone_id, tail_number, callsign, platform_type, \
     serial_number, status, current_position, fuel_remaining_pct, total_engagements, \
     successful_hits, accuracy_pct, current_waypoint, total_waypoints, created_at, updated_at";

/// A drone row plus the mission-progress columns the domain `Drone` does not
/// carry (they live on the wire and in the dashboard, not in the core entity).
#[derive(Debug, Clone)]
pub struct DroneRecord {
    pub drone: Drone,
    pub current_waypoint: i16,
    pub total_waypoints: i16,
}

fn drone_from_row(row: DroneRow) -> DroneRecord {
    let (
        convoy_id,
        drone_id,
        tail_number,
        callsign,
        platform_type,
        serial_number,
        status,
        current_position,
        fuel_remaining_pct,
        total_engagements,
        successful_hits,
        accuracy_pct,
        current_waypoint,
        total_waypoints,
        created_at,
        updated_at,
    ) = row;

    DroneRecord {
        drone: Drone {
            convoy_id,
            drone_id,
            tail_number: tail_number.unwrap_or_default(),
            callsign: callsign.unwrap_or_else(|| "UNKNOWN".to_string()),
            platform_type: parse_platform_type(platform_type.as_deref().unwrap_or_default()),
            serial_number: serial_number.unwrap_or_default(),
            status: parse_drone_status(status.as_deref().unwrap_or_default()),
            current_position: current_position.map(Coordinates::from).unwrap_or_default(),
            fuel_remaining_pct: fuel_remaining_pct.unwrap_or(100.0),
            flight_time_hrs: 0.0,
            weapons: Vec::new(),
            sensors: Vec::new(),
            primary_link: None,
            backup_link: None,
            mesh_neighbors: Vec::new(),
            total_engagements: total_engagements.unwrap_or_default(),
            successful_hits: successful_hits.unwrap_or_default(),
            accuracy_pct: accuracy_pct.unwrap_or_default(),
            created_at: created_at.map(from_ts).unwrap_or_else(Utc::now),
            updated_at: updated_at.map(from_ts).unwrap_or_else(Utc::now),
        },
        current_waypoint: current_waypoint.unwrap_or_default(),
        total_waypoints: total_waypoints.unwrap_or_default(),
    }
}

/// Repository for drone state.
///
/// This repository did not exist before; every "drone" the API returned was a
/// hardcoded struct. It is the source for the dashboard's drone cards, convoy
/// status aggregates, and the callsign/platform used on leaderboard entries.
pub struct ScyllaDroneRepository {
    client: Arc<ScyllaClient>,
}

impl ScyllaDroneRepository {
    /// Create a new drone repository.
    pub fn new(client: Arc<ScyllaClient>) -> Self {
        Self { client }
    }

    /// Upsert a drone's live state. CQL INSERT is an upsert, so the same
    /// statement serves registration and every subsequent state tick.
    pub async fn upsert_state(&self, record: &DroneRecord) -> Result<()> {
        let d = &record.drone;
        let position = CoordinatesUdt::from(d.current_position);

        let query = "INSERT INTO drones (convoy_id, drone_id, tail_number, callsign, \
             platform_type, serial_number, status, current_position, fuel_remaining_pct, \
             total_engagements, successful_hits, accuracy_pct, current_waypoint, \
             total_waypoints, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, toTimestamp(now()))";

        self.client
            .session
            .query_unpaged(
                query,
                (
                    d.convoy_id,
                    d.drone_id,
                    &d.tail_number,
                    &d.callsign,
                    d.platform_type.as_str(),
                    &d.serial_number,
                    drone_status_str(&d.status),
                    &position,
                    d.fuel_remaining_pct,
                    d.total_engagements,
                    d.successful_hits,
                    d.accuracy_pct,
                    record.current_waypoint,
                    record.total_waypoints,
                    ts(d.created_at),
                ),
            )
            .await?;

        Ok(())
    }

    /// Update only the accuracy counters (after an engagement). All targeted
    /// columns are regular columns and the WHERE names the full primary key,
    /// so a plain UPDATE is legal here — unlike on `leaderboard`.
    pub async fn update_stats(
        &self,
        convoy_id: Uuid,
        drone_id: Uuid,
        total_engagements: i32,
        successful_hits: i32,
        accuracy_pct: f32,
    ) -> Result<()> {
        self.client
            .session
            .query_unpaged(
                "UPDATE drones SET total_engagements = ?, successful_hits = ?, \
                 accuracy_pct = ?, updated_at = toTimestamp(now()) \
                 WHERE convoy_id = ? AND drone_id = ?",
                (
                    total_engagements,
                    successful_hits,
                    accuracy_pct,
                    convoy_id,
                    drone_id,
                ),
            )
            .await?;
        Ok(())
    }

    /// All drones in a convoy (single-partition read).
    pub async fn list_by_convoy(&self, convoy_id: Uuid) -> Result<Vec<DroneRecord>> {
        let query = format!("SELECT {DRONE_COLUMNS} FROM drones WHERE convoy_id = ?");
        let result = self
            .client
            .session
            .query_unpaged(query, (convoy_id,))
            .await?;
        let rows_result = result.into_rows_result().map_err(db_err)?;
        let rows = rows_result.rows::<DroneRow>().map_err(db_err)?;
        let mut drones = Vec::new();
        for row in rows {
            drones.push(drone_from_row(row.map_err(db_err)?));
        }
        Ok(drones)
    }

    /// One drone by full primary key.
    pub async fn get(&self, convoy_id: Uuid, drone_id: Uuid) -> Result<Option<DroneRecord>> {
        let query =
            format!("SELECT {DRONE_COLUMNS} FROM drones WHERE convoy_id = ? AND drone_id = ?");
        let result = self
            .client
            .session
            .query_unpaged(query, (convoy_id, drone_id))
            .await?;
        let rows_result = result.into_rows_result().map_err(db_err)?;
        let mut rows = rows_result.rows::<DroneRow>().map_err(db_err)?;
        match rows.next() {
            Some(row) => Ok(Some(drone_from_row(row.map_err(db_err)?))),
            None => Ok(None),
        }
    }
}

// =============================================================================
// WAYPOINT REPOSITORY
// =============================================================================

/// Typed row shape for waypoint SELECTs (mirrors `WAYPOINT_COLUMNS`).
type WaypointRow = (
    Uuid,                   // drone_id
    i16,                    // sequence_number (clustering — never null)
    Option<Uuid>,           // waypoint_id
    Option<String>,         // waypoint_name
    Option<String>,         // waypoint_type
    Option<CoordinatesUdt>, // coordinates
    Option<CqlTimestamp>,   // planned_arrival
    Option<CqlTimestamp>,   // actual_arrival
    Option<CqlTimestamp>,   // planned_departure
    Option<CqlTimestamp>,   // actual_departure
    Option<i32>,            // loiter_duration_min
    Option<String>,         // status
);

const WAYPOINT_COLUMNS: &str = "drone_id, sequence_number, waypoint_id, waypoint_name, \
     waypoint_type, coordinates, planned_arrival, actual_arrival, planned_departure, \
     actual_departure, loiter_duration_min, status";

/// Repository for waypoint operations.
pub struct ScyllaWaypointRepository {
    client: Arc<ScyllaClient>,
}

impl ScyllaWaypointRepository {
    /// Create a new waypoint repository.
    pub fn new(client: Arc<ScyllaClient>) -> Self {
        Self { client }
    }

    /// Get waypoints for a drone in sequence order.
    pub async fn get_waypoints(&self, drone_id: Uuid) -> Result<Vec<Waypoint>> {
        let query = format!("SELECT {WAYPOINT_COLUMNS} FROM waypoints WHERE drone_id = ?");
        let result = self
            .client
            .session
            .query_unpaged(query, (drone_id,))
            .await?;
        let rows_result = result.into_rows_result().map_err(db_err)?;
        let rows = rows_result.rows::<WaypointRow>().map_err(db_err)?;

        let mut waypoints = Vec::new();
        for row in rows {
            let (
                did,
                sequence_number,
                waypoint_id,
                waypoint_name,
                waypoint_type,
                coordinates,
                planned_arrival,
                actual_arrival,
                planned_departure,
                actual_departure,
                loiter_duration_min,
                status,
            ) = row.map_err(db_err)?;

            waypoints.push(Waypoint {
                drone_id: did,
                sequence_number,
                waypoint_id: waypoint_id.unwrap_or_default(),
                waypoint_name: waypoint_name.unwrap_or_default(),
                waypoint_type: parse_waypoint_type(waypoint_type.as_deref().unwrap_or_default()),
                coordinates: coordinates.map(Coordinates::from).unwrap_or_default(),
                planned_arrival: planned_arrival.map(from_ts),
                actual_arrival: actual_arrival.map(from_ts),
                planned_departure: planned_departure.map(from_ts),
                actual_departure: actual_departure.map(from_ts),
                loiter_duration_min,
                authorized_actions: Vec::new(),
                status: parse_waypoint_status(status.as_deref().unwrap_or_default()),
            });
        }
        Ok(waypoints)
    }

    /// Insert a drone's route. One statement per waypoint (≤ ~25 rows); a
    /// cross-row batch would span nothing here since the partition key is the
    /// drone, but sequential awaits keep the error paths obvious.
    pub async fn create_batch(&self, waypoints: &[Waypoint]) -> Result<()> {
        let query = "INSERT INTO waypoints (drone_id, sequence_number, waypoint_id, \
             waypoint_name, waypoint_type, coordinates, planned_arrival, \
             loiter_duration_min, status) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)";

        for wp in waypoints {
            let coordinates = CoordinatesUdt::from(wp.coordinates);
            self.client
                .session
                .query_unpaged(
                    query,
                    (
                        wp.drone_id,
                        wp.sequence_number,
                        wp.waypoint_id,
                        &wp.waypoint_name,
                        waypoint_type_str(&wp.waypoint_type),
                        &coordinates,
                        wp.planned_arrival.map(ts),
                        wp.loiter_duration_min,
                        waypoint_status_str(&wp.status),
                    ),
                )
                .await?;
        }
        Ok(())
    }
}

// =============================================================================
// ENUM <-> TEXT HELPERS
// =============================================================================
//
// Storage strings are the domain `as_str` forms. Parsers accept both the
// dashed storage form ("MQ-9_REAPER") and the GraphQL SCREAMING_SNAKE form
// ("MQ9_REAPER") so rows written by any client round-trip.

fn parse_platform_type(s: &str) -> PlatformType {
    match s {
        "MQ-1C_GRAY_EAGLE" | "MQ1C_GRAY_EAGLE" => PlatformType::Mq1cGrayEagle,
        "RQ-4_GLOBAL_HAWK" | "RQ4_GLOBAL_HAWK" => PlatformType::Rq4GlobalHawk,
        "MQ-25_STINGRAY" | "MQ25_STINGRAY" => PlatformType::Mq25Stingray,
        _ => PlatformType::Mq9Reaper,
    }
}

fn parse_weapon_type(s: &str) -> WeaponType {
    match s {
        "GBU-12_PAVEWAY" | "GBU12_PAVEWAY" => WeaponType::Gbu12Paveway,
        "AIM-9X_SIDEWINDER" | "AIM9X_SIDEWINDER" => WeaponType::Aim9xSidewinder,
        "GBU-38_JDAM" | "GBU38_JDAM" => WeaponType::Gbu38Jdam,
        "AGM-176_GRIFFIN" | "AGM176_GRIFFIN" => WeaponType::Agm176Griffin,
        _ => WeaponType::Agm114Hellfire,
    }
}

fn parse_target_type(s: &str) -> TargetType {
    match s {
        "STRUCTURE" => TargetType::Structure,
        "PERSONNEL" => TargetType::Personnel,
        "RADAR" => TargetType::Radar,
        "AIR_DEFENSE" => TargetType::AirDefense,
        "SUPPLY" => TargetType::Supply,
        _ => TargetType::Vehicle,
    }
}

fn parse_threat_level(s: &str) -> ThreatLevel {
    match s {
        "HIGH" => ThreatLevel::High,
        "MEDIUM" => ThreatLevel::Medium,
        _ => ThreatLevel::Low,
    }
}

fn parse_drone_status(s: &str) -> DroneStatus {
    match s {
        "PREFLIGHT" => DroneStatus::Preflight,
        "LOITER" => DroneStatus::Loiter,
        "INGRESS" => DroneStatus::Ingress,
        "EGRESS" => DroneStatus::Egress,
        "RTB" => DroneStatus::Rtb,
        "LANDED" => DroneStatus::Landed,
        "MAINTENANCE" => DroneStatus::Maintenance,
        _ => DroneStatus::Airborne,
    }
}

fn drone_status_str(s: &DroneStatus) -> &'static str {
    match s {
        DroneStatus::Preflight => "PREFLIGHT",
        DroneStatus::Airborne => "AIRBORNE",
        DroneStatus::Loiter => "LOITER",
        DroneStatus::Ingress => "INGRESS",
        DroneStatus::Egress => "EGRESS",
        DroneStatus::Rtb => "RTB",
        DroneStatus::Landed => "LANDED",
        DroneStatus::Maintenance => "MAINTENANCE",
    }
}

fn parse_mission_type(s: &str) -> MissionType {
    match s {
        "ISR" => MissionType::Isr,
        "ESCORT" => MissionType::Escort,
        "RESUPPLY" => MissionType::Resupply,
        "SAR" => MissionType::Sar,
        _ => MissionType::Strike,
    }
}

fn parse_convoy_status(s: &str) -> ConvoyStatus {
    match s {
        "PLANNING" => ConvoyStatus::Planning,
        "RTB" => ConvoyStatus::Rtb,
        "COMPLETE" => ConvoyStatus::Complete,
        "ABORT" => ConvoyStatus::Abort,
        _ => ConvoyStatus::Active,
    }
}

fn parse_waypoint_type(s: &str) -> WaypointType {
    match s {
        "LOITER" => WaypointType::Loiter,
        "STRIKE" => WaypointType::Strike,
        "REFUEL" => WaypointType::Refuel,
        "RENDEZVOUS" => WaypointType::Rendezvous,
        "CHECKPOINT" => WaypointType::Checkpoint,
        _ => WaypointType::Nav,
    }
}

fn waypoint_type_str(t: &WaypointType) -> &'static str {
    match t {
        WaypointType::Nav => "NAV",
        WaypointType::Loiter => "LOITER",
        WaypointType::Strike => "STRIKE",
        WaypointType::Refuel => "REFUEL",
        WaypointType::Rendezvous => "RENDEZVOUS",
        WaypointType::Checkpoint => "CHECKPOINT",
    }
}

fn parse_waypoint_status(s: &str) -> WaypointStatus {
    match s {
        "ACTIVE" => WaypointStatus::Active,
        "COMPLETE" => WaypointStatus::Complete,
        "SKIPPED" => WaypointStatus::Skipped,
        _ => WaypointStatus::Pending,
    }
}

fn waypoint_status_str(s: &WaypointStatus) -> &'static str {
    match s {
        WaypointStatus::Pending => "PENDING",
        WaypointStatus::Active => "ACTIVE",
        WaypointStatus::Complete => "COMPLETE",
        WaypointStatus::Skipped => "SKIPPED",
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

fn threat_level_str(t: &ThreatLevel) -> &'static str {
    match t {
        ThreatLevel::High => "HIGH",
        ThreatLevel::Medium => "MEDIUM",
        ThreatLevel::Low => "LOW",
        ThreatLevel::Unknown => "UNKNOWN",
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
