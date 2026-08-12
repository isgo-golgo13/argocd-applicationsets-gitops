//! # Drone Persistence Library
//!
//! Production-grade persistence layer for the Drone Convoy Tracking System.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     Application Layer                        │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                   Repository Layer                           │
//! │  (ScyllaLeaderboardRepository, ScyllaEngagementRepository)   │
//! └─────────────────────────────────────────────────────────────┘
//!                    │                   │
//!                    ▼                   ▼
//! ┌─────────────────────────┐   ┌──────────────────────────────┐
//! │     Redis Cache         │   │        ScyllaDB              │
//! │  (Leaderboard, State)   │   │   (Source of Truth)          │
//! └─────────────────────────┘   └──────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use drone_persistence::{
//!     cache::{CacheClient, CacheConfig},
//!     repository::{ScyllaClient, ScyllaConfig, ScyllaLeaderboardRepository},
//! };
//!
//! // Initialize clients
//! let scylla = Arc::new(ScyllaClient::new(ScyllaConfig::default()).await?);
//! let cache = Arc::new(CacheClient::new(CacheConfig::default()).await?);
//!
//! // Create repository with caching
//! let leaderboard_repo = ScyllaLeaderboardRepository::new(scylla, Some(cache));
//!
//! // Use repository
//! let leaderboard = leaderboard_repo.get_leaderboard(convoy_id, 10).await?;
//! ```

#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod cache;
pub mod error;
pub mod repository;
pub mod strategy;

// Re-export commonly used types
pub use cache::{CacheClient, CacheConfig, SharedCacheClient};
pub use error::{PersistenceError, Result};
pub use repository::{
    ScyllaClient, ScyllaConfig,
    ScyllaLeaderboardRepository, ScyllaEngagementRepository,
    ScyllaTelemetryRepository, ScyllaConvoyRepository,
    ScyllaWaypointRepository,
};
pub use strategy::{ReadStrategy, WriteStrategy};

/// Crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize the persistence layer with default configuration
///
/// # Errors
///
/// Returns an error if either ScyllaDB or Redis connection fails.
pub async fn init_default() -> Result<(ScyllaClient, CacheClient)> {
    let scylla = ScyllaClient::new(ScyllaConfig::default()).await?;
    let cache = CacheClient::new(CacheConfig::default()).await?;
    Ok((scylla, cache))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
