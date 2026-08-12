//! Persistence layer error types

use thiserror::Error;

/// Persistence layer errors
#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("ScyllaDB error: {0}")]
    Scylla(String),

    #[error("Redis error: {0}")]
    Redis(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Entity not found: {entity_type} with key {key}")]
    NotFound { entity_type: String, key: String },

    #[error("Connection pool exhausted")]
    PoolExhausted,

    #[error("Query timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("Invalid query parameters: {0}")]
    InvalidQuery(String),

    #[error("Cache miss for key: {0}")]
    CacheMiss(String),

    #[error("Write conflict: {0}")]
    WriteConflict(String),
}

impl From<serde_json::Error> for PersistenceError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

#[cfg(feature = "scylla")]
impl From<scylla::transport::errors::NewSessionError> for PersistenceError {
    fn from(err: scylla::transport::errors::NewSessionError) -> Self {
        Self::Scylla(err.to_string())
    }
}

#[cfg(feature = "scylla")]
impl From<scylla::transport::errors::QueryError> for PersistenceError {
    fn from(err: scylla::transport::errors::QueryError) -> Self {
        Self::Scylla(err.to_string())
    }
}

#[cfg(feature = "redis")]
impl From<redis::RedisError> for PersistenceError {
    fn from(err: redis::RedisError) -> Self {
        Self::Redis(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, PersistenceError>;
