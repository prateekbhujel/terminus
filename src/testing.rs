//! Fluent testing assertions framework for Terminus & Rust
//! Provides `expect(val).to_be(...)`, chainable `.and(...)`, and readable assertions.

use std::fmt::Debug;

pub struct Expectation<T> {
    pub value: T,
}

/// Create a new fluent expectation
pub fn expect<T>(value: T) -> Expectation<T> {
    Expectation { value }
}

/// Fluent test wrapper function
pub fn test<F: FnOnce()>(_description: &str, closure: F) {
    closure();
}

impl<T> Expectation<T> {
    /// Chain another expectation
    pub fn and<U>(&self, next_value: U) -> Expectation<U> {
        Expectation { value: next_value }
    }
}

impl<T: Debug> Expectation<T> {
    /// Assert equality
    pub fn to_be<E: Debug>(self, expected: E) -> Self
    where
        T: PartialEq<E>,
    {
        assert!(
            self.value == expected,
            "Expectation failed: expected `{:?}` to be `{:?}`",
            self.value, expected
        );
        self
    }

    /// Assert equality (alias for to_be)
    pub fn to_equal<E: Debug>(self, expected: E) -> Self
    where
        T: PartialEq<E>,
    {
        self.to_be(expected)
    }

    /// Assert inequality
    pub fn to_not_be<E: Debug>(self, unexpected: E) -> Self
    where
        T: PartialEq<E>,
    {
        assert!(
            self.value != unexpected,
            "Expectation failed: expected `{:?}` to NOT be `{:?}`",
            self.value, unexpected
        );
        self
    }

    /// Assert inequality (alias for to_not_be)
    pub fn to_not_equal<E: Debug>(self, unexpected: E) -> Self
    where
        T: PartialEq<E>,
    {
        self.to_not_be(unexpected)
    }
}

impl<T> Expectation<Vec<T>> {
    /// Assert vector has expected length
    pub fn to_have_length(self, len: usize) -> Self {
        assert_eq!(
            self.value.len(),
            len,
            "Expectation failed: expected vector to have length {}, got {}",
            len,
            self.value.len()
        );
        self
    }
}

impl<T: Debug> Expectation<T> {
    /// Assert greater than
    pub fn to_be_greater_than<E: Debug>(self, other: E) -> Self
    where
        T: PartialOrd<E>,
    {
        assert!(
            self.value > other,
            "Expectation failed: expected `{:?}` to be greater than `{:?}`",
            self.value, other
        );
        self
    }

    /// Assert less than
    pub fn to_be_less_than<E: Debug>(self, other: E) -> Self
    where
        T: PartialOrd<E>,
    {
        assert!(
            self.value < other,
            "Expectation failed: expected `{:?}` to be less than `{:?}`",
            self.value, other
        );
        self
    }
}

impl Expectation<bool> {
    /// Assert boolean true
    pub fn to_be_true(self) -> Self {
        assert!(self.value, "Expectation failed: expected true, got false");
        self
    }

    /// Assert boolean false
    pub fn to_be_false(self) -> Self {
        assert!(!self.value, "Expectation failed: expected false, got true");
        self
    }
}

impl<T: AsRef<str>> Expectation<T> {
    /// Assert string contains substring
    pub fn to_contain(self, needle: &str) -> Self {
        let s = self.value.as_ref();
        assert!(
            s.contains(needle),
            "Expectation failed: expected string `{}` to contain `{}`",
            s, needle
        );
        self
    }

    /// Assert string does not contain substring
    pub fn to_not_contain(self, needle: &str) -> Self {
        let s = self.value.as_ref();
        assert!(
            !s.contains(needle),
            "Expectation failed: expected string `{}` to NOT contain `{}`",
            s, needle
        );
        self
    }

    /// Assert string is empty
    pub fn to_be_empty(self) -> Self {
        let s = self.value.as_ref();
        assert!(
            s.is_empty(),
            "Expectation failed: expected string to be empty, got `{}`",
            s
        );
        self
    }

    /// Assert string is not empty
    pub fn to_not_be_empty(self) -> Self {
        let s = self.value.as_ref();
        assert!(
            !s.is_empty(),
            "Expectation failed: expected string to NOT be empty"
        );
        self
    }
}

impl<T: Debug> Expectation<Option<T>> {
    /// Assert option is Some
    pub fn to_be_some(self) -> Self {
        assert!(
            self.value.is_some(),
            "Expectation failed: expected Option::Some, got Option::None"
        );
        self
    }

    /// Assert option is None
    pub fn to_be_none(self) -> Self {
        assert!(
            self.value.is_none(),
            "Expectation failed: expected Option::None, got `{:?}`",
            self.value
        );
        self
    }
}

impl<T: Debug, E: Debug> Expectation<Result<T, E>> {
    /// Assert result is Ok
    pub fn to_be_ok(self) -> Self {
        assert!(
            self.value.is_ok(),
            "Expectation failed: expected Result::Ok, got `{:?}`",
            self.value
        );
        self
    }

