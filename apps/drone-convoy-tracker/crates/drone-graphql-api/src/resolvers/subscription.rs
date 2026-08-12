//! # GraphQL Subscription Resolver
//!
//! Real-time event subscriptions for the drone convoy API.

use async_graphql::{Context, Subscription, ID};
use futures_util::Stream;

use crate::context::ApiContext;
use crate::schema::*;

/// GraphQL Subscription root
pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    /// Subscribe to engagement events for a convoy
    ///
    /// Emits an event whenever a drone records a hit or miss.
    #[graphql(name = "engagementEvents")]
    async fn engagement_events(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Convoy ID to filter events for")]
        convoy_id: ID,
    ) -> async_graphql::Result<impl Stream<Item = EngagementEvent>> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let mut rx = api_ctx.engagement_tx.subscribe();
        let filter_id = convoy_id.to_string();

        Ok(async_stream::stream! {
            while let Ok(event) = rx.recv().await {
                if event.convoy_id.as_str() == filter_id {
                    yield event;
                }
            }
        })
    }

    /// Subscribe to all engagement events across all convoys
    #[graphql(name = "allEngagementEvents")]
    async fn all_engagement_events(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<impl Stream<Item = EngagementEvent>> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let mut rx = api_ctx.engagement_tx.subscribe();

        Ok(async_stream::stream! {
            while let Ok(event) = rx.recv().await {
                yield event;
            }
        })
    }

    /// Subscribe to leaderboard position changes
    ///
    /// Emits an event whenever a drone's rank changes.
    #[graphql(name = "leaderboardUpdates")]
    async fn leaderboard_updates(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Convoy ID to filter updates for")]
        convoy_id: ID,
    ) -> async_graphql::Result<impl Stream<Item = LeaderboardUpdateEvent>> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let mut rx = api_ctx.leaderboard_tx.subscribe();
        let filter_id = convoy_id.to_string();

        Ok(async_stream::stream! {
            while let Ok(event) = rx.recv().await {
                if event.convoy_id.as_str() == filter_id {
                    yield event;
                }
            }
        })
    }

    /// Subscribe to drone status changes
    #[graphql(name = "droneStatusChanges")]
    async fn drone_status_changes(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Convoy ID to filter events for")]
        convoy_id: ID,
    ) -> async_graphql::Result<impl Stream<Item = DroneStatusEvent>> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let mut rx = api_ctx.drone_status_tx.subscribe();
        let filter_id = convoy_id.to_string();

        Ok(async_stream::stream! {
            while let Ok(event) = rx.recv().await {
                if event.convoy_id.as_str() == filter_id {
                    yield event;
                }
            }
        })
    }

    /// Subscribe to alerts for a convoy
    #[graphql(name = "alerts")]
    async fn alerts(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Convoy ID to filter alerts for")]
        convoy_id: ID,
        #[graphql(desc = "Minimum severity to receive (default: all)")]
        min_severity: Option<AlertSeverity>,
    ) -> async_graphql::Result<impl Stream<Item = AlertEvent>> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let mut rx = api_ctx.alert_tx.subscribe();
        let filter_id = convoy_id.to_string();

        Ok(async_stream::stream! {
            while let Ok(event) = rx.recv().await {
                if event.convoy_id.as_str() != filter_id {
                    continue;
                }

                // Filter by severity if specified
                let passes_filter = match min_severity {
                    None => true,
                    Some(AlertSeverity::Info) => true,
                    Some(AlertSeverity::Warning) => {
                        matches!(event.severity, AlertSeverity::Warning | AlertSeverity::Critical)
                    }
                    Some(AlertSeverity::Critical) => {
                        matches!(event.severity, AlertSeverity::Critical)
                    }
                };

                if passes_filter {
                    yield event;
                }
            }
        })
    }

    /// Subscribe to telemetry updates for a specific drone
    #[graphql(name = "droneTelemetry")]
    async fn drone_telemetry(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Drone ID to receive telemetry for")]
        drone_id: ID,
    ) -> async_graphql::Result<impl Stream<Item = TelemetrySnapshot>> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let mut rx = api_ctx.telemetry_tx.subscribe();
        let filter_id = drone_id.to_string();

        Ok(async_stream::stream! {
            while let Ok(snapshot) = rx.recv().await {
                if snapshot.drone_id.as_str() == filter_id {
                    yield snapshot;
                }
            }
        })
    }

    /// Heartbeat subscription for connection keep-alive
    ///
    /// Emits a timestamp every second.
    #[graphql(name = "heartbeat")]
    async fn heartbeat(&self) -> impl Stream<Item = String> {
        async_stream::stream! {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            loop {
                interval.tick().await;
                yield chrono::Utc::now().to_rfc3339();
            }
        }
    }
}
