//! Observability: in-process HTTP metrics and a recent-request log
//! (Pulse/Telescope-lite). Enable with `App::metrics(true)`; read via
//! [`MetricsRegistry::global`] or [`metrics_snapshot`].

use std::collections::{HashMap, VecDeque};
use std::sync::{OnceLock, RwLock};

const MAX_RECENT: usize = 200;

#[derive(Debug, Clone)]
pub struct RequestRecord {
    pub method: String,
    pub path: String,
    pub status: u16,
    pub duration_ms: f64,
    pub at_unix: i64,
}

/// Thread-safe metrics registry with a bounded recent-request ring buffer.
#[derive(Debug)]
pub struct MetricsRegistry {
    records: RwLock<VecDeque<RequestRecord>>,
    total: RwLock<u64>,
}

impl MetricsRegistry {
    pub fn new() -> Self {
        Self {
            records: RwLock::new(VecDeque::new()),
            total: RwLock::new(0),
        }
    }

    pub fn record(&self, record: RequestRecord) {
        if let Ok(mut total) = self.total.write() {
            *total += 1;
        }
        if let Ok(mut records) = self.records.write() {
            if records.len() >= MAX_RECENT {
                records.pop_front();
            }
            records.push_back(record);
        }
    }

    /// Aggregate snapshot: totals, status classes, latency averages, and
    /// the most recent requests (newest first).
    pub fn snapshot(&self) -> serde_json::Value {
        let total = self.total.read().map(|t| *t).unwrap_or(0);
        let records = match self.records.read() {
            Ok(records) => records,
            Err(_) => {
                return serde_json::json!({ "total_requests": total, "recent": [] });
            }
        };
        let mut by_status: HashMap<String, u64> = HashMap::new();
        let mut errors = 0u64;
        let mut duration_sum = 0f64;
        let mut slowest = 0f64;
        for record in records.iter() {
            *by_status
                .entry(format!("{}xx", record.status / 100))
                .or_default() += 1;
            if record.status >= 500 {
                errors += 1;
            }
            duration_sum += record.duration_ms;
            slowest = slowest.max(record.duration_ms);
        }
        let sampled = records.len() as f64;
        let recent: Vec<serde_json::Value> = records
            .iter()
            .rev()
            .take(20)
            .map(|record| {
                serde_json::json!({
                    "method": record.method,
                    "path": record.path,
                    "status": record.status,
                    "duration_ms": record.duration_ms,
                    "at_unix": record.at_unix,
                })
            })
            .collect();
        serde_json::json!({
            "total_requests": total,
            "sampled_requests": records.len(),
            "by_status_class": by_status,
            "server_errors": errors,
            "avg_duration_ms": if sampled > 0.0 { duration_sum / sampled } else { 0.0 },
            "slowest_sampled_ms": slowest,
            "recent": recent,
        })
    }

    pub fn global() -> &'static Self {
        static REGISTRY: OnceLock<MetricsRegistry> = OnceLock::new();
        REGISTRY.get_or_init(MetricsRegistry::new)
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of the process-global metrics registry.
pub fn metrics_snapshot() -> serde_json::Value {
    MetricsRegistry::global().snapshot()
}
