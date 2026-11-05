//! File storage (Laravel-style disks), local filesystem implementation.
//!
//! Paths are validated to stay inside the storage root: absolute paths and
//! `..` segments are rejected, so request-controlled names cannot escape.

use std::path::{Component, Path, PathBuf};

/// A local disk rooted at one directory.
#[derive(Debug, Clone)]
pub struct LocalStorage {
    root: PathBuf,
    url_prefix: String,
}

impl LocalStorage {
    /// Open (creating if needed) a storage disk at `root`.
    pub fn new(root: impl AsRef<Path>) -> std::io::Result<Self> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root)?;
        Ok(Self {
            root,
            url_prefix: "/storage".to_string(),
        })
    }

    /// Prefix used by `url()` (default `/storage`).
    pub fn url_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.url_prefix = prefix.into();
        self
    }

    fn resolve(&self, path: &str) -> Result<PathBuf, String> {
        let relative = Path::new(path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|part| matches!(part, Component::ParentDir))
        {
            return Err(format!("Invalid storage path: {path}"));
        }
        Ok(self.root.join(relative))
    }

    /// Write bytes to a path, creating parent directories.
    pub fn put(&self, path: &str, contents: &[u8]) -> Result<(), String> {
        let full = self.resolve(path)?;
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&full, contents).map_err(|e| e.to_string())
    }

    /// Read a file's bytes, or `None` when it does not exist.
    pub fn get(&self, path: &str) -> Result<Option<Vec<u8>>, String> {
        let full = self.resolve(path)?;
        match std::fs::read(&full) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    pub fn exists(&self, path: &str) -> bool {
        self.resolve(path).map(|full| full.exists()).unwrap_or(false)
    }

    /// Delete a file. Returns true when a file was removed.
    pub fn delete(&self, path: &str) -> Result<bool, String> {
        let full = self.resolve(path)?;
        match std::fs::remove_file(&full) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.to_string()),
        }
    }

    /// Public URL for a stored path: `{url_prefix}/{path}`.
    pub fn url(&self, path: &str) -> String {
        format!("{}/{}", self.url_prefix.trim_end_matches('/'), path.trim_start_matches('/'))
    }
}

/// An expiring, HMAC-signed URL for a local storage path (Laravel-style
/// temporary URLs). The signature covers the path and expiry; any tampering
/// or expiry makes verification fail.
pub fn signed_url(
    storage: &LocalStorage,
    path: &str,
    secret: &[u8],
    ttl_secs: i64,
) -> String {
    let expires = chrono::Utc::now().timestamp() + ttl_secs;
    let signature = signature_for(path, expires, secret);
    format!("{}?expires={expires}&signature={signature}", storage.url(path))
}

/// Verify a signed URL's path/expiry/signature at `now`.
pub fn verify_signed_url(
    path: &str,
    expires: i64,
    signature: &str,
    secret: &[u8],
    now: i64,
) -> bool {
    if expires < now {
        return false;
    }
    let expected = signature_for(path, expires, secret);
    crate::token::constant_time_eq_str(&expected, signature)
}

fn signature_for(path: &str, expires: i64, secret: &[u8]) -> String {
    crate::token::hmac_sha256_hex(secret, format!("{path}:{expires}").as_bytes())
}

/// Amazon S3 (and S3-compatible) storage disk, signed with AWS Signature
/// Version 4. Enable with the default `s3` Cargo feature.
///
/// Works against AWS S3, MinIO, Cloudflare R2, and other S3-compatible
/// endpoints by setting a custom endpoint.
#[cfg(feature = "s3")]
#[derive(Clone)]
pub struct S3Storage {
    bucket: String,
    region: String,
    endpoint: String,
    access_key: String,
    secret_key: String,
}

#[cfg(feature = "s3")]
impl S3Storage {
    /// An AWS S3 disk in `region` for `bucket`.
    pub fn new(
        bucket: impl Into<String>,
        region: impl Into<String>,
        access_key: impl Into<String>,
        secret_key: impl Into<String>,
    ) -> Self {
        let region = region.into();
        let endpoint = format!("https://s3.{region}.amazonaws.com");
        Self {
            bucket: bucket.into(),
            region,
            endpoint,
            access_key: access_key.into(),
            secret_key: secret_key.into(),
        }
    }

