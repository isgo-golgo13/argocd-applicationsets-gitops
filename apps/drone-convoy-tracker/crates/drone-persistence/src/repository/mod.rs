//! # Repository Module
//!
//! Repository pattern implementations for domain entity persistence.

pub mod scylla_impl;

pub use scylla_impl::{
    ScyllaClient, ScyllaConfig,
    ScyllaLeaderboardRepository, ScyllaEngagementRepository,
    ScyllaTelemetryRepository, ScyllaConvoyRepository,
    ScyllaWaypointRepository,
};
