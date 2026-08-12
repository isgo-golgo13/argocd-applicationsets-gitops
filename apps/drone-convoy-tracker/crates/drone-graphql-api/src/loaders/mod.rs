//! # DataLoaders
//!
//! Batch data loading for N+1 query prevention in GraphQL resolvers.

use async_graphql::dataloader::Loader;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::ApiError;
use crate::schema::{Drone, LeaderboardEntry};
use drone_persistence::ScyllaClient;

// =============================================================================
// DRONE LOADER
// =============================================================================

/// Batch loader for drones by ID
pub struct DroneLoader {
    scylla: Arc<ScyllaClient>,
}

impl DroneLoader {
    pub fn new(scylla: Arc<ScyllaClient>) -> Self {
        Self { scylla }
    }
}

impl Loader<(Uuid, Uuid)> for DroneLoader {
    type Value = Drone;
    type Error = Arc<ApiError>;

    async fn load(
        &self,
        keys: &[(Uuid, Uuid)], // (convoy_id, drone_id)
    ) -> Result<HashMap<(Uuid, Uuid), Self::Value>, Self::Error> {
        tracing::debug!(count = keys.len(), "Batch loading drones");

        // TODO: Implement batch query
        // For now, return empty map
        Ok(HashMap::new())
    }
}

// =============================================================================
// LEADERBOARD ENTRY LOADER
// =============================================================================

/// Batch loader for leaderboard entries
pub struct LeaderboardEntryLoader {
    scylla: Arc<ScyllaClient>,
}

impl LeaderboardEntryLoader {
    pub fn new(scylla: Arc<ScyllaClient>) -> Self {
        Self { scylla }
    }
}

impl Loader<(Uuid, Uuid)> for LeaderboardEntryLoader {
    type Value = LeaderboardEntry;
    type Error = Arc<ApiError>;

    async fn load(
        &self,
        keys: &[(Uuid, Uuid)], // (convoy_id, drone_id)
    ) -> Result<HashMap<(Uuid, Uuid), Self::Value>, Self::Error> {
        tracing::debug!(count = keys.len(), "Batch loading leaderboard entries");

        // TODO: Implement batch query
        Ok(HashMap::new())
    }
}

// =============================================================================
// CONVOY LOADER
// =============================================================================

/// Batch loader for convoys by ID
pub struct ConvoyLoader {
    scylla: Arc<ScyllaClient>,
}

impl ConvoyLoader {
    pub fn new(scylla: Arc<ScyllaClient>) -> Self {
        Self { scylla }
    }
}

impl Loader<Uuid> for ConvoyLoader {
    type Value = crate::schema::Convoy;
    type Error = Arc<ApiError>;

    async fn load(
        &self,
        keys: &[Uuid],
    ) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        tracing::debug!(count = keys.len(), "Batch loading convoys");

        // TODO: Implement batch query
        Ok(HashMap::new())
    }
}
