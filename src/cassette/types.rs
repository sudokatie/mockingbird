//! Cassette data types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The version string for mockingbird cassettes.
pub const MOCKINGBIRD_VERSION: &str = env!("CARGO_PKG_VERSION");

/// A cassette containing recorded HTTP interactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cassette {
    /// Tool that recorded this cassette (e.g., "mockingbird/0.1.0").
    #[serde(default = "default_recorded_with")]
    pub recorded_with: String,
    
    /// Cassette format version.
    pub version: u32,
    
    /// When this cassette was created.
    pub created_at: DateTime<Utc>,
    
    /// When this cassette was last modified.
    pub modified_at: DateTime<Utc>,
    
    /// Recorded interactions.
    pub interactions: Vec<Interaction>,
    
    /// Optional metadata.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

fn default_recorded_with() -> String {
    format!("mockingbird/{}", MOCKINGBIRD_VERSION)
}

impl Cassette {
    /// Create a new empty cassette.
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            recorded_with: format!("mockingbird/{}", MOCKINGBIRD_VERSION),
            version: 1,
            created_at: now,
            modified_at: now,
            interactions: Vec::new(),
            metadata: HashMap::new(),
        }
    }
    
    /// Add an interaction to the cassette.
    #[allow(clippy::should_implement_trait)]
    pub fn add(&mut self, interaction: Interaction) {
        self.interactions.push(interaction);
        self.modified_at = Utc::now();
    }
    
    /// Get the number of interactions.
    pub fn len(&self) -> usize {
        self.interactions.len()
    }
    
    /// Check if the cassette is empty.
    pub fn is_empty(&self) -> bool {
        self.interactions.is_empty()
    }
}

impl Default for Cassette {
    fn default() -> Self {
        Self::new()
    }
}

/// A single HTTP interaction (request + response or error).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interaction {
    /// The recorded request.
    pub request: RecordedRequest,
    
    /// The recorded response (present for successful requests).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<RecordedResponse>,
    
    /// Error information (present for failed requests).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<RecordedError>,
    
    /// When this interaction was recorded.
    pub recorded_at: DateTime<Utc>,
}

impl Interaction {
    /// Create a new successful interaction.
    pub fn new(request: RecordedRequest, response: RecordedResponse) -> Self {
        Self {
            request,
            response: Some(response),
            error: None,
            recorded_at: Utc::now(),
        }
    }
    
    /// Create a new error interaction.
    pub fn error(request: RecordedRequest, error: RecordedError) -> Self {
        Self {
            request,
            response: None,
            error: Some(error),
            recorded_at: Utc::now(),
        }
    }
    
    /// Check if this interaction represents an error.
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }
    
    /// Get the response if this is a successful interaction.
    pub fn get_response(&self) -> Option<&RecordedResponse> {
        self.response.as_ref()
    }
    
    /// Get the error if this is an error interaction.
    pub fn get_error(&self) -> Option<&RecordedError> {
        self.error.as_ref()
    }
}

/// A recorded HTTP error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedError {
    /// The kind of error that occurred.
    pub kind: ErrorKind,
    
    /// Human-readable error message.
    pub message: String,
}

impl RecordedError {
    /// Create a new recorded error.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
    
    /// Create a timeout error.
    pub fn timeout(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Timeout, message)
    }
    
    /// Create a connection error.
    pub fn connection(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Connection, message)
    }
    
    /// Create a DNS error.
    pub fn dns(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Dns, message)
    }
}

/// The kind of HTTP error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// Request timed out.
    Timeout,
    /// Connection failed (refused, reset, etc).
    Connection,
    /// DNS resolution failed.
    Dns,
    /// TLS/SSL error.
    Tls,
    /// Request was cancelled.
    Cancelled,
    /// Other/unknown error.
    Unknown,
}

/// A recorded HTTP request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedRequest {
    /// HTTP method.
    pub method: String,
    
    /// Request URL.
    pub url: String,
    
    /// Request headers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<Header>,
    
    /// Request body (base64 encoded if binary).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    
    /// Whether the body is base64 encoded.
    #[serde(default)]
    pub body_encoding: BodyEncoding,
}

impl RecordedRequest {
    /// Create a new recorded request.
    pub fn new(method: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            url: url.into(),
            headers: Vec::new(),
            body: None,
            body_encoding: BodyEncoding::default(),
        }
    }
    
    /// Add a header.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push(Header::new(name, value));
        self
    }
    
    /// Set the body.
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }
}

