//! Synchronous in-process event bus (Laravel-style events & listeners).
//!
//! Handlers run in registration order on the caller's thread. For work that
//! must survive the process or be retried, dispatch a queue job instead.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

/// An event: a name plus a JSON payload.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub name: String,
    pub payload: serde_json::Value,
}

impl Event {
    pub fn new(name: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            name: name.into(),
            payload,
        }
    }
}

type Listener = Arc<dyn Fn(&Event) + Send + Sync>;

/// Thread-safe event bus with named listeners.
#[derive(Clone, Default)]
pub struct EventBus {
    listeners: Arc<RwLock<HashMap<String, Vec<Listener>>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a listener for an event name. Listeners run in registration order.
    pub fn listen<F>(&self, event: impl Into<String>, listener: F)
    where
        F: Fn(&Event) + Send + Sync + 'static,
    {
        let mut map = self.listeners.write().unwrap();
        map.entry(event.into()).or_default().push(Arc::new(listener));
    }

    /// Dispatch an event to all of its listeners. Returns how many ran.
    pub fn dispatch(&self, event: Event) -> usize {
        let listeners = {
            let map = self.listeners.read().unwrap();
            map.get(&event.name).cloned().unwrap_or_default()
        };
        let count = listeners.len();
        for listener in listeners {
            listener(&event);
        }
        count
    }

    pub fn listener_count(&self, event: &str) -> usize {
        self.listeners
            .read()
            .unwrap()
            .get(event)
            .map(|list| list.len())
            .unwrap_or(0)
    }

    /// The process-wide bus used by the free `listen`/`dispatch` helpers.
    pub fn global() -> &'static Self {
        static BUS: OnceLock<EventBus> = OnceLock::new();
        BUS.get_or_init(EventBus::new)
    }
}

/// Register a listener on the global bus.
pub fn listen<F>(event: impl Into<String>, listener: F)
where
    F: Fn(&Event) + Send + Sync + 'static,
{
    EventBus::global().listen(event, listener);
}

/// Dispatch an event on the global bus. Returns how many listeners ran.
pub fn dispatch(name: impl Into<String>, payload: serde_json::Value) -> usize {
    EventBus::global().dispatch(Event::new(name, payload))
}
