//! # WebSocket Service
//!
//! GraphQL subscription client for real-time updates.

use crate::state::{use_app_state, EngagementEvent, LeaderboardEntry};
use chrono::Utc;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use wasm_bindgen::prelude::*;
use web_sys::{CloseEvent, MessageEvent, WebSocket};

const WS_URL: &str = "ws://localhost:8080/graphql/ws";

/// GraphQL WebSocket message types
#[derive(Serialize)]
#[serde(tag = "type")]
enum WsClientMessage {
    #[serde(rename = "connection_init")]
    ConnectionInit { payload: serde_json::Value },
    #[serde(rename = "subscribe")]
    Subscribe { id: String, payload: SubscribePayload },
}

#[derive(Serialize)]
struct SubscribePayload {
    query: String,
    variables: serde_json::Value,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
enum WsServerMessage {
    #[serde(rename = "connection_ack")]
    ConnectionAck,
    #[serde(rename = "next")]
    Next { id: String, payload: NextPayload },
    #[serde(rename = "error")]
    Error { id: String, payload: Vec<GraphQLError> },
    #[serde(rename = "complete")]
    Complete { id: String },
}

#[derive(Deserialize, Debug)]
struct NextPayload {
    data: serde_json::Value,
}

#[derive(Deserialize, Debug)]
struct GraphQLError {
    message: String,
}

/// WebSocket connection manager
pub struct WsClient {
    ws: WebSocket,
}

impl WsClient {
    pub fn connect(convoy_id: Uuid) -> Result<Self, JsValue> {
        let ws = WebSocket::new(WS_URL)?;
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        let state = use_app_state();
        let convoy_id_str = convoy_id.to_string();

        // Connection opened
        let ws_clone = ws.clone();
        let convoy_id_clone = convoy_id_str.clone();
        let onopen = Closure::wrap(Box::new(move |_| {
            log::info!("WebSocket connected");
            state.ws_connected.set(true);

            // Send connection init
            let init = WsClientMessage::ConnectionInit {
                payload: serde_json::json!({}),
            };
            let msg = serde_json::to_string(&init).unwrap();
            let _ = ws_clone.send_with_str(&msg);

            // Subscribe to engagement events
            let subscribe_engagements = WsClientMessage::Subscribe {
                id: "engagement-sub".to_string(),
                payload: SubscribePayload {
                    query: r#"
                        subscription EngagementEvents($convoyId: ID!) {
                            engagementEvents(convoyId: $convoyId) {
                                convoyId
                                droneId
                                callsign
                                hit
                                weaponType
                                newAccuracyPct
                                timestamp
                            }
                        }
                    "#.to_string(),
                    variables: serde_json::json!({
                        "convoyId": convoy_id_clone
                    }),
                },
            };
            let msg = serde_json::to_string(&subscribe_engagements).unwrap();
            let _ = ws_clone.send_with_str(&msg);

            // Subscribe to leaderboard updates
            let subscribe_leaderboard = WsClientMessage::Subscribe {
                id: "leaderboard-sub".to_string(),
                payload: SubscribePayload {
                    query: r#"
                        subscription LeaderboardUpdates($convoyId: ID!) {
                            leaderboardUpdates(convoyId: $convoyId) {
                                convoyId
                                droneId
                                callsign
                                newRank
                                oldRank
                                accuracyPct
                                changeType
                                timestamp
                            }
                        }
                    "#.to_string(),
                    variables: serde_json::json!({
                        "convoyId": convoy_id_clone
                    }),
                },
            };
            let msg = serde_json::to_string(&subscribe_leaderboard).unwrap();
            let _ = ws_clone.send_with_str(&msg);
        }) as Box<dyn FnMut(JsValue)>);
        ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
        onopen.forget();

        // Message received
        let state_clone = state.clone();
        let onmessage = Closure::wrap(Box::new(move |e: MessageEvent| {
            if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() {
                let msg_str: String = txt.into();
                if let Ok(msg) = serde_json::from_str::<WsServerMessage>(&msg_str) {
                    match msg {
                        WsServerMessage::ConnectionAck => {
                            log::info!("WebSocket connection acknowledged");
                        }
                        WsServerMessage::Next { id, payload } => {
                            handle_subscription_data(&state_clone, &id, payload.data);
                        }
                        WsServerMessage::Error { id, payload } => {
                            log::error!("Subscription error for {}: {:?}", id, payload);
                        }
                        WsServerMessage::Complete { id } => {
                            log::info!("Subscription {} completed", id);
                        }
                    }
                }
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        onmessage.forget();

        // Connection closed
        let state_close = state.clone();
        let onclose = Closure::wrap(Box::new(move |e: CloseEvent| {
            log::warn!("WebSocket closed: code={}, reason={}", e.code(), e.reason());
            state_close.ws_connected.set(false);
        }) as Box<dyn FnMut(CloseEvent)>);
        ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
        onclose.forget();

        // Error handler
        let onerror = Closure::wrap(Box::new(move |e: JsValue| {
            log::error!("WebSocket error: {:?}", e);
        }) as Box<dyn FnMut(JsValue)>);
        ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        onerror.forget();

        Ok(Self { ws })
    }

    pub fn close(&self) {
        let _ = self.ws.close();
    }
}

fn handle_subscription_data(
    state: &crate::state::AppState,
    subscription_id: &str,
    data: serde_json::Value,
) {
    match subscription_id {
        "engagement-sub" => {
            if let Some(event_data) = data.get("engagementEvents") {
                if let Ok(event) = serde_json::from_value::<EngagementEventData>(event_data.clone()) {
                    let engagement = EngagementEvent {
                        id: Uuid::new_v4(),
                        drone_id: Uuid::parse_str(&event.drone_id).unwrap_or_default(),
                        callsign: event.callsign,
                        hit: event.hit,
                        weapon_type: event.weapon_type,
                        new_accuracy_pct: event.new_accuracy_pct,
                        timestamp: Utc::now(),
                    };
                    state.engagements.update(|events| {
                        events.insert(0, engagement);
                        if events.len() > 50 {
                            events.truncate(50);
                        }
                    });
                }
            }
        }
        "leaderboard-sub" => {
            if let Some(update_data) = data.get("leaderboardUpdates") {
                log::debug!("Leaderboard update: {:?}", update_data);
                // Trigger leaderboard refresh
            }
        }
        _ => {}
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EngagementEventData {
    drone_id: String,
    callsign: String,
    hit: bool,
    weapon_type: String,
    new_accuracy_pct: f32,
}

/// Initialize WebSocket on mount
pub fn use_websocket(convoy_id: Signal<Option<Uuid>>) {
    Effect::new(move |_| {
        if let Some(id) = convoy_id.get() {
            match WsClient::connect(id) {
                Ok(_client) => {
                    log::info!("WebSocket client initialized for convoy {}", id);
                }
                Err(e) => {
                    log::error!("Failed to connect WebSocket: {:?}", e);
                }
            }
        }
    });
}
