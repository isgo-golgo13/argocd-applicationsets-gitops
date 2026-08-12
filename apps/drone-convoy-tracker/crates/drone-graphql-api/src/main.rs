//! # Drone Convoy GraphQL API Server
//!
//! Binary entry point for the GraphQL API service.

use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use drone_graphql_api::{build_router, build_schema, ApiContext, Config};
use drone_persistence::{CacheClient, CacheConfig, ScyllaClient, ScyllaConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Load configuration
    let config = Config::from_env();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| config.log_level.clone().into()),
        )
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    tracing::info!(
        version = drone_graphql_api::VERSION,
        "Starting Drone Convoy GraphQL API"
    );

    // Initialize ScyllaDB client
    tracing::info!(
        hosts = ?config.scylla.hosts,
        keyspace = %config.scylla.keyspace,
        "Connecting to ScyllaDB"
    );

    let scylla_config = ScyllaConfig {
        hosts: config.scylla.hosts.clone(),
        keyspace: config.scylla.keyspace.clone(),
        username: config.scylla.username.clone(),
        password: config.scylla.password.clone(),
    };

    let scylla = ScyllaClient::new(scylla_config).await?;
    tracing::info!("ScyllaDB connected");

    // Initialize Redis cache
    tracing::info!(url = %config.redis.url, "Connecting to Redis");

    let cache_config = CacheConfig {
        url: config.redis.url.clone(),
        pool_size: config.redis.pool_size,
        ..Default::default()
    };

    let cache = CacheClient::new(cache_config).await?;
    tracing::info!("Redis connected");

    // Build API context
    let api_ctx = ApiContext::new(scylla, cache);

    // Build GraphQL schema
    let schema = build_schema(api_ctx);

    tracing::info!(
        playground = config.enable_playground,
        introspection = config.enable_introspection,
        max_depth = config.max_query_depth,
        max_complexity = config.max_query_complexity,
        "GraphQL schema built"
    );

    // Build router
    let app = build_router(schema);

    // Start server
    let addr = config.server_addr;
    tracing::info!(%addr, "Starting HTTP server");

    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!(
        "GraphQL Playground available at http://{}/graphql",
        addr
    );
    tracing::info!(
        "WebSocket subscriptions at ws://{}/graphql/ws",
        addr
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Server shut down gracefully");
    Ok(())
}

/// Graceful shutdown signal handler
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Received Ctrl+C, shutting down");
        }
        _ = terminate => {
            tracing::info!("Received SIGTERM, shutting down");
        }
    }
}
