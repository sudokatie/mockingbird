//! Response wrapper.
//!
//! Provides a reqwest-compatible Response type that can be created from
//! recorded responses or captured from real responses.

use crate::cassette::{BodyEncoding, Header, RecordedResponse};
use crate::error::{Error, Result};
use bytes::Bytes;
use serde::de::DeserializeOwned;
use std::collections::HashMap;

/// HTTP response that can be created from recorded data or real responses.
#[derive(Debug, Clone)]
pub struct Response {
    status: u16,
    headers: HashMap<String, String>,
    body: Bytes,
}

impl Response {
    /// Create a response from recorded data.
    pub fn from_recorded(recorded: RecordedResponse) -> Self {
        let body = match (&recorded.body, recorded.body_encoding) {
            (Some(text), BodyEncoding::Text) => Bytes::from(text.clone()),
            (Some(b64), BodyEncoding::Base64) => {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .map(Bytes::from)
                    .unwrap_or_default()
            }
            (None, _) => Bytes::new(),
        };
        
        let headers = recorded
            .headers
            .into_iter()
            .map(|h| (h.name.to_lowercase(), h.value))
            .collect();
        
        Self {
            status: recorded.status,
            headers,
            body,
        }
    }
    
    /// Create a new response with status and body.
    pub fn new(status: u16, body: impl Into<Bytes>) -> Self {
        Self {
            status,
            headers: HashMap::new(),
            body: body.into(),
        }
    }
    
    /// Get the status code.
    pub fn status(&self) -> u16 {
        self.status
    }
    
    /// Check if status is success (2xx).
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
    
    /// Get a header value.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(&name.to_lowercase()).map(|s| s.as_str())
    }
    
    /// Get all headers.
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }
    
    /// Get the body as text.
    pub fn text(&self) -> Result<String> {
        String::from_utf8(self.body.to_vec())
            .map_err(|e| Error::InvalidFormat(format!("Invalid UTF-8: {}", e)))
    }
    
    /// Get the body as bytes.
    pub fn bytes(&self) -> Bytes {
        self.body.clone()
    }
    
    /// Parse the body as JSON.
    pub fn json<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_slice(&self.body).map_err(Error::from)
    }
    
    /// Convert to a RecordedResponse for storage.
    pub fn to_recorded(&self) -> RecordedResponse {
        let (body, encoding) = if self.body.is_empty() {
            (None, BodyEncoding::Text)
        } else if let Ok(text) = String::from_utf8(self.body.to_vec()) {
            (Some(text), BodyEncoding::Text)
        } else {
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&self.body);
            (Some(b64), BodyEncoding::Base64)
        };
        
        RecordedResponse {
            status: self.status,
            headers: self
                .headers
                .iter()
                .map(|(k, v)| Header::new(k, v))
                .collect(),
            body,
            body_encoding: encoding,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json_response() -> RecordedResponse {
        RecordedResponse::new(200)
            .header("Content-Type", "application/json")
            .body(r#"{"value":42}"#)
    }

    #[test]
    fn test_from_recorded() {
        let recorded = json_response();
        let resp = Response::from_recorded(recorded);
        
        assert_eq!(resp.status(), 200);
        assert!(resp.is_success());
    }

    #[test]
    fn test_status() {
        let resp = Response::new(404, "Not Found");
        assert_eq!(resp.status(), 404);
        assert!(!resp.is_success());
    }

    #[test]
    fn test_header() {
        let recorded = json_response();
        let resp = Response::from_recorded(recorded);
        
        assert_eq!(resp.header("content-type"), Some("application/json"));
        assert_eq!(resp.header("Content-Type"), Some("application/json"));
        assert!(resp.header("X-Missing").is_none());
    }

    #[test]
    fn test_text() {
        let recorded = RecordedResponse::new(200).body("Hello, World!");
        let resp = Response::from_recorded(recorded);
        
        assert_eq!(resp.text().unwrap(), "Hello, World!");
    }

    #[test]
    fn test_bytes() {
        let recorded = RecordedResponse::new(200).body("Hello");
        let resp = Response::from_recorded(recorded);
        
        assert_eq!(resp.bytes(), Bytes::from("Hello"));
    }

    #[test]
    fn test_json() {
        let recorded = json_response();
        let resp = Response::from_recorded(recorded);
        
        #[derive(serde::Deserialize)]
        struct Data {
            value: i32,
        }
        
        let data: Data = resp.json().unwrap();
        assert_eq!(data.value, 42);
    }

    #[test]
    fn test_to_recorded() {
        let resp = Response::new(201, r#"{"created":true}"#);
        let recorded = resp.to_recorded();
        
        assert_eq!(recorded.status, 201);
        assert_eq!(recorded.body, Some(r#"{"created":true}"#.to_string()));
    }

    #[test]
    fn test_binary_body() {
        // Binary data that's not valid UTF-8
        let binary = vec![0x00, 0x01, 0x02, 0xFF];
        let resp = Response::new(200, binary.clone());
        
        let recorded = resp.to_recorded();
        assert_eq!(recorded.body_encoding, BodyEncoding::Base64);
        
        // Round-trip
        let resp2 = Response::from_recorded(recorded);
        assert_eq!(resp2.bytes().to_vec(), binary);
    }

    #[test]
    fn test_empty_body() {
        let recorded = RecordedResponse::new(204);
        let resp = Response::from_recorded(recorded);
        
        assert!(resp.bytes().is_empty());
        assert_eq!(resp.text().unwrap(), "");
    }
}
