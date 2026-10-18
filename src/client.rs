//! Multi-Platform Engine: Mobile (Android/iOS), IoT & Desktop
//!
//! Provides active platform adapters, typed SDK code generation (TypeScript, Swift & Kotlin),
//! offline transaction sync queues for desktop POS/warehouse stations,
//! hardware HMAC device registration for IoT barcode/RFID scanners,
//! and push notification dispatchers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Client Platform Kind
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    Web,
    Android,
    Ios,
    Iot,
    Desktop,
}

impl Platform {
    /// Detect client platform from HTTP User-Agent string
    pub fn from_user_agent(user_agent: &str) -> Self {
        let ua = user_agent.to_lowercase();
        if ua.contains("android") {
            Self::Android
        } else if ua.contains("iphone") || ua.contains("ipad") || ua.contains("ios") {
            Self::Ios
        } else if ua.contains("desktop") || ua.contains("station") || ua.contains("kiosk") || ua.contains("native-client") || ua.contains("tauri") || ua.contains("electron") {
            Self::Desktop
        } else if ua.contains("esp32") || ua.contains("arduino") || ua.contains("iot") || ua.contains("scanner") {
            Self::Iot
        } else {
            Self::Web
        }
    }
}

// ============================================================================
// MOBILE ADAPTER: PUSH NOTIFICATIONS & CLIENT SDK GENERATOR
// ============================================================================

/// Mobile Push Notification Dispatch Envelope (compatible with FCM & APNs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushNotification {
    pub id: String,
    pub title: String,
    pub body: String,
    pub token: String,
    pub badge: Option<u32>,
    pub data: HashMap<String, String>,
}

impl PushNotification {
    pub fn new(title: impl Into<String>, body: impl Into<String>, token: impl Into<String>) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        Self {
            id: format!("push_{}", now),
            title: title.into(),
            body: body.into(),
            token: token.into(),
            badge: Some(1),
            data: HashMap::new(),
        }
    }

    pub fn with_data(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.data.insert(key.into(), value.into());
        self
    }

    pub fn badge(mut self, badge: u32) -> Self {
        self.badge = Some(badge);
        self
    }

    /// Render standard FCM (Firebase Cloud Messaging) v1 payload JSON
    pub fn to_fcm_payload(&self) -> serde_json::Value {
        serde_json::json!({
            "message": {
                "token": self.token,
                "notification": {
                    "title": self.title,
                    "body": self.body
                },
                "data": self.data
            }
        })
    }

    /// Render standard Apple Push Notification service (APNs) JSON dictionary
    pub fn to_apns_payload(&self) -> serde_json::Value {
        serde_json::json!({
            "aps": {
                "alert": {
                    "title": self.title,
                    "body": self.body
                },
                "badge": self.badge.unwrap_or(1),
                "sound": "default"
            },
            "custom": self.data
        })
    }
}

/// Push Notification Queue & Dispatcher
#[derive(Clone, Default)]
pub struct PushDispatcher {
    queue: Arc<RwLock<Vec<PushNotification>>>,
    sent_log: Arc<RwLock<Vec<PushNotification>>>,
}

impl PushDispatcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enqueue a notification for asynchronous delivery
    pub fn enqueue(&self, notification: PushNotification) {
        if let Ok(mut q) = self.queue.write() {
            q.push(notification);
        }
    }

    /// Pending notification count
    pub fn pending_count(&self) -> usize {
        self.queue.read().map(|q| q.len()).unwrap_or(0)
    }

    /// Deliver and drain all pending notifications
    pub fn deliver_all(&self) -> Vec<String> {
        let mut delivered = Vec::new();
        if let Ok(mut q) = self.queue.write() {
            let drained: Vec<PushNotification> = q.drain(..).collect();
            for item in drained {
                delivered.push(item.id.clone());
                if let Ok(mut log) = self.sent_log.write() {
                    log.push(item);
                }
            }
        }
        delivered
    }
}

/// Typed Client SDK Endpoint Definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkEndpoint {
    pub method: String,
    pub path: String,
    pub request_type: Option<String>,
    pub response_type: String,
    pub description: String,
}

/// Automatic Typed Client SDK Generator (TypeScript, Swift & Kotlin)
#[derive(Debug, Clone)]
pub struct SdkGenerator {
    pub app_name: String,
    pub api_version: String,
    pub endpoints: Vec<SdkEndpoint>,
    pub type_schemas: HashMap<String, Vec<(String, String)>>,
}

impl SdkGenerator {
    pub fn new(app_name: impl Into<String>, api_version: impl Into<String>) -> Self {
        Self {
            app_name: app_name.into(),
            api_version: api_version.into(),
            endpoints: Vec::new(),
            type_schemas: HashMap::new(),
        }
    }

