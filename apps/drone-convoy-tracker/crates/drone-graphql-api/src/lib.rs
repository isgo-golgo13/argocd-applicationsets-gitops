//! # Drone Convoy GraphQL API
//!
//! Production-grade GraphQL API service for the Drone Convoy Tracking System.
//!
//! ## Features
//!
//! - **Leaderboard Queries**: Real-time accuracy rankings for drone convoy
//! - **Engagement Tracking**: Record and query weapon engagement history
//! - **Subscriptions**: Real-time updates via WebSocket
//! - **DataLoader**: N+1 query prevention for efficient data fetching
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Axum HTTP Server                         │
//! │              (GraphQL Endpoint + Playground)                │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                async-graphql Schema                         │
//! │           (QueryRoot, MutationRoot, SubscriptionRoot)       │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    ApiContext                               │
//! │        (Repositories, Broadcast Channels, DataLoaders)      │
//! └─────────────────────────────────────────────────────────────┘
//!                    │                   │
//!                    ▼                   ▼
//! ┌─────────────────────────┐   ┌──────────────────────────────┐
//! │     Redis Cache         │   │        ScyllaDB              │
//! │  (Leaderboard, State)   │   │   (Source of Truth)          │
//! └─────────────────────────┘   └──────────────────────────────┘
//! ```

#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod config;
pub mod context;
pub mod error;
pub mod loaders;
pub mod resolvers;
pub mod schema;

use async_graphql::Schema;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    extract::State,
    http::Method,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub use config::Config;
pub use context::ApiContext;
pub use resolvers::{MutationRoot, QueryRoot, SubscriptionRoot};

/// GraphQL schema type
pub type ApiSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

/// Build the GraphQL schema with context
pub fn build_schema(ctx: ApiContext) -> ApiSchema {
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(ctx)
        .enable_subscription_in_federation()
        .limit_depth(10)
        .limit_complexity(1000)
        .finish()
}

/// Application state for Axum handlers
#[derive(Clone)]
pub struct AppState {
    pub schema: ApiSchema,
}

/// GraphQL endpoint handler
pub async fn graphql_handler(
    State(state): State<AppState>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    state.schema.execute(req.into_inner()).await.into()
}

/// GraphQL Playground HTML
pub async fn graphql_playground() -> impl IntoResponse {
    Html(
        async_graphql::http::playground_source(
            async_graphql::http::GraphQLPlaygroundConfig::new("/graphql")
                .subscription_endpoint("/graphql/ws"),
        ),
    )
}

/// Health check endpoint
pub async fn health_check() -> impl IntoResponse {
    "OK"
}

/// Build the Axum router
pub fn build_router(schema: ApiSchema) -> Router {
    let state = AppState { schema: schema.clone() };

    // CORS configuration
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_origin(Any)
        .allow_headers(Any);

    Router::new()
        // GraphQL endpoints
        .route("/graphql", get(graphql_playground).post(graphql_handler))
        // TODO: WebSocket subscriptions disabled until async-graphql-axum supports axum 0.8
        // .route("/graphql/ws", any(GraphQLSubscription::new(schema)))
        // Health check
        .route("/health", get(health_check))
        .route("/", get(|| async { "Drone Convoy Tracker API" }))
        // State and middleware
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}

/// Crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
