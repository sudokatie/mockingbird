//! Mockingbird client.
//!
//! The main client that handles recording and playback of HTTP interactions.

use crate::cassette::{Cassette, IndexedCassette, Interaction, RecordedError, ErrorKind, load_cassette, save_cassette};
use crate::error::{Error, Result};
use crate::filter::{RequestFilter, ResponseFilter};
use crate::matcher::{AllMatcher, Matcher};
use crate::mode::Mode;
use crate::request::Request;
use crate::response::Response;
use chrono::{Duration as ChronoDuration, Utc};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Environment variable for default mode.
pub const ENV_MODE: &str = "MOCKINGBIRD_MODE";

/// Environment variable for cassette directory.
pub const ENV_CASSETTE_DIR: &str = "MOCKINGBIRD_CASSETTE_DIR";

/// Get the default mode from environment variable.
pub fn default_mode_from_env() -> Option<Mode> {
    std::env::var(ENV_MODE).ok().and_then(|s| s.parse().ok())
}

/// Get the cassette directory from environment variable.
pub fn cassette_dir_from_env() -> Option<PathBuf> {
    std::env::var(ENV_CASSETTE_DIR).ok().map(PathBuf::from)
}

/// Resolve a cassette path, using MOCKINGBIRD_CASSETTE_DIR if set.
pub fn resolve_cassette_path<P: AsRef<Path>>(path: P) -> PathBuf {
    let path = path.as_ref();
    if path.is_absolute() {
        path.to_path_buf()
    } else if let Some(dir) = cassette_dir_from_env() {
        dir.join(path)
    } else {
        path.to_path_buf()
    }
}

/// Mockingbird HTTP client.
pub struct Client {
    mode: Mode,
    cassette_path: PathBuf,
    cassette: Arc<RwLock<IndexedCassette>>,
    matcher: Box<dyn Matcher>,
    request_filters: Vec<Box<dyn RequestFilter>>,
    response_filters: Vec<Box<dyn ResponseFilter>>,
    expiration: Option<ChronoDuration>,
    http_client: reqwest::Client,
    /// Whether to record errors (timeouts, connection failures) as interactions.
    record_errors: bool,
}

/// Headers to strip when recording (these are modified by transport layer).
const HEADERS_TO_STRIP: &[&str] = &[
    "content-encoding",
    "transfer-encoding",
    "content-length",  // Recalculated after decompression
];

impl Client {
    /// Create a new client builder for recording.
    pub fn record<P: AsRef<Path>>(cassette_path: P) -> ClientBuilder {
        ClientBuilder::new(cassette_path).mode(Mode::Record)
    }
    
    /// Create a new client builder for playback.
    pub fn playback<P: AsRef<Path>>(cassette_path: P) -> ClientBuilder {
        ClientBuilder::new(cassette_path).mode(Mode::Replay)
    }
    
    /// Create a new client builder with auto mode.
    pub fn auto<P: AsRef<Path>>(cassette_path: P) -> ClientBuilder {
        ClientBuilder::new(cassette_path).mode(Mode::Auto)
    }
    
