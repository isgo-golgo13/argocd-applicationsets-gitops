//! Analytics engine using DuckDB for OLAP queries.

use crate::error::{AnalyticsError, Result};
use chrono::{DateTime, Utc};
use duckdb::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

/// DuckDB-based analytics engine for historical drone data analysis.
pub struct AnalyticsEngine {
    pub(crate) conn: Connection,
}

impl AnalyticsEngine {
    /// Create a new in-memory analytics engine.
    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let engine = Self { conn };
        engine.initialize_schema()?;
        Ok(engine)
    }

    /// Create analytics engine with persistent storage.
    pub fn new_persistent<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        let engine = Self { conn };
        engine.initialize_schema()?;
        Ok(engine)
    }

    /// Initialize the analytics schema.
    fn initialize_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            -- Engagements fact table
            CREATE TABLE IF NOT EXISTS engagements (
                engagement_id VARCHAR PRIMARY KEY,
                convoy_id VARCHAR NOT NULL,
                drone_id VARCHAR NOT NULL,
                callsign VARCHAR NOT NULL,
                platform_type VARCHAR NOT NULL,
                hit BOOLEAN NOT NULL,
                weapon_type VARCHAR NOT NULL,
                target_type VARCHAR,
                range_km DOUBLE,
                altitude_m DOUBLE,
                timestamp TIMESTAMP NOT NULL
            );

            -- Drone performance dimension
            CREATE TABLE IF NOT EXISTS drone_performance (
                drone_id VARCHAR PRIMARY KEY,
                callsign VARCHAR NOT NULL,
                platform_type VARCHAR NOT NULL,
                total_engagements INTEGER DEFAULT 0,
                total_hits INTEGER DEFAULT 0,
                accuracy_pct DOUBLE DEFAULT 0.0,
                best_streak INTEGER DEFAULT 0,
                total_flight_hours DOUBLE DEFAULT 0.0,
                first_engagement TIMESTAMP,
                last_engagement TIMESTAMP
            );

            -- Mission summaries
            CREATE TABLE IF NOT EXISTS mission_summaries (
                convoy_id VARCHAR PRIMARY KEY,
                callsign VARCHAR NOT NULL,
                mission_type VARCHAR NOT NULL,
                start_time TIMESTAMP NOT NULL,
                end_time TIMESTAMP,
                drone_count INTEGER NOT NULL,
                total_engagements INTEGER DEFAULT 0,
                total_hits INTEGER DEFAULT 0,
                avg_accuracy_pct DOUBLE DEFAULT 0.0
            );

            -- Create indexes for common queries
            CREATE INDEX IF NOT EXISTS idx_engagements_convoy ON engagements(convoy_id);
            CREATE INDEX IF NOT EXISTS idx_engagements_drone ON engagements(drone_id);
            CREATE INDEX IF NOT EXISTS idx_engagements_timestamp ON engagements(timestamp);
            CREATE INDEX IF NOT EXISTS idx_engagements_weapon ON engagements(weapon_type);
            "#,
        )?;
        Ok(())
    }

    /// Ingest an engagement record.
    pub fn ingest_engagement(&self, engagement: &EngagementRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO engagements (
                engagement_id, convoy_id, drone_id, callsign, platform_type,
                hit, weapon_type, target_type, range_km, altitude_m, timestamp
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT (engagement_id) DO NOTHING
            "#,
            params![
                engagement.engagement_id.to_string(),
                engagement.convoy_id.to_string(),
                engagement.drone_id.to_string(),
                engagement.callsign,
                engagement.platform_type,
                engagement.hit,
                engagement.weapon_type,
                engagement.target_type,
                engagement.range_km,
                engagement.altitude_m,
                engagement.timestamp.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Batch ingest engagements.
    pub fn ingest_engagements_batch(&self, engagements: &[EngagementRecord]) -> Result<usize> {
        let mut count = 0;
        for engagement in engagements {
            self.ingest_engagement(engagement)?;
            count += 1;
        }
        Ok(count)
    }

    /// Get accuracy trend over time for a drone.
    pub fn accuracy_trend(
        &self,
        drone_id: Uuid,
        interval: &str,
    ) -> Result<Vec<AccuracyDataPoint>> {
        let query = format!(
            r#"
            SELECT 
                date_trunc('{}', timestamp) as period,
                COUNT(*) as total,
                SUM(CASE WHEN hit THEN 1 ELSE 0 END) as hits,
                ROUND(100.0 * SUM(CASE WHEN hit THEN 1 ELSE 0 END) / COUNT(*), 2) as accuracy
            FROM engagements
            WHERE drone_id = ?
            GROUP BY period
            ORDER BY period
            "#,
            interval
        );

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map(params![drone_id.to_string()], |row| {
            Ok(AccuracyDataPoint {
                period: row.get(0)?,
                total_engagements: row.get(1)?,
                hits: row.get(2)?,
                accuracy_pct: row.get(3)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(AnalyticsError::from)
    }

    /// Get weapon effectiveness analysis.
    pub fn weapon_effectiveness(&self, convoy_id: Option<Uuid>) -> Result<Vec<WeaponStats>> {
        let results = match convoy_id {
            Some(id) => {
                let mut stmt = self.conn.prepare(
                    r#"
                    SELECT 
                        weapon_type,
                        COUNT(*) as total,
                        SUM(CASE WHEN hit THEN 1 ELSE 0 END) as hits,
                        ROUND(100.0 * SUM(CASE WHEN hit THEN 1 ELSE 0 END) / COUNT(*), 2) as accuracy,
                        ROUND(AVG(range_km), 2) as avg_range
                    FROM engagements
                    WHERE convoy_id = ?
                    GROUP BY weapon_type
                    ORDER BY accuracy DESC
                    "#,
                )?;
                let rows = stmt.query_map(params![id.to_string()], |row: &duckdb::Row| {
                    Ok(WeaponStats {
                        weapon_type: row.get(0)?,
                        total_engagements: row.get(1)?,
                        hits: row.get(2)?,
                        accuracy_pct: row.get(3)?,
                        avg_range_km: row.get(4)?,
                    })
                })?;
                rows.collect::<std::result::Result<Vec<_>, _>>()?
            }
            None => {
                let mut stmt = self.conn.prepare(
                    r#"
                    SELECT 
                        weapon_type,
                        COUNT(*) as total,
                        SUM(CASE WHEN hit THEN 1 ELSE 0 END) as hits,
                        ROUND(100.0 * SUM(CASE WHEN hit THEN 1 ELSE 0 END) / COUNT(*), 2) as accuracy,
                        ROUND(AVG(range_km), 2) as avg_range
                    FROM engagements
                    GROUP BY weapon_type
                    ORDER BY accuracy DESC
                    "#,
                )?;
                let rows = stmt.query_map([], |row: &duckdb::Row| {
                    Ok(WeaponStats {
                        weapon_type: row.get(0)?,
                        total_engagements: row.get(1)?,
                        hits: row.get(2)?,
                        accuracy_pct: row.get(3)?,
                        avg_range_km: row.get(4)?,
                    })
                })?;
                rows.collect::<std::result::Result<Vec<_>, _>>()?
            }
        };

        Ok(results)
    }

    /// Get top performers by accuracy.
    pub fn top_performers(&self, limit: usize) -> Result<Vec<DronePerformance>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT 
                drone_id,
                callsign,
                platform_type,
                COUNT(*) as total,
                SUM(CASE WHEN hit THEN 1 ELSE 0 END) as hits,
                ROUND(100.0 * SUM(CASE WHEN hit THEN 1 ELSE 0 END) / COUNT(*), 2) as accuracy
            FROM engagements
            GROUP BY drone_id, callsign, platform_type
            HAVING COUNT(*) >= 5
            ORDER BY accuracy DESC
            LIMIT ?
            "#,
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(DronePerformance {
                drone_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                callsign: row.get(1)?,
                platform_type: row.get(2)?,
                total_engagements: row.get(3)?,
                hits: row.get(4)?,
                accuracy_pct: row.get(5)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(AnalyticsError::from)
    }

    /// Get engagement distribution by hour of day.
    pub fn hourly_distribution(&self) -> Result<Vec<HourlyStats>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT 
                EXTRACT(HOUR FROM timestamp) as hour,
                COUNT(*) as total,
                SUM(CASE WHEN hit THEN 1 ELSE 0 END) as hits,
                ROUND(100.0 * SUM(CASE WHEN hit THEN 1 ELSE 0 END) / COUNT(*), 2) as accuracy
            FROM engagements
            GROUP BY hour
            ORDER BY hour
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(HourlyStats {
                hour: row.get(0)?,
                total_engagements: row.get(1)?,
                hits: row.get(2)?,
                accuracy_pct: row.get(3)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(AnalyticsError::from)
    }

    /// Export data to Parquet file.
    pub fn export_to_parquet<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let query = format!(
            "COPY engagements TO '{}' (FORMAT PARQUET)",
            path.as_ref().display()
        );
        self.conn.execute(&query, [])?;
        Ok(())
    }

    /// Import data from Parquet file.
    pub fn import_from_parquet<P: AsRef<Path>>(&self, path: P) -> Result<usize> {
        let query = format!(
            "INSERT INTO engagements SELECT * FROM read_parquet('{}')",
            path.as_ref().display()
        );
        let count = self.conn.execute(&query, [])?;
        Ok(count)
    }
}

/// Engagement record for analytics ingestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngagementRecord {
    pub engagement_id: Uuid,
    pub convoy_id: Uuid,
    pub drone_id: Uuid,
    pub callsign: String,
    pub platform_type: String,
    pub hit: bool,
    pub weapon_type: String,
    pub target_type: Option<String>,
    pub range_km: Option<f64>,
    pub altitude_m: Option<f64>,
    pub timestamp: DateTime<Utc>,
}

/// Accuracy data point for trend analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccuracyDataPoint {
    pub period: String,
    pub total_engagements: i64,
    pub hits: i64,
    pub accuracy_pct: f64,
}

/// Weapon effectiveness statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponStats {
    pub weapon_type: String,
    pub total_engagements: i64,
    pub hits: i64,
    pub accuracy_pct: f64,
    pub avg_range_km: Option<f64>,
}

/// Drone performance summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DronePerformance {
    pub drone_id: Uuid,
    pub callsign: String,
    pub platform_type: String,
    pub total_engagements: i64,
    pub hits: i64,
    pub accuracy_pct: f64,
}

/// Hourly engagement statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HourlyStats {
    pub hour: i32,
    pub total_engagements: i64,
    pub hits: i64,
    pub accuracy_pct: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_engine() {
        let engine = AnalyticsEngine::new_in_memory().unwrap();
        assert!(engine.top_performers(10).unwrap().is_empty());
    }

    #[test]
    fn test_ingest_and_query() {
        let engine = AnalyticsEngine::new_in_memory().unwrap();

        let engagement = EngagementRecord {
            engagement_id: Uuid::new_v4(),
            convoy_id: Uuid::new_v4(),
            drone_id: Uuid::new_v4(),
            callsign: "REAPER-01".to_string(),
            platform_type: "MQ9_REAPER".to_string(),
            hit: true,
            weapon_type: "AGM114_HELLFIRE".to_string(),
            target_type: Some("VEHICLE".to_string()),
            range_km: Some(5.5),
            altitude_m: Some(5000.0),
            timestamp: Utc::now(),
        };

        engine.ingest_engagement(&engagement).unwrap();

        let weapons = engine.weapon_effectiveness(None).unwrap();
        assert_eq!(weapons.len(), 1);
        assert_eq!(weapons[0].weapon_type, "AGM114_HELLFIRE");
        assert_eq!(weapons[0].accuracy_pct, 100.0);
    }
}
