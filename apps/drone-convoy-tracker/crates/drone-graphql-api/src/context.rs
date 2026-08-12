//! # API Context
//!
//! Application state and dependency injection for GraphQL resolvers.

use std::sync::Arc;
use tokio::sync::broadcast;

use crate::schema::*;
use drone_persistence::{
    CacheClient, ScyllaClient, ScyllaLeaderboardRepository, SharedCacheClient,
};

/// Broadcast channel capacity
const CHANNEL_CAPACITY: usize = 1024;

/// Application context shared across all GraphQL resolvers
#[derive(Clone)]
pub struct ApiContext {
    /// Leaderboard repository
    pub leaderboard_repo: Arc<ScyllaLeaderboardRepository>,

    /// ScyllaDB client
    pub scylla: Arc<ScyllaClient>,

    /// Redis cache client
    pub cache: SharedCacheClient,

    /// Engagement event broadcaster
    pub engagement_tx: broadcast::Sender<EngagementEvent>,

    /// Leaderboard update broadcaster
    pub leaderboard_tx: broadcast::Sender<LeaderboardUpdateEvent>,

    /// Drone status change broadcaster
    pub drone_status_tx: broadcast::Sender<DroneStatusEvent>,

    /// Alert broadcaster
    pub alert_tx: broadcast::Sender<AlertEvent>,

    /// Telemetry broadcaster
    pub telemetry_tx: broadcast::Sender<TelemetrySnapshot>,
}

impl ApiContext {
    /// Create a new API context with real dependencies
    pub fn new(scylla: ScyllaClient, cache: CacheClient) -> Self {
        let scylla = Arc::new(scylla);
        let cache = Arc::new(cache);

        // Create leaderboard repository with cache
        let leaderboard_repo = Arc::new(ScyllaLeaderboardRepository::new(
            scylla.clone(),
            Some(cache.clone()),
        ));

        // Create broadcast channels
        let (engagement_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (leaderboard_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (drone_status_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (alert_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (telemetry_tx, _) = broadcast::channel(CHANNEL_CAPACITY);

        Self {
            leaderboard_repo,
            scylla,
            cache,
            engagement_tx,
            leaderboard_tx,
            drone_status_tx,
            alert_tx,
            telemetry_tx,
        }
    }

    /// Create a mock context for testing
    #[cfg(test)]
    pub fn mock() -> Self {
        // For testing without real DB connections
        let (engagement_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (leaderboard_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (drone_status_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (alert_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (telemetry_tx, _) = broadcast::channel(CHANNEL_CAPACITY);

        // Would need mock implementations of repos
        unimplemented!("Mock context not yet implemented")
    }
}

/// Builder for ApiContext
pub struct ApiContextBuilder {
    scylla: Option<ScyllaClient>,
    cache: Option<CacheClient>,
}

impl ApiContextBuilder {
    pub fn new() -> Self {
        Self {
            scylla: None,
            cache: None,
        }
    }

    pub fn with_scylla(mut self, scylla: ScyllaClient) -> Self {
        self.scylla = Some(scylla);
        self
    }

    pub fn with_cache(mut self, cache: CacheClient) -> Self {
        self.cache = Some(cache);
        self
    }

    pub fn build(self) -> Result<ApiContext, &'static str> {
        let scylla = self.scylla.ok_or("ScyllaDB client required")?;
        let cache = self.cache.ok_or("Redis cache client required")?;
        Ok(ApiContext::new(scylla, cache))
    }
}

impl Default for ApiContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}