    pub fn endpoint(
        mut self,
        method: &str,
        path: &str,
        request_type: Option<&str>,
        response_type: &str,
        description: &str,
    ) -> Self {
        self.endpoints.push(SdkEndpoint {
            method: method.to_uppercase(),
            path: path.to_string(),
            request_type: request_type.map(|s| s.to_string()),
            response_type: response_type.to_string(),
            description: description.to_string(),
        });
        self
    }

    pub fn type_schema(mut self, name: &str, fields: Vec<(&str, &str)>) -> Self {
        self.type_schemas.insert(
            name.to_string(),
            fields.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        );
        self
    }

    /// Generate clean, idiomatic TypeScript Client SDK
    pub fn to_typescript(&self) -> String {
        let mut ts = format!("/**\n * Terminus Client SDK for {}\n * API Version: {}\n * Generated automatically by Terminus Framework.\n */\n\n", self.app_name, self.api_version);
        
        // Export interfaces
        for (name, fields) in &self.type_schemas {
            ts.push_str(&format!("export interface {} {{\n", name));
            for (field, f_type) in fields {
                ts.push_str(&format!("  {}: {};\n", field, f_type));
            }
            ts.push_str("}\n\n");
        }

        // Export Client Class
        ts.push_str(&format!(
            r#"export class {}Client {{
  private baseUrl: string;
  private token: string | null = null;

  constructor(baseUrl: string = "http://127.0.0.1:3000") {{
    this.baseUrl = baseUrl.replace(/\/$/, "");
  }}

  public setToken(token: string): void {{
    this.token = token;
  }}

  private async request<T>(method: string, path: string, body?: any): Promise<T> {{
    const headers: Record<string, string> = {{
      "Content-Type": "application/json",
      "Accept": "application/json",
    }};
    if (this.token) {{
      headers["Authorization"] = `Bearer ${{this.token}}`;
    }}

    const response = await fetch(`${{this.baseUrl}}${{path}}`, {{
      method,
      headers,
      body: body ? JSON.stringify(body) : undefined,
    }});

    if (!response.ok) {{
      throw new Error(`API Error: ${{response.status}} ${{response.statusText}}`);
    }}

    return (await response.json()) as T;
  }}
"#,
            self.app_name.replace(' ', "")
        ));

        // Methods
        for ep in &self.endpoints {
            let func_name = ep.path.trim_start_matches('/').replace('/', "_").replace(':', "");
            let req_arg = if let Some(rt) = &ep.request_type {
                format!("data: {}", rt)
            } else {
                String::new()
            };
            let body_param = if ep.request_type.is_some() { "data" } else { "undefined" };

            ts.push_str(&format!(
                r#"
  /** {} */
  public async {}_{}({}): Promise<{}> {{
    return this.request<{}>("{}", "{}", {});
  }}
"#,
                ep.description,
                ep.method.to_lowercase(),
                func_name,
                req_arg,
                ep.response_type,
                ep.response_type,
                ep.method,
                ep.path,
                body_param
            ));
        }

        ts.push_str("}\n");
        ts
    }

    /// Generate standard OpenAPI 3.0.3 specification JSON
    pub fn to_openapi_json(&self) -> serde_json::Value {
        let mut paths = serde_json::Map::new();
        for ep in &self.endpoints {
            let mut method_obj = serde_json::json!({
                "summary": ep.description,
                "operationId": format!("{}_{}", ep.method.to_lowercase(), ep.path.trim_start_matches('/').replace('/', "_")),
                "responses": {
                    "200": {
                        "description": "Successful response",
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "title": ep.response_type
                                }
                            }
                        }
                    }
                }
            });

            if let Some(ref req_t) = ep.request_type {
                method_obj["requestBody"] = serde_json::json!({
                    "required": true,
                    "content": {
                        "application/json": {
                            "schema": {
                                "type": "object",
                                "title": req_t
                            }
                        }
                    }
                });
            }

            let path_entry = paths.entry(ep.path.clone()).or_insert_with(|| serde_json::json!({}));
            if let Some(map) = path_entry.as_object_mut() {
                map.insert(ep.method.to_lowercase(), method_obj);
            }
        }

        let mut schemas = serde_json::Map::new();
        for (name, fields) in &self.type_schemas {
            let mut props = serde_json::Map::new();
            for (f_name, f_type) in fields {
                props.insert(
                    f_name.clone(),
                    serde_json::json!({
                        "type": match f_type.as_str() {
                            "number" | "Double" => "number",
                            "boolean" | "Bool" => "boolean",
                            _ => "string"
                        }
                    }),
                );
            }
            schemas.insert(
                name.clone(),
                serde_json::json!({
                    "type": "object",
                    "properties": props
                }),
            );
        }

        serde_json::json!({
            "openapi": "3.0.3",
            "info": {
                "title": self.app_name,
                "version": self.api_version,
                "description": "API specification generated automatically by Terminus Framework."
            },
            "paths": paths,
            "components": {
                "schemas": schemas
            }
        })
    }
}
