//! Client Generator & OpenAPI Contract Engine
//!
//! Provides typed TypeScript client generation and standard OpenAPI 3.0.3
//! specification export from application endpoints and domain types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Client Platform Kind
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    Web,
    Mobile,
    Desktop,
}

impl Platform {
    /// Detect client platform from HTTP User-Agent string
    pub fn from_user_agent(user_agent: &str) -> Self {
        let ua = user_agent.to_lowercase();
        if ua.contains("android") || ua.contains("iphone") || ua.contains("ipad") || ua.contains("mobile") {
            Self::Mobile
        } else if ua.contains("desktop") || ua.contains("tauri") || ua.contains("electron") {
            Self::Desktop
        } else {
            Self::Web
        }
    }
}

/// Endpoint definition for client SDK generators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkEndpoint {
    pub method: String,
    pub path: String,
    pub request_type: Option<String>,
    pub response_type: String,
    pub description: String,
}

/// Typed Client SDK & OpenAPI Contract Generator
#[derive(Debug, Clone)]
pub struct SdkGenerator {
    pub app_name: String,
    pub version: String,
    pub endpoints: Vec<SdkEndpoint>,
    pub type_schemas: HashMap<String, Vec<(String, String)>>,
}

impl SdkGenerator {
    pub fn new(app_name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            app_name: app_name.into(),
            version: version.into(),
            endpoints: Vec::new(),
            type_schemas: HashMap::new(),
        }
    }

    pub fn type_schema(mut self, name: impl Into<String>, fields: Vec<(&str, &str)>) -> Self {
        let field_pairs = fields.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        self.type_schemas.insert(name.into(), field_pairs);
        self
    }

    pub fn endpoint(
        mut self,
        method: impl Into<String>,
        path: impl Into<String>,
        req: Option<&str>,
        res: impl Into<String>,
        desc: impl Into<String>,
    ) -> Self {
        self.endpoints.push(SdkEndpoint {
            method: method.into().to_uppercase(),
            path: path.into(),
            request_type: req.map(|r| r.to_string()),
            response_type: res.into(),
            description: desc.into(),
        });
        self
    }

    /// Generate an OpenAPI 3.0.3 compliant specification JSON object
    pub fn to_openapi_json(&self) -> serde_json::Value {
        let mut paths = serde_json::Map::new();

        for ep in &self.endpoints {
            let method_lower = ep.method.to_lowercase();
            let mut operation = serde_json::Map::new();
            operation.insert("summary".to_string(), serde_json::Value::String(ep.description.clone()));
            operation.insert(
                "operationId".to_string(),
                serde_json::Value::String(format!(
                    "{}_{}",
                    method_lower,
                    ep.path.replace('/', "_").trim_start_matches('_')
                )),
            );

            // Request body if present
            if let Some(req_type) = &ep.request_type {
                let mut content = serde_json::Map::new();
                content.insert(
                    "application/json".to_string(),
                    serde_json::json!({
                        "schema": {
                            "type": "object",
                            "description": req_type
                        }
                    }),
                );
                operation.insert(
                    "requestBody".to_string(),
                    serde_json::json!({
                        "required": true,
                        "content": content
                    }),
                );
            }

            // Responses
            let mut responses = serde_json::Map::new();
            responses.insert(
                "200".to_string(),
                serde_json::json!({
                    "description": "Successful response",
                    "content": {
                        "application/json": {
                            "schema": {
                                "type": "object",
                                "description": ep.response_type
                            }
                        }
                    }
                }),
            );
            operation.insert("responses".to_string(), serde_json::Value::Object(responses));

            let mut method_map = serde_json::Map::new();
            method_map.insert(method_lower, serde_json::Value::Object(operation));
            paths.insert(ep.path.clone(), serde_json::Value::Object(method_map));
        }

        // Components schemas
        let mut schemas = serde_json::Map::new();
        for (name, fields) in &self.type_schemas {
            let mut properties = serde_json::Map::new();
            for (fname, ftype) in fields {
                let openapi_type = match ftype.as_str() {
                    "number" | "u32" | "i64" | "f64" => "number",
                    "boolean" | "bool" => "boolean",
                    _ => "string",
                };
                properties.insert(
                    fname.clone(),
                    serde_json::json!({
                        "type": openapi_type
                    }),
                );
            }
            schemas.insert(
                name.clone(),
                serde_json::json!({
                    "type": "object",
                    "properties": properties
                }),
            );
        }

        serde_json::json!({
            "openapi": "3.0.3",
            "info": {
                "title": self.app_name,
                "version": self.version,
                "description": "Auto-generated OpenAPI 3.0.3 contract from Terminus application routes"
            },
            "paths": paths,
            "components": {
                "schemas": schemas
            }
        })
    }

    /// Generate strongly-typed TypeScript client SDK
    pub fn to_typescript(&self) -> String {
        let mut ts = String::new();
        ts.push_str(&format!("// Auto-generated TypeScript Client for {}\n", self.app_name));
        ts.push_str(&format!("// API Version: {}\n\n", self.version));

        // Generate TypeScript interfaces
        for (name, fields) in &self.type_schemas {
            ts.push_str(&format!("export interface {} {{\n", name));
            for (fname, ftype) in fields {
                ts.push_str(&format!("  {}: {};\n", fname, ftype));
            }
            ts.push_str("}\n\n");
        }

        // Generate API Client Class
        let class_name = {
            let sanitized: String = self.app_name
                .split(|c: char| !c.is_alphanumeric())
                .filter(|s| !s.is_empty())
                .map(|s| {
                    let mut c = s.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    }
                })
                .collect();
            if sanitized.is_empty() {
                "TerminusClient".to_string()
            } else {
                format!("{}Client", sanitized)
            }
        };
        ts.push_str(&format!("export class {} {{\n", class_name));
        ts.push_str("  private baseUrl: string;\n");
        ts.push_str("  private token?: string;\n\n");
        ts.push_str("  constructor(baseUrl: string = '', token?: string) {\n");
        ts.push_str("    this.baseUrl = baseUrl.replace(/\\/$/, '');\n");
        ts.push_str("    this.token = token;\n");
        ts.push_str("  }\n\n");

        ts.push_str("  private async request<T>(method: string, path: string, body?: any): Promise<T> {\n");
        ts.push_str("    const headers: Record<string, string> = {\n");
        ts.push_str("      'Content-Type': 'application/json',\n");
        ts.push_str("      'Accept': 'application/json',\n");
        ts.push_str("    };\n");
        ts.push_str("    if (this.token) {\n");
        ts.push_str("      headers['Authorization'] = `Bearer ${this.token}`;\n");
        ts.push_str("    }\n");
        ts.push_str("    const res = await fetch(`${this.baseUrl}${path}`, {\n");
        ts.push_str("      method,\n");
        ts.push_str("      headers,\n");
        ts.push_str("      body: body ? JSON.stringify(body) : undefined,\n");
        ts.push_str("    });\n");
        ts.push_str("    if (!res.ok) {\n");
        ts.push_str("      const err = await res.json().catch(() => ({ message: res.statusText }));\n");
        ts.push_str("      throw new Error(err.message || `Request failed with status ${res.status}`);\n");
        ts.push_str("    }\n");
        ts.push_str("    return res.json();\n");
        ts.push_str("  }\n\n");

        for ep in &self.endpoints {
            let fn_name = format!(
                "{}{}",
                ep.method.to_lowercase(),
                ep.path
                    .split('/')
                    .filter(|s| !s.is_empty())
                    .map(|s| {
                        let mut c = s.chars();
                        match c.next() {
                            None => String::new(),
                            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                        }
                    })
                    .collect::<String>()
            );

            ts.push_str(&format!("  /** {} */\n", ep.description));
            if let Some(req) = &ep.request_type {
                ts.push_str(&format!(
                    "  async {}(data: {}): Promise<{}> {{\n",
                    fn_name, req, ep.response_type
                ));
                ts.push_str(&format!(
                    "    return this.request<{}>('{}', '{}', data);\n",
                    ep.response_type, ep.method, ep.path
                ));
            } else {
                ts.push_str(&format!(
                    "  async {}(): Promise<{}> {{\n",
                    fn_name, ep.response_type
                ));
                ts.push_str(&format!(
                    "    return this.request<{}>('{}', '{}');\n",
                    ep.response_type, ep.method, ep.path
                ));
            }
            ts.push_str("  }\n\n");
        }

        ts.push_str("}\n");
        ts
    }
}
