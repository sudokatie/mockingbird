//! Request and response filters.
//!
//! Filters modify requests/responses before recording or after replay.
//! Common use: sanitize sensitive data.

use crate::cassette::{RecordedRequest, RecordedResponse};

/// Filter for modifying requests before recording.
pub trait RequestFilter: Send + Sync {
    /// Apply the filter to a request.
    fn filter(&self, request: &mut RecordedRequest);
    
    /// Get the name of this filter.
    fn name(&self) -> &'static str;
}

/// Filter for modifying responses before recording.
pub trait ResponseFilter: Send + Sync {
    /// Apply the filter to a response.
    fn filter(&self, response: &mut RecordedResponse);
    
    /// Get the name of this filter.
    fn name(&self) -> &'static str;
}

/// Remove a header from requests.
#[derive(Debug)]
pub struct RemoveRequestHeader {
    header_name: String,
}

impl RemoveRequestHeader {
    /// Create a new header removal filter.
    pub fn new(header_name: impl Into<String>) -> Self {
        Self {
            header_name: header_name.into().to_lowercase(),
        }
    }
}

impl RequestFilter for RemoveRequestHeader {
    fn filter(&self, request: &mut RecordedRequest) {
        request.headers.retain(|h| h.name.to_lowercase() != self.header_name);
    }
    
    fn name(&self) -> &'static str {
        "remove_request_header"
    }
}

/// Replace a header value in requests.
#[derive(Debug)]
pub struct ReplaceRequestHeader {
    header_name: String,
    replacement: String,
}

impl ReplaceRequestHeader {
    /// Create a new header replacement filter.
    pub fn new(header_name: impl Into<String>, replacement: impl Into<String>) -> Self {
        Self {
            header_name: header_name.into().to_lowercase(),
            replacement: replacement.into(),
        }
    }
}

impl RequestFilter for ReplaceRequestHeader {
    fn filter(&self, request: &mut RecordedRequest) {
        for header in &mut request.headers {
            if header.name.to_lowercase() == self.header_name {
                header.value = self.replacement.clone();
            }
        }
    }
    
    fn name(&self) -> &'static str {
        "replace_request_header"
    }
}

/// Remove a header from responses.
#[derive(Debug)]
pub struct RemoveResponseHeader {
    header_name: String,
}

impl RemoveResponseHeader {
    /// Create a new header removal filter.
    pub fn new(header_name: impl Into<String>) -> Self {
        Self {
            header_name: header_name.into().to_lowercase(),
        }
    }
}

impl ResponseFilter for RemoveResponseHeader {
    fn filter(&self, response: &mut RecordedResponse) {
        response.headers.retain(|h| h.name.to_lowercase() != self.header_name);
    }
    
    fn name(&self) -> &'static str {
        "remove_response_header"
    }
}

/// Replace text in request body.
#[derive(Debug)]
pub struct ReplaceRequestBody {
    pattern: String,
    replacement: String,
}

impl ReplaceRequestBody {
    /// Create a new body replacement filter.
    pub fn new(pattern: impl Into<String>, replacement: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            replacement: replacement.into(),
        }
    }
}

impl RequestFilter for ReplaceRequestBody {
    fn filter(&self, request: &mut RecordedRequest) {
        if let Some(body) = &mut request.body {
            *body = body.replace(&self.pattern, &self.replacement);
        }
    }
    
    fn name(&self) -> &'static str {
        "replace_request_body"
    }
}

/// Replace text in response body.
#[derive(Debug)]
pub struct ReplaceResponseBody {
    pattern: String,
    replacement: String,
}

impl ReplaceResponseBody {
    /// Create a new body replacement filter.
    pub fn new(pattern: impl Into<String>, replacement: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            replacement: replacement.into(),
        }
    }
}

impl ResponseFilter for ReplaceResponseBody {
    fn filter(&self, response: &mut RecordedResponse) {
        if let Some(body) = &mut response.body {
            *body = body.replace(&self.pattern, &self.replacement);
        }
    }
    
    fn name(&self) -> &'static str {
        "replace_response_body"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cassette::Header;

    #[test]
    fn test_remove_request_header() {
        let filter = RemoveRequestHeader::new("Authorization");
        let mut req = RecordedRequest::new("GET", "http://example.com")
            .header("Authorization", "Bearer secret")
            .header("Content-Type", "application/json");
        
        filter.filter(&mut req);
        
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Content-Type");
    }

    #[test]
    fn test_replace_request_header() {
        let filter = ReplaceRequestHeader::new("Authorization", "[REDACTED]");
        let mut req = RecordedRequest::new("GET", "http://example.com")
            .header("Authorization", "Bearer secret");
        
        filter.filter(&mut req);
        
        assert_eq!(req.headers[0].value, "[REDACTED]");
    }

    #[test]
    fn test_remove_response_header() {
        let filter = RemoveResponseHeader::new("Set-Cookie");
        let mut res = RecordedResponse::new(200)
            .header("Set-Cookie", "session=abc123")
            .header("Content-Type", "application/json");
        
        filter.filter(&mut res);
        
        assert_eq!(res.headers.len(), 1);
    }

    #[test]
    fn test_replace_request_body() {
        let filter = ReplaceRequestBody::new("secret_key_123", "[KEY]");
        let mut req = RecordedRequest::new("POST", "http://example.com")
            .body(r#"{"key": "secret_key_123"}"#);
        
        filter.filter(&mut req);
        
        assert_eq!(req.body.unwrap(), r#"{"key": "[KEY]"}"#);
    }

    #[test]
    fn test_replace_response_body() {
        let filter = ReplaceResponseBody::new("user@example.com", "[EMAIL]");
        let mut res = RecordedResponse::new(200)
            .body(r#"{"email": "user@example.com"}"#);
        
        filter.filter(&mut res);
        
        assert_eq!(res.body.unwrap(), r#"{"email": "[EMAIL]"}"#);
    }
}
