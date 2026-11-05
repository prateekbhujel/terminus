//! Real-time WebSocket broadcasting system for Terminus
//! Provides pub/sub channels, instant record update broadcasts, and Axum WebSocket handlers.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;

/// Event payload sent over WebSockets
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BroadcastEvent {
    pub channel: String,
    pub event: String,
    pub data: serde_json::Value,
}

/// Thread-safe event broadcaster powered by Tokio async channels
#[derive(Clone)]
pub struct Broadcaster {
    sender: broadcast::Sender<BroadcastEvent>,
    presence: Arc<RwLock<HashMap<String, HashMap<String, serde_json::Value>>>>,
    authorizer: Arc<RwLock<Option<ChannelAuthorizer>>>,
}

impl Broadcaster {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            presence: Arc::new(RwLock::new(HashMap::new())),
            authorizer: Arc::new(RwLock::new(None)),
        }
    }

    /// Broadcast an event to all connected WebSocket clients
    pub fn broadcast(&self, channel: &str, event: &str, data: serde_json::Value) -> usize {
        let msg = BroadcastEvent {
            channel: channel.to_string(),
            event: event.to_string(),
            data,
        };
        self.sender.send(msg).unwrap_or(0)
    }

    /// Subscribe to real-time events
    pub fn subscribe(&self) -> broadcast::Receiver<BroadcastEvent> {
        self.sender.subscribe()
    }

    /// Get current active subscriber count
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for Broadcaster {
    fn default() -> Self {
        Self::new(256)
    }
}

/// Axum WebSocket upgrade handler
pub async fn ws_handler(ws: WebSocketUpgrade, broadcaster: Arc<Broadcaster>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, broadcaster))
}

async fn handle_socket(mut socket: WebSocket, broadcaster: Arc<Broadcaster>) {
    let mut rx = broadcaster.subscribe();

    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Ok(evt) => {
                        if let Ok(json_str) = serde_json::to_string(&evt) {
                            if socket.send(Message::Text(json_str)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            client_msg = socket.recv() => {
                match client_msg {
                    Some(Ok(Message::Ping(payload))) => {
                        if socket.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Private and presence channels (broadcast authorization + member tracking)
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::RwLock;

type ChannelAuthorizer = Arc<dyn Fn(&str, &str) -> bool + Send + Sync>;

#[derive(Debug, Clone, PartialEq)]
pub enum ChannelKind {
    Public,
    Private,
    Presence,
}

pub fn channel_kind(channel: &str) -> ChannelKind {
    if channel.starts_with("presence-") {
        ChannelKind::Presence
    } else if channel.starts_with("private-") {
        ChannelKind::Private
    } else {
        ChannelKind::Public
    }
}

impl Broadcaster {
    /// Register the authorization callback for private/presence channels.
    /// It receives the authenticated user id and channel name. Without an
    /// authorizer, private and presence subscriptions are denied.
    pub fn authorize_with<F>(&self, authorizer: F)
    where
        F: Fn(&str, &str) -> bool + Send + Sync + 'static,
    {
        if let Ok(mut slot) = self.authorizer.write() {
            *slot = Some(Arc::new(authorizer));
        }
    }

    /// Whether `user_id` may subscribe to `channel`. Public channels are
    /// open; private/presence channels need a registered authorizer to allow.
    pub fn authorize(&self, user_id: &str, channel: &str) -> bool {
        if channel_kind(channel) == ChannelKind::Public {
            return true;
        }
        let guard = match self.authorizer.read() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        match guard.as_ref() {
            Some(authorizer) => authorizer(user_id, channel),
            None => false,
        }
    }

    /// Add a member to a presence channel. Returns false when unauthorized.
    pub fn join_presence(
        &self,
        channel: &str,
        user_id: &str,
        info: serde_json::Value,
    ) -> bool {
        if channel_kind(channel) != ChannelKind::Presence || !self.authorize(user_id, channel) {
            return false;
        }
        if let Ok(mut presence) = self.presence.write() {
            presence
                .entry(channel.to_string())
                .or_default()
                .insert(user_id.to_string(), info);
        }
        self.broadcast(channel, "presence:joined", serde_json::json!({ "user_id": user_id }));
        true
    }

    /// Remove a member from a presence channel.
    pub fn leave_presence(&self, channel: &str, user_id: &str) -> bool {
        let removed = if let Ok(mut presence) = self.presence.write() {
            presence
                .get_mut(channel)
                .map(|members| members.remove(user_id).is_some())
                .unwrap_or(false)
        } else {
            false
        };
        if removed {
            self.broadcast(channel, "presence:left", serde_json::json!({ "user_id": user_id }));
        }
        removed
    }

    /// Current members of a presence channel: user id -> member info.
    pub fn presence_members(&self, channel: &str) -> HashMap<String, serde_json::Value> {
        self.presence
            .read()
            .ok()
            .and_then(|presence| presence.get(channel).cloned())
            .unwrap_or_default()
    }
}