/// A recorded HTTP response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedResponse {
    /// HTTP status code.
    pub status: u16,
    
    /// Response headers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<Header>,
    
    /// Response body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    
    /// Whether the body is base64 encoded.
    #[serde(default)]
    pub body_encoding: BodyEncoding,
}

impl RecordedResponse {
    /// Create a new recorded response.
    pub fn new(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: None,
            body_encoding: BodyEncoding::default(),
        }
    }
    
    /// Add a header.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push(Header::new(name, value));
        self
    }
    
    /// Set the body.
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }
}

/// HTTP header.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Header {
    /// Header name.
    pub name: String,
    
    /// Header value.
    pub value: String,
}

impl Header {
    /// Create a new header.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// Body encoding type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BodyEncoding {
    /// Plain text (UTF-8).
    #[default]
    Text,
    
    /// Base64 encoded binary.
    Base64,
}

/// An indexed cassette for O(1) interaction lookup.
/// 
/// Wraps a Cassette and maintains an index by method+URL for fast matching.
#[derive(Debug)]
pub struct IndexedCassette {
    cassette: Cassette,
    /// Index: (method, url) -> Vec<interaction_index>
    /// Multiple interactions may match the same method+url
    index: HashMap<(String, String), Vec<usize>>,
}

impl IndexedCassette {
    /// Create an indexed cassette from a regular cassette.
    pub fn new(cassette: Cassette) -> Self {
        let mut index: HashMap<(String, String), Vec<usize>> = HashMap::new();
        
        for (i, interaction) in cassette.interactions.iter().enumerate() {
            let key = (
                interaction.request.method.to_uppercase(),
                interaction.request.url.clone(),
            );
            index.entry(key).or_default().push(i);
        }
        
        Self { cassette, index }
    }
    
    /// Find interactions matching the given method and URL.
    /// Returns indices into the interactions vec for O(1) lookup.
    pub fn find_by_method_url(&self, method: &str, url: &str) -> &[usize] {
        let key = (method.to_uppercase(), url.to_string());
        self.index.get(&key).map(|v| v.as_slice()).unwrap_or(&[])
    }
    
    /// Get an interaction by index.
    pub fn get(&self, index: usize) -> Option<&Interaction> {
        self.cassette.interactions.get(index)
    }
    
    /// Get all interactions (for linear search when custom matchers are used).
    pub fn interactions(&self) -> &[Interaction] {
        &self.cassette.interactions
    }
    
    /// Add an interaction and update the index.
    pub fn add(&mut self, interaction: Interaction) {
        let key = (
            interaction.request.method.to_uppercase(),
            interaction.request.url.clone(),
        );
        let idx = self.cassette.interactions.len();
        self.cassette.add(interaction);
        self.index.entry(key).or_default().push(idx);
    }
    
    /// Get the underlying cassette.
    pub fn cassette(&self) -> &Cassette {
        &self.cassette
    }
    
    /// Get the underlying cassette mutably.
    pub fn cassette_mut(&mut self) -> &mut Cassette {
        &mut self.cassette
    }
    
    /// Consume and return the underlying cassette.
    pub fn into_cassette(self) -> Cassette {
        self.cassette
    }
    
    /// Get the number of interactions.
    pub fn len(&self) -> usize {
        self.cassette.len()
    }
    
    /// Check if the cassette is empty.
    pub fn is_empty(&self) -> bool {
        self.cassette.is_empty()
    }
    
    /// Rebuild the index (call after modifying interactions directly).
    pub fn rebuild_index(&mut self) {
        self.index.clear();
        for (i, interaction) in self.cassette.interactions.iter().enumerate() {
            let key = (
                interaction.request.method.to_uppercase(),
                interaction.request.url.clone(),
            );
            self.index.entry(key).or_default().push(i);
        }
    }
}