    /// Create a GET request.
    pub fn get(&self, url: impl Into<String>) -> RequestBuilder<'_> {
        RequestBuilder::new(self, Request::get(url))
    }
    
    /// Create a POST request.
    pub fn post(&self, url: impl Into<String>) -> RequestBuilder<'_> {
        RequestBuilder::new(self, Request::post(url))
    }
    
    /// Create a PUT request.
    pub fn put(&self, url: impl Into<String>) -> RequestBuilder<'_> {
        RequestBuilder::new(self, Request::put(url))
    }
    
    /// Create a DELETE request.
    pub fn delete(&self, url: impl Into<String>) -> RequestBuilder<'_> {
        RequestBuilder::new(self, Request::delete(url))
    }
    
    /// Create a PATCH request.
    pub fn patch(&self, url: impl Into<String>) -> RequestBuilder<'_> {
        RequestBuilder::new(self, Request::patch(url))
    }
    
    /// Create a HEAD request.
    pub fn head(&self, url: impl Into<String>) -> RequestBuilder<'_> {
        RequestBuilder::new(self, Request::head(url))
    }
    
    /// Create a request with any HTTP method.
    pub fn request(&self, method: impl Into<String>, url: impl Into<String>) -> RequestBuilder<'_> {
        RequestBuilder::new(self, Request::new(method, url))
    }
    
    /// Execute a request.
    pub async fn execute(&self, request: Request) -> Result<Response> {
        match self.mode {
            Mode::Replay => self.playback_request(request),
            Mode::Record => self.record_request(request).await,
            Mode::Auto => self.auto_request(request).await,
            Mode::Passthrough => self.passthrough_request(request).await,
        }
    }
    
    /// Save the cassette to disk.
    pub fn save(&self) -> Result<()> {
        let cassette = self.cassette.read().map_err(|_| Error::Config("Lock poisoned".into()))?;
        save_cassette(&self.cassette_path, cassette.cassette())
    }
    
    fn playback_request(&self, request: Request) -> Result<Response> {
        let mut recorded_request = request.to_recorded();
        
        // Apply request filters
        for filter in &self.request_filters {
            filter.filter(&mut recorded_request);
        }
        
        // Find matching interaction using indexed lookup for O(1) performance
        let cassette = self.cassette.read().map_err(|_| Error::Config("Lock poisoned".into()))?;
        
        // First try indexed lookup by method+URL (O(1))
        let candidate_indices = cassette.find_by_method_url(&recorded_request.method, &recorded_request.url);
        
        if !candidate_indices.is_empty() {
            // Check candidates with the full matcher
            for &idx in candidate_indices {
                if let Some(interaction) = cassette.get(idx) {
                    if self.matcher.matches(&interaction.request, &recorded_request) {
                        // Check expiration
                        if let Some(max_age) = self.expiration {
                            let age = Utc::now() - interaction.recorded_at;
                            if age > max_age {
                                return Err(Error::CassetteExpired {
                                    recorded_at: interaction.recorded_at,
                                    max_age,
                                });
                            }
                        }
                        return self.interaction_to_response(interaction);
                    }
                }
            }
        }
        
        // Fall back to linear search for custom matchers that may use different criteria
        for interaction in cassette.interactions() {
            if self.matcher.matches(&interaction.request, &recorded_request) {
                // Check expiration
                if let Some(max_age) = self.expiration {
                    let age = Utc::now() - interaction.recorded_at;
                    if age > max_age {
                        return Err(Error::CassetteExpired {
                            recorded_at: interaction.recorded_at,
                            max_age,
                        });
                    }
                }
                return self.interaction_to_response(interaction);
            }
        }
        
        Err(Error::NoMatch)
    }
    
    /// Convert an interaction to a response, handling both success and error cases.
    fn interaction_to_response(&self, interaction: &Interaction) -> Result<Response> {
        // Check for error interaction first
        if let Some(error) = &interaction.error {
            return Err(Self::recorded_error_to_error(error));
        }
        
        // Return the response if present
        match &interaction.response {
            Some(response) => Ok(Response::from_recorded(response.clone())),
            None => Err(Error::InvalidFormat("Interaction has no response or error".to_string())),
        }
    }
    
    /// Convert a RecordedError back to an Error for replay.
    fn recorded_error_to_error(error: &RecordedError) -> Error {
        match error.kind {
            ErrorKind::Timeout => Error::RecordedTimeout { message: error.message.clone() },
            ErrorKind::Connection => Error::RecordedConnection { message: error.message.clone() },
            ErrorKind::Dns => Error::RecordedDns { message: error.message.clone() },
            ErrorKind::Tls => Error::RecordedTls { message: error.message.clone() },
            ErrorKind::Cancelled => Error::RecordedCancelled { message: error.message.clone() },
            ErrorKind::Unknown => Error::RecordedUnknown { message: error.message.clone() },
        }
    }
    
    async fn record_request(&self, request: Request) -> Result<Response> {
        let mut recorded_request = request.to_recorded();
        
        // Apply request filters before making the request
        for filter in &self.request_filters {
            filter.filter(&mut recorded_request);
        }
        
        // Make real request, handling errors
        match self.make_real_request(&request).await {
            Ok(real_response) => {
                let response = Response::from_recorded(real_response.clone());
                
                // Apply response filters
                let mut recorded_response = real_response;
                for filter in &self.response_filters {
                    filter.filter(&mut recorded_response);
                }
                
                // Store successful interaction
                let interaction = Interaction::new(recorded_request, recorded_response);
                {
                    let mut cassette = self.cassette.write().map_err(|_| Error::Config("Lock poisoned".into()))?;
                    cassette.add(interaction);
                }
                
                // Auto-save
                self.save()?;
                
                Ok(response)
            }
            Err(err) => {
                // Optionally record the error
                if self.record_errors {
                    let recorded_error = Self::error_to_recorded(&err);
                    let interaction = Interaction::error(recorded_request, recorded_error);
                    {
                        let mut cassette = self.cassette.write().map_err(|_| Error::Config("Lock poisoned".into()))?;
                        cassette.add(interaction);
                    }
                    self.save()?;
                }
                
                Err(err)
            }
        }
    }
    
    /// Convert an Error to a RecordedError for storage.
    fn error_to_recorded(error: &Error) -> RecordedError {
        match error {
            Error::Http(reqwest_err) => {
                let message = reqwest_err.to_string();
                if reqwest_err.is_timeout() {
                    RecordedError::timeout(message)
                } else if reqwest_err.is_connect() {
                    RecordedError::connection(message)
                } else {
                    RecordedError::new(ErrorKind::Unknown, message)
                }
            }
            _ => RecordedError::new(ErrorKind::Unknown, error.to_string()),
        }
    }
    
    async fn auto_request(&self, request: Request) -> Result<Response> {
        // Try playback first
        if let Ok(response) = self.playback_request(request.clone()) {
            return Ok(response);
        }
        
        // Fall back to recording
        self.record_request(request).await
    }
    
    async fn passthrough_request(&self, request: Request) -> Result<Response> {
        let real_response = self.make_real_request(&request).await?;
        Ok(Response::from_recorded(real_response))
    }
    
    async fn make_real_request(&self, request: &Request) -> Result<crate::cassette::RecordedResponse> {
        let mut builder = self.http_client.request(
            request.method().parse().unwrap_or(reqwest::Method::GET),
            request.url(),
        );
        
        // Add headers
        for (name, value) in request.headers() {
            builder = builder.header(name, value);
        }
        
        // Add body
        if let Some(body) = request.get_body() {
            builder = builder.body(body.to_vec());
        }
        
        let response = builder.send().await?;
        
        // Convert to recorded response
        let status = response.status().as_u16();
        
        // Collect headers, stripping transport-layer headers that are modified
        // by decompression (reqwest automatically decompresses gzip/br/deflate)
        let headers: Vec<_> = response
            .headers()
            .iter()
            .filter(|(k, _)| {
                let name = k.as_str().to_lowercase();
                !HEADERS_TO_STRIP.contains(&name.as_str())
            })
            .map(|(k, v)| crate::cassette::Header::new(k.as_str(), v.to_str().unwrap_or("")))
            .collect();
        let body = response.bytes().await?;
        
        let mut recorded = crate::cassette::RecordedResponse::new(status);
        recorded.headers = headers;
        if !body.is_empty() {
            if let Ok(text) = String::from_utf8(body.to_vec()) {
                recorded.body = Some(text);
            } else {
                use base64::Engine;
                recorded.body = Some(base64::engine::general_purpose::STANDARD.encode(&body));
                recorded.body_encoding = crate::cassette::BodyEncoding::Base64;
            }
        }
        
        Ok(recorded)
    }
}

