//! Report generation for analytics data.

use crate::engine::{AnalyticsEngine, DronePerformance, WeaponStats};
use crate::error::Result;
use crate::queries::{MissionSummary, PlatformComparison};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Comprehensive analytics report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsReport {
    pub generated_at: String,
    pub convoy_id: Option<Uuid>,
    pub mission_summary: Option<MissionSummary>,
    pub top_performers: Vec<DronePerformance>,
    pub weapon_stats: Vec<WeaponStats>,
    pub platform_comparison: Vec<PlatformComparison>,
    pub accuracy_by_altitude: Vec<(String, f64)>,
    pub accuracy_by_range: Vec<(String, f64)>,
}

impl AnalyticsEngine {
    /// Generate comprehensive analytics report.
    pub fn generate_report(&self, convoy_id: Option<Uuid>) -> Result<AnalyticsReport> {
        let mission_summary = convoy_id
            .map(|id| self.mission_summary(id))
            .transpose()?
            .flatten();

        let top_performers = self.top_performers(10)?;
        let weapon_stats = self.weapon_effectiveness(convoy_id)?;
        let platform_comparison = self.platform_comparison()?;
        let accuracy_by_altitude = self.accuracy_by_altitude()?;
        let accuracy_by_range = self.accuracy_by_range()?;

        Ok(AnalyticsReport {
            generated_at: chrono::Utc::now().to_rfc3339(),
            convoy_id,
            mission_summary,
            top_performers,
            weapon_stats,
            platform_comparison,
            accuracy_by_altitude,
            accuracy_by_range,
        })
    }

    /// Generate report as JSON string.
    pub fn generate_report_json(&self, convoy_id: Option<Uuid>) -> Result<String> {
        let report = self.generate_report(convoy_id)?;
        serde_json::to_string_pretty(&report)
            .map_err(|e| crate::error::AnalyticsError::Conversion(e.to_string()))
    }

    /// Generate Markdown report.
    pub fn generate_report_markdown(&self, convoy_id: Option<Uuid>) -> Result<String> {
        let report = self.generate_report(convoy_id)?;

        let mut md = String::new();
        md.push_str("# Drone Convoy Analytics Report\n\n");
        md.push_str(&format!("**Generated:** {}\n\n", report.generated_at));

        if let Some(ref summary) = report.mission_summary {
            md.push_str("## Mission Summary\n\n");
            md.push_str(&format!("| Metric | Value |\n"));
            md.push_str(&format!("|--------|-------|\n"));
            md.push_str(&format!("| Total Drones | {} |\n", summary.total_drones));
            md.push_str(&format!("| Total Engagements | {} |\n", summary.total_engagements));
            md.push_str(&format!("| Total Hits | {} |\n", summary.total_hits));
            md.push_str(&format!("| Accuracy | {:.1}% |\n", summary.accuracy_pct));
            if let Some(ref top) = summary.top_performer {
                md.push_str(&format!("| Top Performer | {} |\n", top));
            }
            if let Some(ref weapon) = summary.most_used_weapon {
                md.push_str(&format!("| Most Used Weapon | {} |\n", weapon));
            }
            md.push_str("\n");
        }

        if !report.top_performers.is_empty() {
            md.push_str("## Top Performers\n\n");
            md.push_str("| Rank | Callsign | Platform | Engagements | Hits | Accuracy |\n");
            md.push_str("|------|----------|----------|-------------|------|----------|\n");
            for (i, perf) in report.top_performers.iter().enumerate() {
                md.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {:.1}% |\n",
                    i + 1,
                    perf.callsign,
                    perf.platform_type,
                    perf.total_engagements,
                    perf.hits,
                    perf.accuracy_pct
                ));
            }
            md.push_str("\n");
        }

        if !report.weapon_stats.is_empty() {
            md.push_str("## Weapon Effectiveness\n\n");
            md.push_str("| Weapon | Engagements | Hits | Accuracy | Avg Range |\n");
            md.push_str("|--------|-------------|------|----------|----------|\n");
            for stat in &report.weapon_stats {
                let range_str = stat
                    .avg_range_km
                    .map(|r| format!("{:.1} km", r))
                    .unwrap_or_else(|| "N/A".to_string());
                md.push_str(&format!(
                    "| {} | {} | {} | {:.1}% | {} |\n",
                    stat.weapon_type,
                    stat.total_engagements,
                    stat.hits,
                    stat.accuracy_pct,
                    range_str
                ));
            }
            md.push_str("\n");
        }

        if !report.platform_comparison.is_empty() {
            md.push_str("## Platform Comparison\n\n");
            md.push_str("| Platform | Drones | Engagements | Accuracy | Avg/Drone |\n");
            md.push_str("|----------|--------|-------------|----------|----------|\n");
            for plat in &report.platform_comparison {
                md.push_str(&format!(
                    "| {} | {} | {} | {:.1}% | {:.1} |\n",
                    plat.platform_type,
                    plat.drone_count,
                    plat.total_engagements,
                    plat.accuracy_pct,
                    plat.avg_engagements_per_drone
                ));
            }
            md.push_str("\n");
        }

        if !report.accuracy_by_altitude.is_empty() {
            md.push_str("## Accuracy by Altitude\n\n");
            md.push_str("| Altitude Band | Accuracy |\n");
            md.push_str("|---------------|----------|\n");
            for (band, acc) in &report.accuracy_by_altitude {
                md.push_str(&format!("| {} | {:.1}% |\n", band, acc));
            }
            md.push_str("\n");
        }

        if !report.accuracy_by_range.is_empty() {
            md.push_str("## Accuracy by Range\n\n");
            md.push_str("| Range Band | Accuracy |\n");
            md.push_str("|------------|----------|\n");
            for (band, acc) in &report.accuracy_by_range {
                md.push_str(&format!("| {} | {:.1}% |\n", band, acc));
            }
            md.push_str("\n");
        }

        md.push_str("---\n");
        md.push_str("*Classification: UNCLASSIFIED // FOUO*\n");

        Ok(md)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_report() {
        let engine = AnalyticsEngine::new_in_memory().unwrap();
        let report = engine.generate_report(None).unwrap();
        assert!(report.top_performers.is_empty());
        assert!(report.weapon_stats.is_empty());
    }

    #[test]
    fn test_markdown_generation() {
        let engine = AnalyticsEngine::new_in_memory().unwrap();
        let md = engine.generate_report_markdown(None).unwrap();
        assert!(md.contains("# Drone Convoy Analytics Report"));
    }
}