impl From<Cassette> for IndexedCassette {
    fn from(cassette: Cassette) -> Self {
        Self::new(cassette)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cassette_new() {
        let cassette = Cassette::new();
        assert_eq!(cassette.version, 1);
        assert!(cassette.is_empty());
        assert!(cassette.recorded_with.starts_with("mockingbird/"));
    }

    #[test]
    fn test_cassette_add() {
        let mut cassette = Cassette::new();
        let req = RecordedRequest::new("GET", "https://example.com");
        let res = RecordedResponse::new(200);
        let interaction = Interaction::new(req, res);
        
        cassette.add(interaction);
        assert_eq!(cassette.len(), 1);
    }

    #[test]
    fn test_recorded_request_builder() {
        let req = RecordedRequest::new("POST", "https://api.example.com/users")
            .header("Content-Type", "application/json")
            .body(r#"{"name": "test"}"#);
        
        assert_eq!(req.method, "POST");
        assert_eq!(req.headers.len(), 1);
        assert!(req.body.is_some());
    }

    #[test]
    fn test_recorded_response_builder() {
        let res = RecordedResponse::new(201)
            .header("Content-Type", "application/json")
            .body(r#"{"id": 1}"#);
        
        assert_eq!(res.status, 201);
        assert_eq!(res.headers.len(), 1);
    }

    #[test]
    fn test_header() {
        let header = Header::new("Authorization", "Bearer token");
        assert_eq!(header.name, "Authorization");
        assert_eq!(header.value, "Bearer token");
    }

    #[test]
    fn test_body_encoding_default() {
        assert_eq!(BodyEncoding::default(), BodyEncoding::Text);
    }

    #[test]
    fn test_cassette_serialize() {
        let cassette = Cassette::new();
        let json = serde_json::to_string(&cassette).unwrap();
        assert!(json.contains("\"version\":1"));
    }

    #[test]
    fn test_indexed_cassette_new() {
        let cassette = Cassette::new();
        let indexed = IndexedCassette::new(cassette);
        assert!(indexed.is_empty());
    }

    #[test]
    fn test_indexed_cassette_find() {
        let mut cassette = Cassette::new();
        cassette.add(Interaction::new(
            RecordedRequest::new("GET", "https://example.com/a"),
            RecordedResponse::new(200),
        ));
        cassette.add(Interaction::new(
            RecordedRequest::new("POST", "https://example.com/a"),
            RecordedResponse::new(201),
        ));
        cassette.add(Interaction::new(
            RecordedRequest::new("GET", "https://example.com/b"),
            RecordedResponse::new(200),
        ));
        
        let indexed = IndexedCassette::new(cassette);
        
        // Find GET /a
        let matches = indexed.find_by_method_url("GET", "https://example.com/a");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], 0);
        
        // Find POST /a
        let matches = indexed.find_by_method_url("POST", "https://example.com/a");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], 1);
        
        // Find GET /b
        let matches = indexed.find_by_method_url("GET", "https://example.com/b");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], 2);
        
        // No match
        let matches = indexed.find_by_method_url("DELETE", "https://example.com/a");
        assert!(matches.is_empty());
    }

    #[test]
    fn test_indexed_cassette_add() {
        let cassette = Cassette::new();
        let mut indexed = IndexedCassette::new(cassette);
        
        indexed.add(Interaction::new(
            RecordedRequest::new("GET", "https://example.com/new"),
            RecordedResponse::new(200),
        ));
        
        assert_eq!(indexed.len(), 1);
        
        let matches = indexed.find_by_method_url("GET", "https://example.com/new");
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_indexed_cassette_multiple_same_url() {
        let mut cassette = Cassette::new();
        // Add multiple interactions with same method+url
        cassette.add(Interaction::new(
            RecordedRequest::new("GET", "https://example.com/api"),
            RecordedResponse::new(200),
        ));
        cassette.add(Interaction::new(
            RecordedRequest::new("GET", "https://example.com/api"),
            RecordedResponse::new(201),
        ));
        
        let indexed = IndexedCassette::new(cassette);
        let matches = indexed.find_by_method_url("GET", "https://example.com/api");
        
        // Both should be found
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0], 0);
        assert_eq!(matches[1], 1);
    }

    #[test]
    fn test_indexed_cassette_case_insensitive_method() {
        let mut cassette = Cassette::new();
        cassette.add(Interaction::new(
            RecordedRequest::new("get", "https://example.com/api"),
            RecordedResponse::new(200),
        ));
        
        let indexed = IndexedCassette::new(cassette);
        
        // Should find regardless of case
        let matches = indexed.find_by_method_url("GET", "https://example.com/api");
        assert_eq!(matches.len(), 1);
        
        let matches = indexed.find_by_method_url("get", "https://example.com/api");
        assert_eq!(matches.len(), 1);
    }
}
