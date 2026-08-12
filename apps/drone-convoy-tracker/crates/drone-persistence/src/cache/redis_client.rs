//! # Redis Cache Layer
//!
//! Redis client wrapper with typed operations for drone convoy caching.

use redis::aio::ConnectionManager;
use redis::{AsyncCommands, Client};
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use crate::error::Result;

/// Cache TTL configuration
#[derive(Debug, Clone, Copy)]
pub struct CacheTtl {
    pub telemetry: Duration,
    pub drone_state: Duration,
    pub leaderboard: Duration,
    pub convoy_summary: Duration,
    pub engagement_stats: Duration,
    pub convoy_roster: Duration,
}

impl Default for CacheTtl {
    fn default() -> Self {
        Self {
            telemetry: Duration::from_secs(10),
            drone_state: Duration::from_secs(60),
            leaderboard: Duration::from_secs(300),
            convoy_summary: Duration::from_secs(120),
            engagement_stats: Duration::from_secs(300),
            convoy_roster: Duration::from_secs(3600),
        }
    }
}

/// Redis cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub url: String,
    pub pool_size: usize,
    pub ttl: CacheTtl,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            url: "redis://127.0.0.1:6379".to_string(),
            pool_size: 10,
            ttl: CacheTtl::default(),
        }
    }
}

/// Redis cache client with connection pooling
#[derive(Clone)]
pub struct CacheClient {
    conn: ConnectionManager,
    config: CacheConfig,
}

impl CacheClient {
    /// Create a new cache client
    pub async fn new(config: CacheConfig) -> Result<Self> {
        let client = Client::open(config.url.as_str())?;
        let conn = ConnectionManager::new(client).await?;

        Ok(Self { conn, config })
    }

    /// Get raw connection for advanced operations
    pub fn connection(&self) -> ConnectionManager {
        self.conn.clone()
    }

    // =========================================================================
    // GENERIC OPERATIONS
    // =========================================================================

    /// Get a JSON value from cache
    pub async fn get_json<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let mut conn = self.conn.clone();
        let value: Option<String> = conn.get(key).await?;

