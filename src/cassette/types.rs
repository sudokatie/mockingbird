//! Cassette data types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A cassette containing recorded HTTP interactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cassette {
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

impl Cassette {
    /// Create a new empty cassette.
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            version: 1,
            created_at: now,
            modified_at: now,
            interactions: Vec::new(),
            metadata: HashMap::new(),
        }
    }
    
    /// Add an interaction to the cassette.
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

/// A single HTTP interaction (request + response).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interaction {
    /// The recorded request.
    pub request: RecordedRequest,
    
    /// The recorded response.
    pub response: RecordedResponse,
    
    /// When this interaction was recorded.
    pub recorded_at: DateTime<Utc>,
}

impl Interaction {
    /// Create a new interaction.
    pub fn new(request: RecordedRequest, response: RecordedResponse) -> Self {
        Self {
            request,
            response,
            recorded_at: Utc::now(),
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cassette_new() {
        let cassette = Cassette::new();
        assert_eq!(cassette.version, 1);
        assert!(cassette.is_empty());
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
}