/// Builder for configuring a Client.
pub struct ClientBuilder {
    cassette_path: PathBuf,
    mode: Mode,
    matcher: Option<Box<dyn Matcher>>,
    request_filters: Vec<Box<dyn RequestFilter>>,
    response_filters: Vec<Box<dyn ResponseFilter>>,
    expiration: Option<ChronoDuration>,
    record_errors: bool,
    follow_redirects: bool,
}

impl ClientBuilder {
    /// Create a new builder.
    /// 
    /// If `MOCKINGBIRD_CASSETTE_DIR` is set, relative paths are resolved against it.
    /// If `MOCKINGBIRD_MODE` is set, it becomes the default mode.
    pub fn new<P: AsRef<Path>>(cassette_path: P) -> Self {
        Self {
            cassette_path: resolve_cassette_path(cassette_path),
            mode: default_mode_from_env().unwrap_or(Mode::Auto),
            matcher: None,
            request_filters: Vec::new(),
            response_filters: Vec::new(),
            expiration: None,
            record_errors: false,
            follow_redirects: true,
        }
    }
    
    /// Set the operating mode.
    pub fn mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
    }
    
    /// Set a custom matcher.
    pub fn matcher(mut self, matcher: impl Matcher + 'static) -> Self {
        self.matcher = Some(Box::new(matcher));
        self
    }
    
    /// Set matching strategy (alias for matcher).
    pub fn match_by(self, matcher: impl Matcher + 'static) -> Self {
        self.matcher(matcher)
    }
    
    /// Add a request filter.
    pub fn request_filter(mut self, filter: impl RequestFilter + 'static) -> Self {
        self.request_filters.push(Box::new(filter));
        self
    }
    
    /// Add a response filter.
    pub fn response_filter(mut self, filter: impl ResponseFilter + 'static) -> Self {
        self.response_filters.push(Box::new(filter));
        self
    }
    
    /// Set cassette expiration duration.
    pub fn expire_after(mut self, duration: Duration) -> Self {
        self.expiration = Some(ChronoDuration::from_std(duration).unwrap_or(ChronoDuration::MAX));
        self
    }
    
    /// Filter a request header (convenience method).
    pub fn filter_request_header(self, name: &str, replacement: &str) -> Self {
        self.request_filter(crate::filter::ReplaceRequestHeader::new(name, replacement))
    }
    
    /// Filter a response header (convenience method).
    pub fn filter_response_header(self, name: &str, replacement: &str) -> Self {
        self.response_filter(crate::filter::ReplaceResponseHeader::new(name, replacement))
    }
    
    /// Filter a JSON path in request body (convenience method).
    pub fn filter_request_body_json(self, json_path: &str, replacement: &str) -> Self {
        self.request_filter(crate::filter::JsonPathRequestFilter::new(json_path, replacement))
    }
    
    /// Filter a JSON path in response body (convenience method).
    pub fn filter_response_body_json(self, json_path: &str, replacement: &str) -> Self {
        self.response_filter(crate::filter::JsonPathResponseFilter::new(json_path, replacement))
    }
    
    /// Enable recording of errors (timeouts, connection failures).
    /// 
    /// When enabled, errors during recording are saved to the cassette and
    /// will be replayed during playback.
    pub fn record_errors(mut self, enable: bool) -> Self {
        self.record_errors = enable;
        self
    }
    
    /// Configure redirect following.
    /// 
    /// When `false`, redirects are captured as-is instead of following them.
    /// Default is `true` (follow redirects, store final response).
    pub fn follow_redirects(mut self, follow: bool) -> Self {
        self.follow_redirects = follow;
        self
    }
    
    /// Build the client.
    pub fn build(self) -> Result<Client> {
        let cassette = if self.cassette_path.exists() {
            load_cassette(&self.cassette_path)?
        } else {
            Cassette::new()
        };
        
        // Create indexed cassette for O(1) lookups
        let indexed_cassette = IndexedCassette::new(cassette);
        
        let matcher = self.matcher.unwrap_or_else(|| Box::new(AllMatcher::default_matchers()));
        
        // Build reqwest client with redirect policy
        let mut http_builder = reqwest::Client::builder();
        if !self.follow_redirects {
            http_builder = http_builder.redirect(reqwest::redirect::Policy::none());
        }
        let http_client = http_builder.build().map_err(|e| Error::Config(e.to_string()))?;
        
        Ok(Client {
            mode: self.mode,
            cassette_path: self.cassette_path,
            cassette: Arc::new(RwLock::new(indexed_cassette)),
            matcher,
            request_filters: self.request_filters,
            response_filters: self.response_filters,
            expiration: self.expiration,
            http_client,
            record_errors: self.record_errors,
        })
    }
}

