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
    
    /// Get the status code as u16.
    pub fn status(&self) -> u16 {
        self.status
    }
    
    /// Get the status code as reqwest::StatusCode.
    pub fn status_code(&self) -> reqwest::StatusCode {
        reqwest::StatusCode::from_u16(self.status).unwrap_or(reqwest::StatusCode::OK)
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
    
    /// Apply compression based on Accept-Encoding header.
    /// 
    /// If the accept_encoding contains a supported encoding (gzip, deflate),
    /// compresses the body and sets the Content-Encoding header.
    /// Returns self unchanged if no supported encoding is requested.
    pub fn with_compression(mut self, accept_encoding: Option<&str>) -> Self {
        let Some(accept) = accept_encoding else {
            return self;
        };
        
        // Skip if body is empty or already compressed
        if self.body.is_empty() || self.headers.contains_key("content-encoding") {
            return self;
        }
        
        // Parse Accept-Encoding and find best match
        // Supports: gzip, deflate (not brotli - would need another dep)
        let accept_lower = accept.to_lowercase();
        
        if accept_lower.contains("gzip") {
            if let Some(compressed) = self.compress_gzip() {
                self.body = compressed;
                self.headers.insert("content-encoding".to_string(), "gzip".to_string());
            }
        } else if accept_lower.contains("deflate") {
            if let Some(compressed) = self.compress_deflate() {
                self.body = compressed;
                self.headers.insert("content-encoding".to_string(), "deflate".to_string());
            }
        }
        
        self
    }
    
    /// Compress body with gzip.
    fn compress_gzip(&self) -> Option<Bytes> {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;
        
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&self.body).ok()?;
        let compressed = encoder.finish().ok()?;
        Some(Bytes::from(compressed))
    }
    
    /// Compress body with deflate.
    fn compress_deflate(&self) -> Option<Bytes> {
        use flate2::write::DeflateEncoder;
        use flate2::Compression;
        use std::io::Write;
        
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&self.body).ok()?;
        let compressed = encoder.finish().ok()?;
        Some(Bytes::from(compressed))
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
    fn test_status_code() {
        let resp = Response::new(201, "Created");
        assert_eq!(resp.status_code(), reqwest::StatusCode::CREATED);
        
        let resp2 = Response::new(404, "Not Found");
        assert_eq!(resp2.status_code(), reqwest::StatusCode::NOT_FOUND);
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
    
    #[test]
    fn test_with_compression_gzip() {
        use flate2::read::GzDecoder;
        use std::io::Read;
        
        let resp = Response::new(200, "Hello, World!");
        let compressed = resp.with_compression(Some("gzip, deflate"));
        
        // Should have content-encoding header
        assert_eq!(compressed.header("content-encoding"), Some("gzip"));
        
        // Body should be compressed (different from original)
        let compressed_bytes = compressed.bytes();
        assert_ne!(compressed_bytes, Bytes::from("Hello, World!"));
        
        // Should decompress to original
        let mut decoder = GzDecoder::new(compressed_bytes.as_ref());
        let mut decompressed = String::new();
        decoder.read_to_string(&mut decompressed).unwrap();
        assert_eq!(decompressed, "Hello, World!");
    }
    
    #[test]
    fn test_with_compression_deflate() {
        use flate2::read::DeflateDecoder;
        use std::io::Read;
        
        let resp = Response::new(200, "Hello, Deflate!");
        let compressed = resp.with_compression(Some("deflate"));
        
        assert_eq!(compressed.header("content-encoding"), Some("deflate"));
        
        let compressed_bytes = compressed.bytes();
        let mut decoder = DeflateDecoder::new(compressed_bytes.as_ref());
        let mut decompressed = String::new();
        decoder.read_to_string(&mut decompressed).unwrap();
        assert_eq!(decompressed, "Hello, Deflate!");
    }
    
    #[test]
    fn test_with_compression_none() {
        let resp = Response::new(200, "No compression");
        let same = resp.clone().with_compression(None);
        
        assert!(same.header("content-encoding").is_none());
        assert_eq!(same.bytes(), Bytes::from("No compression"));
    }
    
    #[test]
    fn test_with_compression_unsupported() {
        let resp = Response::new(200, "Brotli not supported");
        let same = resp.clone().with_compression(Some("br"));
        
        // Should not compress with unsupported encoding
        assert!(same.header("content-encoding").is_none());
        assert_eq!(same.bytes(), Bytes::from("Brotli not supported"));
    }
    
    #[test]
    fn test_with_compression_empty_body() {
        let resp = Response::new(204, Bytes::new());
        let same = resp.with_compression(Some("gzip"));
        
        // Should not compress empty body
        assert!(same.header("content-encoding").is_none());
        assert!(same.bytes().is_empty());
    }
}
