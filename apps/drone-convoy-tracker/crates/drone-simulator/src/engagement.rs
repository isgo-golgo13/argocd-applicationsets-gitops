//! Engagement simulation for drone combat scenarios.

use chrono::{DateTime, Utc};
use rand::Rng;
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Weapon types available for engagement.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum WeaponType {
    Agm114Hellfire,
    Gbu12Paveway,
    Aim9xSidewinder,
    Gbu38Jdam,
    Agm176Griffin,
}

impl WeaponType {
    /// Get base accuracy for this weapon type.
    pub fn base_accuracy(&self) -> f64 {
        match self {
            Self::Agm114Hellfire => 0.92,
            Self::Gbu12Paveway => 0.88,
            Self::Aim9xSidewinder => 0.85,
            Self::Gbu38Jdam => 0.90,
            Self::Agm176Griffin => 0.87,
        }
    }

    /// Get typical engagement range in km.
    pub fn typical_range_km(&self) -> f64 {
        match self {
            Self::Agm114Hellfire => 8.0,
            Self::Gbu12Paveway => 12.0,
            Self::Aim9xSidewinder => 5.0,
            Self::Gbu38Jdam => 15.0,
            Self::Agm176Griffin => 6.0,
        }
    }

    /// Convert to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Agm114Hellfire => "AGM114_HELLFIRE",
            Self::Gbu12Paveway => "GBU12_PAVEWAY",
            Self::Aim9xSidewinder => "AIM9X_SIDEWINDER",
            Self::Gbu38Jdam => "GBU38_JDAM",
            Self::Agm176Griffin => "AGM176_GRIFFIN",
        }
    }

    /// Get random weapon type.
    pub fn random() -> Self {
        let mut rng = rand::thread_rng();
        match rng.gen_range(0..5) {
            0 => Self::Agm114Hellfire,
            1 => Self::Gbu12Paveway,
            2 => Self::Aim9xSidewinder,
            3 => Self::Gbu38Jdam,
            _ => Self::Agm176Griffin,
        }
    }
}

/// Target types for engagements.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TargetType {
    Vehicle,
    Personnel,
    Structure,
    Artillery,
    Radar,
    Aircraft,
}

impl TargetType {
    /// Get random target type.
    pub fn random() -> Self {
        let mut rng = rand::thread_rng();
        match rng.gen_range(0..6) {
            0 => Self::Vehicle,
            1 => Self::Personnel,
            2 => Self::Structure,
            3 => Self::Artillery,
            4 => Self::Radar,
            _ => Self::Aircraft,
        }
    }

    /// Convert to string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Vehicle => "VEHICLE",
            Self::Personnel => "PERSONNEL",
            Self::Structure => "STRUCTURE",
            Self::Artillery => "ARTILLERY",
            Self::Radar => "RADAR",
            Self::Aircraft => "AIRCRAFT",
        }
    }
}

/// Simulated engagement event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulatedEngagement {
    pub engagement_id: Uuid,
    pub convoy_id: Uuid,
    pub drone_id: Uuid,
    pub callsign: String,
    pub weapon_type: WeaponType,
    pub target_type: TargetType,
    pub range_km: f64,
    pub altitude_m: f64,
    pub hit: bool,
    pub timestamp: DateTime<Utc>,
}

/// Engagement simulator for generating realistic combat scenarios.
pub struct EngagementSimulator {
    /// Base accuracy modifier (skill level)
    skill_modifier: f64,
    /// Environmental modifier
    env_modifier: f64,
    rng: rand::rngs::ThreadRng,
    range_noise: Normal<f64>,
}

impl EngagementSimulator {
    /// Create a new engagement simulator.
    pub fn new() -> Self {
        Self {
            skill_modifier: 1.0,
            env_modifier: 1.0,
            rng: rand::thread_rng(),
            range_noise: Normal::new(0.0, 1.5).unwrap(),
        }
    }

    /// Create with custom skill modifier.
    pub fn with_skill(skill: f64) -> Self {
        Self {
            skill_modifier: skill.clamp(0.5, 1.5),
            ..Self::new()
        }
    }

    /// Set environmental conditions modifier.
    pub fn set_environment(&mut self, modifier: f64) {
        self.env_modifier = modifier.clamp(0.7, 1.0);
    }

    /// Simulate an engagement.
    pub fn simulate_engagement(
        &mut self,
        convoy_id: Uuid,
        drone_id: Uuid,
        callsign: &str,
        altitude_m: f64,
    ) -> SimulatedEngagement {
        let weapon = WeaponType::random();
        let target = TargetType::random();

        // Calculate range with noise
        let base_range = weapon.typical_range_km();
        let range = (base_range + self.range_noise.sample(&mut self.rng)).max(0.5);

        // Calculate hit probability
        let hit = self.calculate_hit(weapon, range, altitude_m);

        SimulatedEngagement {
            engagement_id: Uuid::new_v4(),
            convoy_id,
            drone_id,
            callsign: callsign.to_string(),
            weapon_type: weapon,
            target_type: target,
            range_km: range,
            altitude_m,
            hit,
            timestamp: Utc::now(),
        }
    }

    /// Calculate if engagement results in hit.
    fn calculate_hit(&mut self, weapon: WeaponType, range_km: f64, altitude_m: f64) -> bool {
        let base_acc = weapon.base_accuracy();
        let typical_range = weapon.typical_range_km();

        // Range penalty (accuracy drops at extreme ranges)
        let range_factor = if range_km <= typical_range {
            1.0
        } else {
            (typical_range / range_km).powf(0.5)
        };

        // Altitude factor (slightly worse at very high or low altitudes)
        let alt_factor = if altitude_m >= 3000.0 && altitude_m <= 6000.0 {
            1.0
        } else {
            0.95
        };

        // Final probability
        let hit_probability =
            base_acc * range_factor * alt_factor * self.skill_modifier * self.env_modifier;

        self.rng.gen_bool(hit_probability.clamp(0.1, 0.99))
    }

    /// Simulate multiple engagements.
    pub fn simulate_batch(
        &mut self,
        convoy_id: Uuid,
        drone_id: Uuid,
        callsign: &str,
        count: usize,
        altitude_m: f64,
    ) -> Vec<SimulatedEngagement> {
        (0..count)
            .map(|_| self.simulate_engagement(convoy_id, drone_id, callsign, altitude_m))
            .collect()
    }
}

impl Default for EngagementSimulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weapon_accuracy() {
        assert!(WeaponType::Agm114Hellfire.base_accuracy() > 0.9);
    }

    #[test]
    fn test_simulate_engagement() {
        let mut sim = EngagementSimulator::new();
        let engagement = sim.simulate_engagement(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "TEST-01",
            5000.0,
        );

        assert!(!engagement.callsign.is_empty());
        assert!(engagement.range_km > 0.0);
    }

    #[test]
    fn test_batch_simulation() {
        let mut sim = EngagementSimulator::new();
        let engagements = sim.simulate_batch(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "TEST-01",
            100,
            5000.0,
        );

        assert_eq!(engagements.len(), 100);

        // Check hit rate is reasonable (not 0% or 100%)
        let hits: usize = engagements.iter().filter(|e| e.hit).count();
        assert!(hits > 50 && hits < 100);
    }
}