/// Builder for a single request.
pub struct RequestBuilder<'a> {
    client: &'a Client,
    request: Request,
}

impl<'a> RequestBuilder<'a> {
    fn new(client: &'a Client, request: Request) -> Self {
        Self { client, request }
    }
    
    /// Add a header.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.request = self.request.header(name, value);
        self
    }
    
    /// Add multiple headers.
    pub fn headers(mut self, headers: std::collections::HashMap<String, String>) -> Self {
        self.request = self.request.with_headers(headers);
        self
    }
    
    /// Set the body.
    pub fn body(mut self, body: impl Into<bytes::Bytes>) -> Self {
        self.request = self.request.body(body);
        self
    }
    
    /// Set JSON body.
    pub fn json<T: serde::Serialize>(mut self, value: &T) -> Self {
        self.request = self.request.json(value);
        self
    }
    
    /// Set form body.
    pub fn form<T: serde::Serialize>(mut self, value: &T) -> Self {
        self.request = self.request.form(value);
        self
    }
    
    /// Add query parameters.
    pub fn query<T: serde::Serialize>(mut self, params: &T) -> Self {
        self.request = self.request.query(params);
        self
    }
    
    /// Set request timeout.
    pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
        self.request = self.request.timeout(timeout);
        self
    }
    
    /// Send the request.
    pub async fn send(self) -> Result<Response> {
        self.client.execute(self.request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_client_builder() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let client = Client::auto(&path).build().unwrap();
        assert!(matches!(client.mode, Mode::Auto));
    }

    #[test]
    fn test_playback_mode() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let client = Client::playback(&path).build().unwrap();
        assert!(matches!(client.mode, Mode::Replay));
    }

    #[test]
    fn test_record_mode() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let client = Client::record(&path).build().unwrap();
        assert!(matches!(client.mode, Mode::Record));
    }

    #[tokio::test]
    async fn test_playback_no_match() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("empty.json");
        save_cassette(&path, &Cassette::new()).unwrap();
        
        let client = Client::playback(&path).build().unwrap();
        let result = client.get("https://example.com/missing").send().await;
        
        assert!(matches!(result, Err(Error::NoMatch)));
    }

    #[tokio::test]
    async fn test_playback_returns_recorded() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        // Create cassette with one interaction
        let mut cassette = Cassette::new();
        let req = crate::cassette::RecordedRequest::new("GET", "https://example.com/api");
        let res = crate::cassette::RecordedResponse::new(200).body("recorded response");
        cassette.add(Interaction::new(req, res));
        save_cassette(&path, &cassette).unwrap();
        
        // Playback
        let client = Client::playback(&path).build().unwrap();
        let resp = client.get("https://example.com/api").send().await.unwrap();
        
        assert_eq!(resp.status(), 200);
        assert_eq!(resp.text().unwrap(), "recorded response");
    }

    #[tokio::test]
    async fn test_patch_method() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let mut cassette = Cassette::new();
        let req = crate::cassette::RecordedRequest::new("PATCH", "https://example.com/api");
        let res = crate::cassette::RecordedResponse::new(200).body("patched");
        cassette.add(Interaction::new(req, res));
        save_cassette(&path, &cassette).unwrap();
        
        let client = Client::playback(&path).build().unwrap();
        let resp = client.patch("https://example.com/api").send().await.unwrap();
        
        assert_eq!(resp.status(), 200);
        assert_eq!(resp.text().unwrap(), "patched");
    }

    #[tokio::test]
    async fn test_head_method() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let mut cassette = Cassette::new();
        let req = crate::cassette::RecordedRequest::new("HEAD", "https://example.com/api");
        let res = crate::cassette::RecordedResponse::new(200);
        cassette.add(Interaction::new(req, res));
        save_cassette(&path, &cassette).unwrap();
        
        let client = Client::playback(&path).build().unwrap();
        let resp = client.head("https://example.com/api").send().await.unwrap();
        
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn test_request_with_custom_method() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let mut cassette = Cassette::new();
        let req = crate::cassette::RecordedRequest::new("OPTIONS", "https://example.com/api");
        let res = crate::cassette::RecordedResponse::new(200);
        cassette.add(Interaction::new(req, res));
        save_cassette(&path, &cassette).unwrap();
        
        let client = Client::playback(&path).build().unwrap();
        let resp = client.request("OPTIONS", "https://example.com/api").send().await.unwrap();
        
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn test_request_with_query() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let mut cassette = Cassette::new();
        let req = crate::cassette::RecordedRequest::new("GET", "https://example.com/api?page=1&limit=10");
        let res = crate::cassette::RecordedResponse::new(200).body("results");
        cassette.add(Interaction::new(req, res));
        save_cassette(&path, &cassette).unwrap();
        
        let client = Client::playback(&path).build().unwrap();
        let resp = client
            .get("https://example.com/api")
            .query(&[("page", "1"), ("limit", "10")])
            .send()
            .await
            .unwrap();
        
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn test_request_with_form() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let client = Client::playback(&path).build().unwrap();
        
        // Just test that form() doesn't panic
        let builder = client.post("https://example.com/login")
            .form(&[("username", "test"), ("password", "secret")]);
        
        // The request should be properly constructed
        assert!(builder.request.get_header("content-type").is_some());
    }

    #[test]
    fn test_expire_after() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let client = Client::auto(&path)
            .expire_after(std::time::Duration::from_secs(3600))
            .build()
            .unwrap();
        
        assert!(client.expiration.is_some());
    }

    #[tokio::test]
    async fn test_cassette_expired() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        // Create cassette with old interaction
        let mut cassette = Cassette::new();
        let req = crate::cassette::RecordedRequest::new("GET", "https://example.com/api");
        let res = crate::cassette::RecordedResponse::new(200);
        let mut interaction = Interaction::new(req, res);
        // Set recorded_at to 2 days ago
        interaction.recorded_at = Utc::now() - ChronoDuration::days(2);
        cassette.interactions.push(interaction);
        save_cassette(&path, &cassette).unwrap();
        
        // Create client with 1 day expiration
        let client = Client::playback(&path)
            .expire_after(std::time::Duration::from_secs(24 * 60 * 60))
            .build()
            .unwrap();
        
        let result = client.get("https://example.com/api").send().await;
        assert!(matches!(result, Err(Error::CassetteExpired { .. })));
    }

    #[test]
    fn test_filter_request_header_convenience() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        // Just verify it compiles and builds
        let _client = Client::auto(&path)
            .filter_request_header("Authorization", "[REDACTED]")
            .build()
            .unwrap();
    }

    #[test]
    fn test_filter_response_header_convenience() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let _client = Client::auto(&path)
            .filter_response_header("Set-Cookie", "[REDACTED]")
            .build()
            .unwrap();
    }

    #[test]
    fn test_filter_request_body_json_convenience() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let _client = Client::auto(&path)
            .filter_request_body_json("$.password", "[FILTERED]")
            .build()
            .unwrap();
    }

    #[test]
    fn test_filter_response_body_json_convenience() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let _client = Client::auto(&path)
            .filter_response_body_json("$.api_key", "[FILTERED]")
            .build()
            .unwrap();
    }

    #[test]
    fn test_env_mode_parsing() {
        // Test the function exists and works
        std::env::remove_var(ENV_MODE);
        assert!(default_mode_from_env().is_none());
    }

    #[test]
    fn test_resolve_cassette_path_absolute() {
        let abs_path = PathBuf::from("/absolute/path/cassette.json");
        let resolved = resolve_cassette_path(&abs_path);
        assert_eq!(resolved, abs_path);
    }

    #[test]
    fn test_resolve_cassette_path_relative() {
        std::env::remove_var(ENV_CASSETTE_DIR);
        let rel_path = PathBuf::from("cassette.json");
        let resolved = resolve_cassette_path(&rel_path);
        assert_eq!(resolved, rel_path);
    }

    #[test]
    fn test_match_by_alias() {
        use crate::matcher::UrlMethodMatcher;
        
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        // Just verify match_by compiles and works as alias for matcher
        let _client = Client::auto(&path)
            .match_by(UrlMethodMatcher)
            .build()
            .unwrap();
    }

    #[test]
    fn test_headers_method() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let client = Client::playback(&path).build().unwrap();
        
        let mut headers = std::collections::HashMap::new();
        headers.insert("X-Test".to_string(), "value".to_string());
        
        // Just test that headers() doesn't panic
        let _builder = client.get("https://example.com").headers(headers);
    }

    #[test]
    fn test_follow_redirects_default() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        // Default should be follow_redirects = true
        let _client = Client::auto(&path).build().unwrap();
    }

    #[test]
    fn test_follow_redirects_disabled() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        // Should compile and build without error
        let _client = Client::auto(&path)
            .follow_redirects(false)
            .build()
            .unwrap();
    }

    #[test]
    fn test_record_errors_config() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        // Should compile and build without error
        let _client = Client::auto(&path)
            .record_errors(true)
            .build()
            .unwrap();
    }

    #[test]
    fn test_headers_to_strip() {
        // Verify the HEADERS_TO_STRIP constant contains expected values
        assert!(HEADERS_TO_STRIP.contains(&"content-encoding"));
        assert!(HEADERS_TO_STRIP.contains(&"transfer-encoding"));
        assert!(HEADERS_TO_STRIP.contains(&"content-length"));
    }
}
