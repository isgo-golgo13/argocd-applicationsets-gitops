//! # Cache Module
//!
//! Redis cache layer for hot-path data access.

pub mod redis_client;

pub use redis_client::{CacheClient, CacheConfig, CacheTtl, SharedCacheClient, shared_cache};
