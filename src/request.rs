//! Request builder.
//!
//! Fluent API for constructing HTTP requests.

use crate::cassette::{BodyEncoding, Header, RecordedRequest};
use bytes::Bytes;
use std::collections::HashMap;

/// HTTP request builder.
#[derive(Debug, Clone)]
pub struct Request {
    method: String,
    url: String,
    headers: HashMap<String, String>,
    body: Option<Bytes>,
}

impl Request {
    /// Create a new GET request.
    pub fn get(url: impl Into<String>) -> Self {
        Self::new("GET", url)
    }
    
    /// Create a new POST request.
    pub fn post(url: impl Into<String>) -> Self {
        Self::new("POST", url)
    }
    
    /// Create a new PUT request.
    pub fn put(url: impl Into<String>) -> Self {
        Self::new("PUT", url)
    }
    
    /// Create a new DELETE request.
    pub fn delete(url: impl Into<String>) -> Self {
        Self::new("DELETE", url)
    }
    
    /// Create a new PATCH request.
    pub fn patch(url: impl Into<String>) -> Self {
        Self::new("PATCH", url)
    }
    
    /// Create a new request with any method.
    pub fn new(method: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            method: method.into().to_uppercase(),
            url: url.into(),
            headers: HashMap::new(),
            body: None,
        }
    }
    
    /// Add a header.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(name.into().to_lowercase(), value.into());
        self
    }
    
    /// Set the body as text.
    pub fn body(mut self, body: impl Into<Bytes>) -> Self {
        self.body = Some(body.into());
        self
    }
    
    /// Set the body as JSON.
    pub fn json<T: serde::Serialize>(mut self, value: &T) -> Self {
        if let Ok(json) = serde_json::to_vec(value) {
            self.body = Some(Bytes::from(json));
            self.headers.insert("content-type".to_string(), "application/json".to_string());
        }
        self
    }
    
    /// Get the HTTP method.
    pub fn method(&self) -> &str {
        &self.method
    }
    
    /// Get the URL.
    pub fn url(&self) -> &str {
        &self.url
    }
    
    /// Get a header value.
    pub fn get_header(&self, name: &str) -> Option<&str> {
        self.headers.get(&name.to_lowercase()).map(|s| s.as_str())
    }
    
    /// Get all headers.
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }
    
    /// Get the body.
    pub fn get_body(&self) -> Option<&Bytes> {
        self.body.as_ref()
    }
    
    /// Convert to a RecordedRequest for matching/storage.
    pub fn to_recorded(&self) -> RecordedRequest {
        let (body, encoding) = match &self.body {
            None => (None, BodyEncoding::Text),
            Some(bytes) => {
                if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                    (Some(text), BodyEncoding::Text)
                } else {
                    use base64::Engine;
                    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
                    (Some(b64), BodyEncoding::Base64)
                }
            }
        };
        
        let mut req = RecordedRequest::new(&self.method, &self.url);
        req.headers = self
            .headers
            .iter()
            .map(|(k, v)| Header::new(k, v))
            .collect();
        req.body = body;
        req.body_encoding = encoding;
        req
    }
    
    /// Create from a RecordedRequest.
    pub fn from_recorded(recorded: &RecordedRequest) -> Self {
        let body = match (&recorded.body, recorded.body_encoding) {
            (Some(text), BodyEncoding::Text) => Some(Bytes::from(text.clone())),
            (Some(b64), BodyEncoding::Base64) => {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .ok()
                    .map(Bytes::from)
            }
            (None, _) => None,
        };
        
        let headers = recorded
            .headers
            .iter()
            .map(|h| (h.name.to_lowercase(), h.value.clone()))
            .collect();
        
        Self {
            method: recorded.method.clone(),
            url: recorded.url.clone(),
            headers,
            body,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get() {
        let req = Request::get("https://api.example.com/users");
        assert_eq!(req.method(), "GET");
        assert_eq!(req.url(), "https://api.example.com/users");
    }

    #[test]
    fn test_post() {
        let req = Request::post("https://api.example.com/users");
        assert_eq!(req.method(), "POST");
    }

    #[test]
    fn test_header() {
        let req = Request::get("https://example.com")
            .header("Authorization", "Bearer token")
            .header("Accept", "application/json");
        
        assert_eq!(req.get_header("authorization"), Some("Bearer token"));
        assert_eq!(req.get_header("accept"), Some("application/json"));
    }

    #[test]
    fn test_body() {
        let req = Request::post("https://example.com")
            .body("Hello, World!");
        
        assert_eq!(req.get_body().map(|b| b.as_ref()), Some(b"Hello, World!".as_slice()));
    }

    #[test]
    fn test_json() {
        #[derive(serde::Serialize)]
        struct User {
            name: String,
        }
        
        let user = User {
            name: "Alice".to_string(),
        };
        
        let req = Request::post("https://example.com").json(&user);
        
        assert_eq!(req.get_header("content-type"), Some("application/json"));
        let body = req.get_body().unwrap();
        assert!(String::from_utf8_lossy(body).contains("Alice"));
    }

    #[test]
    fn test_to_recorded() {
        let req = Request::post("https://example.com")
            .header("Content-Type", "text/plain")
            .body("test body");
        
        let recorded = req.to_recorded();
        assert_eq!(recorded.method, "POST");
        assert_eq!(recorded.url, "https://example.com");
        assert_eq!(recorded.body, Some("test body".to_string()));
    }

    #[test]
    fn test_from_recorded() {
        let recorded = RecordedRequest::new("PUT", "https://api.example.com")
            .header("X-Custom", "value")
            .body("content");
        
        let req = Request::from_recorded(&recorded);
        assert_eq!(req.method(), "PUT");
        assert_eq!(req.url(), "https://api.example.com");
        assert_eq!(req.get_header("x-custom"), Some("value"));
    }

    #[test]
    fn test_method_case() {
        let req = Request::new("get", "https://example.com");
        assert_eq!(req.method(), "GET");
    }

    #[test]
    fn test_all_methods() {
        assert_eq!(Request::get("u").method(), "GET");
        assert_eq!(Request::post("u").method(), "POST");
        assert_eq!(Request::put("u").method(), "PUT");
        assert_eq!(Request::delete("u").method(), "DELETE");
        assert_eq!(Request::patch("u").method(), "PATCH");
    }
}
