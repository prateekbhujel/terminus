//! OAuth 2.0 social login (authorization-code flow), Socialite-style.
//!
//! Configure a provider, redirect the user with `authorization_url`, then
//! `exchange_code` on the callback and `userinfo` for the profile. State is
//! generated and verified explicitly so callers keep CSRF control.

use serde::Deserialize;

/// OAuth user profile normalized across providers.
#[derive(Debug, Clone, PartialEq)]
pub struct OAuthUser {
    pub id: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub raw: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
}

/// An OAuth 2.0 provider client.
#[derive(Debug, Clone)]
pub struct OAuthClient {
    auth_url: String,
    token_url: String,
    userinfo_url: String,
    client_id: String,
    client_secret: String,
    scopes: Vec<String>,
}

impl OAuthClient {
    pub fn github(client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        Self {
            auth_url: "https://github.com/login/oauth/authorize".into(),
            token_url: "https://github.com/login/oauth/access_token".into(),
            userinfo_url: "https://api.github.com/user".into(),
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            scopes: vec!["read:user".into(), "user:email".into()],
        }
    }

    pub fn google(client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        Self {
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token_url: "https://oauth2.googleapis.com/token".into(),
            userinfo_url: "https://openidconnect.googleapis.com/v1/userinfo".into(),
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            scopes: vec!["openid".into(), "email".into(), "profile".into()],
        }
    }

    /// Any OAuth 2.0 provider from its three endpoints.
    pub fn custom(
        auth_url: impl Into<String>,
        token_url: impl Into<String>,
        userinfo_url: impl Into<String>,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Self {
        Self {
            auth_url: auth_url.into(),
            token_url: token_url.into(),
            userinfo_url: userinfo_url.into(),
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            scopes: vec!["openid".into(), "email".into(), "profile".into()],
        }
    }

    pub fn scopes(mut self, scopes: Vec<String>) -> Self {
        self.scopes = scopes;
        self
    }

    /// A fresh CSRF state token for `authorization_url`.
    pub fn new_state() -> String {
        crate::session::secure_random_hex(16)
    }

    /// The provider URL to redirect the user to.
    pub fn authorization_url(&self, redirect_uri: &str, state: &str) -> String {
        format!(
            "{}?client_id={}&redirect_uri={}&state={}&response_type=code&scope={}",
            self.auth_url,
            url_encode(&self.client_id),
            url_encode(redirect_uri),
            url_encode(state),
            url_encode(&self.scopes.join(" "))
        )
    }

    /// Exchange an authorization code for an access token.
    pub fn exchange_code(&self, code: &str, redirect_uri: &str) -> Result<String, String> {
        let response: TokenResponse = ureq::post(&self.token_url)
            .set("Accept", "application/json")
            .send_form(&[
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
                ("code", code),
                ("redirect_uri", redirect_uri),
                ("grant_type", "authorization_code"),
            ])
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())?;
        Ok(response.access_token)
    }

    /// Fetch the authenticated user's profile with an access token.
    pub fn userinfo(&self, access_token: &str) -> Result<OAuthUser, String> {
        let raw: serde_json::Value = ureq::get(&self.userinfo_url)
            .set("Authorization", &format!("Bearer {access_token}"))
            .set("Accept", "application/json")
            .set("User-Agent", "terminus-framework")
            .call()
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())?;
        let id = raw
            .get("id")
            .and_then(|v| v.as_i64().map(|n| n.to_string()).or_else(|| v.as_str().map(str::to_string)))
            .or_else(|| raw.get("sub").and_then(|v| v.as_str().map(str::to_string)))
            .ok_or("OAuth userinfo response has no id")?;
        Ok(OAuthUser {
            id,
            email: raw.get("email").and_then(|v| v.as_str()).map(str::to_string),
            name: raw
                .get("name")
                .and_then(|v| v.as_str())
                .or_else(|| raw.get("login").and_then(|v| v.as_str()))
                .map(str::to_string),
            avatar_url: raw
                .get("avatar_url")
                .and_then(|v| v.as_str())
                .or_else(|| raw.get("picture").and_then(|v| v.as_str()))
                .map(str::to_string),
            raw,
        })
    }
}

fn url_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
