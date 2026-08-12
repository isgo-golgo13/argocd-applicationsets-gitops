//! # API Configuration
//!
//! Environment-based configuration for the GraphQL API service.

use std::env;
use std::net::SocketAddr;

/// API server configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// Server bind address
    pub server_addr: SocketAddr,

    /// Enable GraphQL Playground
    pub enable_playground: bool,

    /// Enable GraphQL introspection
    pub enable_introspection: bool,

    /// Maximum query depth
    pub max_query_depth: usize,

    /// Maximum query complexity
    pub max_query_complexity: usize,

    /// ScyllaDB configuration
    pub scylla: ScyllaConfig,

    /// Redis configuration
    pub redis: RedisConfig,

    /// Logging level
    pub log_level: String,

    /// CORS allowed origins
    pub cors_origins: Vec<String>,
}

/// ScyllaDB connection configuration
#[derive(Debug, Clone)]
pub struct ScyllaConfig {
    pub hosts: Vec<String>,
    pub keyspace: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

/// Redis connection configuration
#[derive(Debug, Clone)]
pub struct RedisConfig {
    pub url: String,
    pub pool_size: usize,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            // A malformed SERVER_ADDR falls back to the default and warns rather
            // than panicking: in a Pod, a panic here is CrashLoopBackOff with the
            // real cause buried in a stack trace.
            server_addr: env::var("SERVER_ADDR")
                .ok()
                .and_then(|raw| match raw.parse() {
                    Ok(addr) => Some(addr),
                    Err(err) => {
                        tracing::warn!(%raw, %err, "invalid SERVER_ADDR, using default");
                        None
                    }
                })
                .unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], 8080))),

            enable_playground: env::var("ENABLE_PLAYGROUND")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(true),

            enable_introspection: env::var("ENABLE_INTROSPECTION")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(true),

            max_query_depth: env::var("MAX_QUERY_DEPTH")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),

            max_query_complexity: env::var("MAX_QUERY_COMPLEXITY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000),

            scylla: ScyllaConfig {
                hosts: env::var("SCYLLA_HOSTS")
                    .unwrap_or_else(|_| "127.0.0.1:9042".to_string())
                    .split(',')
                    .map(String::from)
                    .collect(),
                keyspace: env::var("SCYLLA_KEYSPACE")
                    .unwrap_or_else(|_| "drone_ops".to_string()),
                username: env::var("SCYLLA_USERNAME").ok(),
                password: env::var("SCYLLA_PASSWORD").ok(),
            },

            redis: RedisConfig {
                url: env::var("REDIS_URL")
                    .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
                pool_size: env::var("REDIS_POOL_SIZE")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(10),
            },

            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),

            cors_origins: env::var("CORS_ORIGINS")
                .unwrap_or_else(|_| "*".to_string())
                .split(',')
                .map(String::from)
                .collect(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::from_env()
    }
}