    /// Use a custom S3-compatible endpoint (MinIO, R2, Spaces, ...).
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into().trim_end_matches('/').to_string();
        self
    }

    fn object_url(&self, path: &str) -> String {
        let encoded: String = path
            .split('/')
            .map(uri_encode)
            .collect::<Vec<_>>()
            .join("/");
        format!("{}/{}/{}", self.endpoint, self.bucket, encoded)
    }

    /// An expiring pre-signed GET URL (SigV4 query signing), valid for
    /// `ttl_secs`, that anyone can use without credentials.
    pub fn temporary_url(&self, path: &str, ttl_secs: u32) -> String {
        let now = chrono::Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = now.format("%Y%m%d").to_string();
        let credential_scope = format!("{date_stamp}/{}/s3/aws4_request", self.region);
        let credential = format!("{}/{}", self.access_key, credential_scope);
        let encoded_path: String = path
            .split('/')
            .map(uri_encode)
            .collect::<Vec<_>>()
            .join("/");
        let canonical_uri = format!("/{}/{}", self.bucket, encoded_path);
        let host = self
            .endpoint
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .to_string();
        let query = format!(
            "X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential={}&X-Amz-Date={}&X-Amz-Expires={}&X-Amz-SignedHeaders=host",
            uri_encode_strict(&credential),
            amz_date,
            ttl_secs
        );
        let canonical_request = format!(
            "GET\n{canonical_uri}\n{query}\nhost:{host}\n\nhost\nUNSIGNED-PAYLOAD"
        );
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{amz_date}\n{credential_scope}\n{}",
            crate::token::sha256_hex(canonical_request.as_bytes())
        );
        let signature = sigv4_signature(&self.secret_key, &date_stamp, &self.region, &string_to_sign);
        format!(
            "{}://{host}{canonical_uri}?{query}&X-Amz-Signature={signature}",
            if self.endpoint.starts_with("https://") { "https" } else { "http" }
        )
    }

    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<(u16, Vec<u8>), String> {
        let url = self.object_url(path);
        let now = chrono::Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = now.format("%Y%m%d").to_string();
        let payload_hash = crate::token::sha256_hex(body.unwrap_or(&[]));
        let host = url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap_or("")
            .to_string();
        let canonical_uri = format!(
            "/{}",
            url.trim_start_matches("https://")
                .trim_start_matches("http://").split_once('/').map(|x| x.1)
                .unwrap_or("")
        );
        let canonical_headers =
            format!("host:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n");
        let signed_headers = "host;x-amz-content-sha256;x-amz-date";
        let canonical_request = format!(
            "{method}\n{canonical_uri}\n\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
        );
        let credential_scope = format!("{date_stamp}/{}/s3/aws4_request", self.region);
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{amz_date}\n{credential_scope}\n{}",
            crate::token::sha256_hex(canonical_request.as_bytes())
        );
        let signature = sigv4_signature(&self.secret_key, &date_stamp, &self.region, &string_to_sign);
        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}",
            self.access_key
        );

        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(30))
            .build();
        let mut request = agent
            .request(method, &url)
            .set("Authorization", &authorization)
            .set("x-amz-date", &amz_date)
            .set("x-amz-content-sha256", &payload_hash);
        if let Some(bytes) = body {
            request = request.set("Content-Length", &bytes.len().to_string());
            match request.send_bytes(bytes) {
                Ok(response) => {
                    let status = response.status();
                    let mut buf = Vec::new();
                    use std::io::Read;
                    response
                        .into_reader()
                        .read_to_end(&mut buf)
                        .map_err(|e| e.to_string())?;
                    Ok((status, buf))
                }
                Err(ureq::Error::Status(status, response)) => {
                    let text = response.into_string().unwrap_or_default();
                    if method == "GET" && status == 404 {
                        return Ok((404, Vec::new()));
                    }
                    Err(format!("S3 {method} failed with {status}: {text}"))
                }
                Err(error) => Err(error.to_string()),
            }
        } else {
            match request.call() {
                Ok(response) => {
                    let status = response.status();
                    let mut buf = Vec::new();
                    use std::io::Read;
                    response
                        .into_reader()
                        .read_to_end(&mut buf)
                        .map_err(|e| e.to_string())?;
                    Ok((status, buf))
                }
                Err(ureq::Error::Status(status, response)) => {
                    let text = response.into_string().unwrap_or_default();
                    if method == "GET" && status == 404 {
                        return Ok((404, Vec::new()));
                    }
                    if method == "HEAD" {
                        return Ok((status, Vec::new()));
                    }
                    Err(format!("S3 {method} failed with {status}: {text}"))
                }
                Err(error) => Err(error.to_string()),
            }
        }
    }

    pub fn put(&self, path: &str, contents: &[u8]) -> Result<(), String> {
        let (status, _) = self.request("PUT", path, Some(contents))?;
        if (200..300).contains(&status) {
            Ok(())
        } else {
            Err(format!("S3 PUT failed with status {status}"))
        }
    }

    pub fn get(&self, path: &str) -> Result<Option<Vec<u8>>, String> {
        let (status, body) = self.request("GET", path, None)?;
        if status == 404 {
            return Ok(None);
        }
        if (200..300).contains(&status) {
            Ok(Some(body))
        } else {
            Err(format!("S3 GET failed with status {status}"))
        }
    }

    pub fn exists(&self, path: &str) -> bool {
        matches!(self.request("HEAD", path, None), Ok((status, _)) if (200..300).contains(&status))
    }

    pub fn delete(&self, path: &str) -> Result<bool, String> {
        let existed = self.exists(path);
        let (status, _) = self.request("DELETE", path, None)?;
        if (200..300).contains(&status) || status == 404 {
            Ok(existed)
        } else {
            Err(format!("S3 DELETE failed with status {status}"))
        }
    }

    /// Public URL for an object on this disk.
    pub fn url(&self, path: &str) -> String {
        self.object_url(path)
    }
}

#[cfg(feature = "s3")]
fn uri_encode(segment: &str) -> String {
    let mut out = String::new();
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(feature = "s3")]
fn uri_encode_strict(value: &str) -> String {
    value.replace('/', "%2F")
}

#[cfg(feature = "s3")]
fn sigv4_signature(secret: &str, date_stamp: &str, region: &str, string_to_sign: &str) -> String {
    let k_date = crate::session::hmac_sha256(format!("AWS4{secret}").as_bytes(), date_stamp.as_bytes());
    let k_region = crate::session::hmac_sha256(&k_date, region.as_bytes());
    let k_service = crate::session::hmac_sha256(&k_region, b"s3");
    let k_signing = crate::session::hmac_sha256(&k_service, b"aws4_request");
    crate::token::hmac_sha256_hex(&k_signing, string_to_sign.as_bytes())
}
