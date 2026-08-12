//! Predefined analytical queries.

use crate::engine::AnalyticsEngine;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Mission summary statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionSummary {
    pub convoy_id: Uuid,
    pub total_drones: i64,
    pub total_engagements: i64,
    pub total_hits: i64,
    pub accuracy_pct: f64,
    pub top_performer: Option<String>,
    pub most_used_weapon: Option<String>,
}

/// Platform comparison statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformComparison {
    pub platform_type: String,
    pub drone_count: i64,
    pub total_engagements: i64,
    pub accuracy_pct: f64,
    pub avg_engagements_per_drone: f64,
}

impl AnalyticsEngine {
    /// Get comprehensive mission summary.
    pub fn mission_summary(&self, convoy_id: Uuid) -> Result<Option<MissionSummary>> {
        let mut stmt = self.conn.prepare(
            r#"
            WITH mission_stats AS (
                SELECT 
                    convoy_id,
                    COUNT(DISTINCT drone_id) as total_drones,
                    COUNT(*) as total_engagements,
                    SUM(CASE WHEN hit THEN 1 ELSE 0 END) as total_hits
                FROM engagements
                WHERE convoy_id = ?
                GROUP BY convoy_id
            ),
            top_drone AS (
                SELECT callsign
                FROM engagements
                WHERE convoy_id = ?
                GROUP BY callsign
                ORDER BY SUM(CASE WHEN hit THEN 1 ELSE 0 END)::FLOAT / COUNT(*) DESC
                LIMIT 1
            ),
            top_weapon AS (
                SELECT weapon_type
                FROM engagements
                WHERE convoy_id = ?
                GROUP BY weapon_type
                ORDER BY COUNT(*) DESC
                LIMIT 1
            )
            SELECT 
                m.total_drones,
                m.total_engagements,
                m.total_hits,
                ROUND(100.0 * m.total_hits / NULLIF(m.total_engagements, 0), 2) as accuracy,
                d.callsign as top_performer,
                w.weapon_type as most_used_weapon
            FROM mission_stats m
            LEFT JOIN top_drone d ON 1=1
            LEFT JOIN top_weapon w ON 1=1
            "#,
        )?;

        let convoy_str = convoy_id.to_string();
        let mut rows = stmt.query(duckdb::params![&convoy_str, &convoy_str, &convoy_str])?;

        if let Some(row) = rows.next()? {
            Ok(Some(MissionSummary {
                convoy_id,
                total_drones: row.get(0)?,
                total_engagements: row.get(1)?,
                total_hits: row.get(2)?,
                accuracy_pct: row.get::<_, Option<f64>>(3)?.unwrap_or(0.0),
                top_performer: row.get(4)?,
                most_used_weapon: row.get(5)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Compare performance across platform types.
    pub fn platform_comparison(&self) -> Result<Vec<PlatformComparison>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT 
                platform_type,
                COUNT(DISTINCT drone_id) as drone_count,
                COUNT(*) as total_engagements,
                ROUND(100.0 * SUM(CASE WHEN hit THEN 1 ELSE 0 END) / COUNT(*), 2) as accuracy,
                ROUND(COUNT(*)::FLOAT / COUNT(DISTINCT drone_id), 2) as avg_per_drone
            FROM engagements
            GROUP BY platform_type
            ORDER BY accuracy DESC
            "#,
        )?;

        let rows = stmt.query_map([], |row: &duckdb::Row| {
            Ok(PlatformComparison {
                platform_type: row.get(0)?,
                drone_count: row.get(1)?,
                total_engagements: row.get(2)?,
                accuracy_pct: row.get(3)?,
                avg_engagements_per_drone: row.get(4)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(crate::error::AnalyticsError::from)
    }

    /// Get engagement count by date range.
    pub fn engagement_counts_by_date(
        &self,
        start: &str,
        end: &str,
    ) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT 
                DATE(timestamp) as date,
                COUNT(*) as count
            FROM engagements
            WHERE timestamp >= ? AND timestamp <= ?
            GROUP BY date
            ORDER BY date
            "#,
        )?;

        let rows = stmt.query_map(duckdb::params![start, end], |row: &duckdb::Row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(crate::error::AnalyticsError::from)
    }

    /// Get accuracy by altitude bands.
    pub fn accuracy_by_altitude(&self) -> Result<Vec<(String, f64)>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT 
                CASE 
                    WHEN altitude_m < 3000 THEN 'Low (<3km)'
                    WHEN altitude_m < 5000 THEN 'Medium (3-5km)'
                    WHEN altitude_m < 7000 THEN 'High (5-7km)'
                    ELSE 'Very High (>7km)'
                END as altitude_band,
                ROUND(100.0 * SUM(CASE WHEN hit THEN 1 ELSE 0 END) / COUNT(*), 2) as accuracy
            FROM engagements
            WHERE altitude_m IS NOT NULL
            GROUP BY altitude_band
            ORDER BY MIN(altitude_m)
            "#,
        )?;

        let rows = stmt.query_map([], |row: &duckdb::Row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(crate::error::AnalyticsError::from)
    }

    /// Get accuracy by range bands.
    pub fn accuracy_by_range(&self) -> Result<Vec<(String, f64)>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT 
                CASE 
                    WHEN range_km < 2 THEN 'Close (<2km)'
                    WHEN range_km < 5 THEN 'Medium (2-5km)'
                    WHEN range_km < 10 THEN 'Long (5-10km)'
                    ELSE 'Extended (>10km)'
                END as range_band,
                ROUND(100.0 * SUM(CASE WHEN hit THEN 1 ELSE 0 END) / COUNT(*), 2) as accuracy
            FROM engagements
            WHERE range_km IS NOT NULL
            GROUP BY range_band
            ORDER BY MIN(range_km)
            "#,
        )?;

        let rows = stmt.query_map([], |row: &duckdb::Row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(crate::error::AnalyticsError::from)
    }
}
