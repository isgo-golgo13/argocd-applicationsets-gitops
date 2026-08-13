//! # API Context
//!
//! Application state and dependency injection for GraphQL resolvers.

use std::sync::Arc;
use tokio::sync::broadcast;

use crate::schema::*;
use drone_persistence::{
    CacheClient, ScyllaClient, ScyllaConvoyRepository, ScyllaDroneRepository,
    ScyllaEngagementRepository, ScyllaLeaderboardRepository, ScyllaTelemetryRepository,
    ScyllaWaypointRepository, SharedCacheClient,
};

/// Broadcast channel capacity
const CHANNEL_CAPACITY: usize = 1024;

/// Application context shared across all GraphQL resolvers
#[derive(Clone)]
pub struct ApiContext {
    /// Leaderboard repository
    pub leaderboard_repo: Arc<ScyllaLeaderboardRepository>,

    /// Engagement repository (dual-write: engagements + engagements_by_drone)
    pub engagement_repo: Arc<ScyllaEngagementRepository>,

    /// Telemetry repository (hourly-bucketed time series)
    pub telemetry_repo: Arc<ScyllaTelemetryRepository>,

    /// Convoy repository
    pub convoy_repo: Arc<ScyllaConvoyRepository>,

    /// Drone state repository
    pub drone_repo: Arc<ScyllaDroneRepository>,

    /// Waypoint repository
    pub waypoint_repo: Arc<ScyllaWaypointRepository>,

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

        // Leaderboard keeps its Redis read-through cache; the other
        // repositories go straight to ScyllaDB.
        let leaderboard_repo = Arc::new(ScyllaLeaderboardRepository::new(
            scylla.clone(),
            Some(cache.clone()),
        ));
        let engagement_repo = Arc::new(ScyllaEngagementRepository::new(scylla.clone()));
        let telemetry_repo = Arc::new(ScyllaTelemetryRepository::new(scylla.clone()));
        let convoy_repo = Arc::new(ScyllaConvoyRepository::new(scylla.clone()));
        let drone_repo = Arc::new(ScyllaDroneRepository::new(scylla.clone()));
        let waypoint_repo = Arc::new(ScyllaWaypointRepository::new(scylla.clone()));

        // Create broadcast channels
        let (engagement_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (leaderboard_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (drone_status_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (alert_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (telemetry_tx, _) = broadcast::channel(CHANNEL_CAPACITY);

        Self {
            leaderboard_repo,
            engagement_repo,
            telemetry_repo,
            convoy_repo,
            drone_repo,
            waypoint_repo,
            scylla,
            cache,
            engagement_tx,
            leaderboard_tx,
            drone_status_tx,
            alert_tx,
            telemetry_tx,
        }
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