    /// Assert result is Err
    pub fn to_be_err(self) -> Self {
        assert!(
            self.value.is_err(),
            "Expectation failed: expected Result::Err, got `{:?}`",
            self.value
        );
        self
    }
}

use axum::{
    body::Body,
    http::{header, HeaderMap, Method, Request, StatusCode},
};

/// Simulated HTTP Response for headless test execution
#[derive(Debug, Clone)]
pub struct TestResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl TestResponse {
    /// Return the HTTP status code
    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// Return the raw numeric status code
    pub fn status_code(&self) -> u16 {
        self.status.as_u16()
    }

    /// Retrieve an HTTP header value by name
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|v| v.to_str().ok())
    }

    /// Return response body as UTF-8 string
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).to_string()
    }

    /// Parse response body as JSON Value
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap_or(serde_json::Value::Null)
    }

    /// Deserialize response body into typed struct
    pub fn try_json<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_slice(&self.body)
    }

    /// Assert exact HTTP status code
    pub fn assert_status(&self, expected: u16) -> &Self {
        assert_eq!(
            self.status.as_u16(),
            expected,
            "Expected HTTP status {}, but received {}. Body: {}",
            expected,
            self.status.as_u16(),
            self.text()
        );
        self
    }

    /// Assert HTTP 200 OK
    pub fn assert_ok(&self) -> &Self {
        self.assert_status(200)
    }

    /// Assert HTTP 201 Created
    pub fn assert_created(&self) -> &Self {
        self.assert_status(201)
    }

    /// Assert HTTP 400 Bad Request
    pub fn assert_bad_request(&self) -> &Self {
        self.assert_status(400)
    }

    /// Assert HTTP 401 Unauthorized
    pub fn assert_unauthorized(&self) -> &Self {
        self.assert_status(401)
    }

    /// Assert HTTP 403 Forbidden
    pub fn assert_forbidden(&self) -> &Self {
        self.assert_status(403)
    }

    /// Assert HTTP 404 Not Found
    pub fn assert_not_found(&self) -> &Self {
        self.assert_status(404)
    }

    /// Assert HTTP 422 Unprocessable Entity
    pub fn assert_unprocessable(&self) -> &Self {
        self.assert_status(422)
    }

    /// Assert response body contains substring
    pub fn assert_contains(&self, substring: &str) -> &Self {
        let text = self.text();
        assert!(
            text.contains(substring),
            "Expected response body to contain `{}`, but received: `{}`",
            substring,
            text
        );
        self
    }
}

/// Headless in-memory test client for Terminus and Axum applications
#[derive(Clone)]
pub struct TestClient {
    router: axum::Router,
}

impl TestClient {
    /// Create a test client wrapping a Terminus App instance
    pub fn new(app: crate::app::App) -> Self {
        Self {
            router: app.build(),
        }
    }

    /// Create a test client from an existing Axum Router
    pub fn from_router(router: axum::Router) -> Self {
        Self { router }
    }

    /// Dispatch an arbitrary HTTP request in-memory
    pub async fn request(&self, req: Request<Body>) -> TestResponse {
        use tower::ServiceExt;
        let app = self.router.clone();
        let res = app
            .oneshot(req)
            .await
            .expect("In-memory request execution failed");

        let status = res.status();
        let headers = res.headers().clone();
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .expect("Failed to read response body bytes");

        TestResponse {
            status,
            headers,
            body: bytes.to_vec(),
        }
    }

    /// Dispatch a GET request
    pub async fn get(&self, uri: &str) -> TestResponse {
        let req = Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Body::empty())
            .expect("Failed to build GET request");
        self.request(req).await
    }

    /// Dispatch a POST request with JSON payload
    pub async fn post(&self, uri: &str, body: serde_json::Value) -> TestResponse {
        let payload = serde_json::to_vec(&body).expect("Failed to serialize json payload");
        let req = Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(payload))
            .expect("Failed to build POST request");
        self.request(req).await
    }

    /// Dispatch a PUT request with JSON payload
    pub async fn put(&self, uri: &str, body: serde_json::Value) -> TestResponse {
        let payload = serde_json::to_vec(&body).expect("Failed to serialize json payload");
        let req = Request::builder()
            .method(Method::PUT)
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(payload))
            .expect("Failed to build PUT request");
        self.request(req).await
    }

    /// Dispatch a PATCH request with JSON payload
    pub async fn patch(&self, uri: &str, body: serde_json::Value) -> TestResponse {
        let payload = serde_json::to_vec(&body).expect("Failed to serialize json payload");
        let req = Request::builder()
            .method(Method::PATCH)
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(payload))
            .expect("Failed to build PATCH request");
        self.request(req).await
    }

    /// Dispatch a DELETE request
    pub async fn delete(&self, uri: &str) -> TestResponse {
        let req = Request::builder()
            .method(Method::DELETE)
            .uri(uri)
            .body(Body::empty())
            .expect("Failed to build DELETE request");
        self.request(req).await
    }
}