        match value {
            Some(json) => {
                let parsed = serde_json::from_str(&json)?;
                Ok(Some(parsed))
            }
            None => Ok(None),
        }
    }

    /// Set a JSON value in cache with TTL
    pub async fn set_json<T: Serialize>(&self, key: &str, value: &T, ttl: Duration) -> Result<()> {
        let mut conn = self.conn.clone();
        let json = serde_json::to_string(value)?;
        let _: () = conn.set_ex(key, json, ttl.as_secs()).await?;
        Ok(())
    }

    /// Delete a key from cache
    pub async fn delete(&self, key: &str) -> Result<bool> {
        let mut conn = self.conn.clone();
        let deleted: i64 = conn.del(key).await?;
        Ok(deleted > 0)
    }

    /// Delete multiple keys
    pub async fn delete_many(&self, keys: &[String]) -> Result<i64> {
        if keys.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn.clone();
        let deleted: i64 = conn.del(keys).await?;
        Ok(deleted)
    }

    /// Check if key exists
    pub async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn = self.conn.clone();
        let exists: bool = conn.exists(key).await?;
        Ok(exists)
    }

    // =========================================================================
    // LEADERBOARD OPERATIONS (SORTED SET)
    // =========================================================================

    /// Get convoy leaderboard (top N by accuracy)
    pub async fn get_leaderboard(
        &self,
        convoy_id: Uuid,
        limit: usize,
    ) -> Result<Vec<(Uuid, f64)>> {
        let key = format!("convoy:leaderboard:{convoy_id}");
        let mut conn = self.conn.clone();

        // ZREVRANGE with scores (highest accuracy first)
        let results: Vec<(String, f64)> = conn
            .zrevrange_withscores(&key, 0, (limit - 1) as isize)
            .await?;

        let parsed: Vec<(Uuid, f64)> = results
            .into_iter()
            .filter_map(|(id_str, score)| {
                Uuid::parse_str(&id_str).ok().map(|id| (id, score))
            })
            .collect();

        Ok(parsed)
    }

    /// Update drone accuracy in leaderboard
    pub async fn update_leaderboard_score(
        &self,
        convoy_id: Uuid,
        drone_id: Uuid,
        accuracy_pct: f64,
    ) -> Result<()> {
        let key = format!("convoy:leaderboard:{convoy_id}");
        let mut conn = self.conn.clone();

        let _: () = conn.zadd(&key, drone_id.to_string(), accuracy_pct).await?;
        let _: () = conn.expire(&key, self.config.ttl.leaderboard.as_secs() as i64)
            .await?;

        Ok(())
    }

    /// Get drone rank in leaderboard (0-indexed, None if not present)
    pub async fn get_drone_rank(&self, convoy_id: Uuid, drone_id: Uuid) -> Result<Option<i64>> {
        let key = format!("convoy:leaderboard:{convoy_id}");
        let mut conn = self.conn.clone();

        let rank: Option<i64> = conn.zrevrank(&key, drone_id.to_string()).await?;
        Ok(rank)
    }

    /// Remove drone from leaderboard
    pub async fn remove_from_leaderboard(&self, convoy_id: Uuid, drone_id: Uuid) -> Result<bool> {
        let key = format!("convoy:leaderboard:{convoy_id}");
        let mut conn = self.conn.clone();

        let removed: i64 = conn.zrem(&key, drone_id.to_string()).await?;
        Ok(removed > 0)
    }

    // =========================================================================
    // DRONE STATE OPERATIONS (HASH)
    // =========================================================================

    /// Get drone state hash
    pub async fn get_drone_state(&self, drone_id: Uuid) -> Result<Option<std::collections::HashMap<String, String>>> {
        let key = format!("drone:state:{drone_id}");
        let mut conn = self.conn.clone();

        let state: std::collections::HashMap<String, String> = conn.hgetall(&key).await?;
        
        if state.is_empty() {
            Ok(None)
        } else {
            Ok(Some(state))
        }
    }

    /// Set drone state hash
    pub async fn set_drone_state(
        &self,
        drone_id: Uuid,
        fields: &[(&str, String)],
    ) -> Result<()> {
        let key = format!("drone:state:{drone_id}");
        let mut conn = self.conn.clone();

        for (field, value) in fields {
            conn.hset::<_, _, _, ()>(&key, *field, value).await?;
        }
        let _: () = conn.expire(&key, self.config.ttl.drone_state.as_secs() as i64)
            .await?;

        Ok(())
    }

    /// Increment engagement counter for drone
    pub async fn increment_engagements(
        &self,
        drone_id: Uuid,
        hit: bool,
    ) -> Result<(i64, i64)> {
        let key = format!("stats:engagements:{drone_id}");
        let mut conn = self.conn.clone();

        let total: i64 = conn.hincr(&key, "total_engagements", 1i64).await?;
        let hits: i64 = if hit {
            conn.hincr(&key, "successful_hits", 1i64).await?
        } else {
            conn.hget(&key, "successful_hits").await.unwrap_or(0)
        };

        let _: () = conn.expire(&key, self.config.ttl.engagement_stats.as_secs() as i64)
            .await?;

        Ok((total, hits))
    }

    // =========================================================================
    // CONVOY ROSTER OPERATIONS (SET)
    // =========================================================================

    /// Get all drone IDs in convoy
    pub async fn get_convoy_roster(&self, convoy_id: Uuid) -> Result<Vec<Uuid>> {
        let key = format!("convoy:roster:{convoy_id}");
        let mut conn = self.conn.clone();

        let members: Vec<String> = conn.smembers(&key).await?;
        
        let parsed: Vec<Uuid> = members
            .into_iter()
            .filter_map(|s| Uuid::parse_str(&s).ok())
            .collect();

        Ok(parsed)
    }

    /// Add drone to convoy roster
    pub async fn add_to_convoy_roster(&self, convoy_id: Uuid, drone_id: Uuid) -> Result<bool> {
        let key = format!("convoy:roster:{convoy_id}");
        let mut conn = self.conn.clone();

        let added: i64 = conn.sadd(&key, drone_id.to_string()).await?;
        let _: () = conn.expire(&key, self.config.ttl.convoy_roster.as_secs() as i64)
            .await?;

        Ok(added > 0)
    }

    /// Remove drone from convoy roster
    pub async fn remove_from_convoy_roster(
        &self,
        convoy_id: Uuid,
        drone_id: Uuid,
    ) -> Result<bool> {
        let key = format!("convoy:roster:{convoy_id}");
        let mut conn = self.conn.clone();

        let removed: i64 = conn.srem(&key, drone_id.to_string()).await?;
        Ok(removed > 0)
    }

    // =========================================================================
    // TELEMETRY OPERATIONS
    // =========================================================================

    /// Set latest telemetry for drone
    pub async fn set_latest_telemetry<T: Serialize>(
        &self,
        drone_id: Uuid,
        telemetry: &T,
    ) -> Result<()> {
        let key = format!("telemetry:latest:{drone_id}");
        self.set_json(&key, telemetry, self.config.ttl.telemetry)
            .await
    }

    /// Get latest telemetry for drone
    pub async fn get_latest_telemetry<T: DeserializeOwned>(
        &self,
        drone_id: Uuid,
    ) -> Result<Option<T>> {
        let key = format!("telemetry:latest:{drone_id}");
        self.get_json(&key).await
    }

    // =========================================================================
    // CACHE INVALIDATION
    // =========================================================================

    /// Invalidate all cache keys for a drone
    pub async fn invalidate_drone(&self, drone_id: Uuid) -> Result<()> {
        let keys = vec![
            format!("drone:state:{drone_id}"),
            format!("telemetry:latest:{drone_id}"),
            format!("stats:engagements:{drone_id}"),
            format!("waypoints:progress:{drone_id}"),
        ];

        self.delete_many(&keys).await?;
        Ok(())
    }

    /// Invalidate all cache keys for a convoy
    pub async fn invalidate_convoy(&self, convoy_id: Uuid) -> Result<()> {
        let keys = vec![
            format!("convoy:leaderboard:{convoy_id}"),
            format!("convoy:roster:{convoy_id}"),
            format!("convoy:summary:{convoy_id}"),
            format!("mesh:topology:{convoy_id}"),
        ];

        self.delete_many(&keys).await?;
        Ok(())
    }
}

/// Shared cache client wrapper
pub type SharedCacheClient = Arc<CacheClient>;

/// Create a shared cache client
pub fn shared_cache(client: CacheClient) -> SharedCacheClient {
    Arc::new(client)
}
