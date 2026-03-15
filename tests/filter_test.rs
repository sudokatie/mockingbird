//! Sensitive data filtering tests.
//!
//! Tests for request and response filters.

use mockingbird::cassette::{RecordedRequest, RecordedResponse};
use mockingbird::filter::{
    RequestFilter, ResponseFilter,
    RemoveRequestHeader, ReplaceRequestHeader,
    RemoveResponseHeader, ReplaceResponseHeader,
    ReplaceRequestBody, ReplaceResponseBody,
    JsonPathRequestFilter, JsonPathResponseFilter,
};

#[test]
fn test_remove_request_header() {
    let filter = RemoveRequestHeader::new("Authorization");
    let mut req = RecordedRequest::new("GET", "http://example.com")
        .header("Authorization", "Bearer secret")
        .header("Content-Type", "application/json");
    
    filter.filter(&mut req);
    
    assert_eq!(req.headers.len(), 1);
    assert_eq!(req.headers[0].name, "Content-Type");
    assert_eq!(filter.name(), "remove_request_header");
}

#[test]
fn test_remove_request_header_case_insensitive() {
    let filter = RemoveRequestHeader::new("authorization");
    let mut req = RecordedRequest::new("GET", "http://example.com")
        .header("AUTHORIZATION", "Bearer secret")
        .header("Content-Type", "application/json");
    
    filter.filter(&mut req);
    
    assert_eq!(req.headers.len(), 1);
}

#[test]
fn test_replace_request_header() {
    let filter = ReplaceRequestHeader::new("Authorization", "[REDACTED]");
    let mut req = RecordedRequest::new("GET", "http://example.com")
        .header("Authorization", "Bearer secret");
    
    filter.filter(&mut req);
    
    assert_eq!(req.headers[0].value, "[REDACTED]");
    assert_eq!(filter.name(), "replace_request_header");
}

#[test]
fn test_replace_request_header_no_match() {
    let filter = ReplaceRequestHeader::new("X-Api-Key", "[REDACTED]");
    let mut req = RecordedRequest::new("GET", "http://example.com")
        .header("Authorization", "Bearer secret");
    
    filter.filter(&mut req);
    
    // Original value unchanged
    assert_eq!(req.headers[0].value, "Bearer secret");
}

#[test]
fn test_remove_response_header() {
    let filter = RemoveResponseHeader::new("Set-Cookie");
    let mut res = RecordedResponse::new(200)
        .header("Set-Cookie", "session=abc123")
        .header("Content-Type", "application/json");
    
    filter.filter(&mut res);
    
    assert_eq!(res.headers.len(), 1);
    assert_eq!(filter.name(), "remove_response_header");
}

#[test]
fn test_replace_response_header() {
    let filter = ReplaceResponseHeader::new("X-Auth-Token", "[REDACTED]");
    let mut res = RecordedResponse::new(200)
        .header("X-Auth-Token", "secret-token-123");
    
    filter.filter(&mut res);
    
    assert_eq!(res.headers[0].value, "[REDACTED]");
    assert_eq!(filter.name(), "replace_response_header");
}

#[test]
fn test_replace_request_body() {
    let filter = ReplaceRequestBody::new("secret_key_123", "[KEY]");
    let mut req = RecordedRequest::new("POST", "http://example.com")
        .body(r#"{"key": "secret_key_123"}"#);
    
    filter.filter(&mut req);
    
    assert_eq!(req.body.unwrap(), r#"{"key": "[KEY]"}"#);
    assert_eq!(filter.name(), "replace_request_body");
}

#[test]
fn test_replace_request_body_no_body() {
    let filter = ReplaceRequestBody::new("secret", "[REDACTED]");
    let mut req = RecordedRequest::new("GET", "http://example.com");
    
    // Should not panic on None body
    filter.filter(&mut req);
    
    assert!(req.body.is_none());
}

#[test]
fn test_replace_response_body() {
    let filter = ReplaceResponseBody::new("user@example.com", "[EMAIL]");
    let mut res = RecordedResponse::new(200)
        .body(r#"{"email": "user@example.com"}"#);
    
    filter.filter(&mut res);
    
    assert_eq!(res.body.unwrap(), r#"{"email": "[EMAIL]"}"#);
    assert_eq!(filter.name(), "replace_response_body");
}

#[test]
fn test_replace_response_body_multiple() {
    let filter = ReplaceResponseBody::new("secret", "[REDACTED]");
    let mut res = RecordedResponse::new(200)
        .body("first secret, second secret, third");
    
    filter.filter(&mut res);
    
    assert_eq!(res.body.unwrap(), "first [REDACTED], second [REDACTED], third");
}

#[test]
fn test_json_path_request_filter_simple() {
    let filter = JsonPathRequestFilter::new("$.password", "[FILTERED]");
    let mut req = RecordedRequest::new("POST", "http://example.com")
        .body(r#"{"username":"alice","password":"secret123"}"#);
    
    filter.filter(&mut req);
    
    let body = req.body.unwrap();
    assert!(body.contains("[FILTERED]"));
    assert!(!body.contains("secret123"));
    assert_eq!(filter.name(), "json_path_request_filter");
}

#[test]
fn test_json_path_request_filter_nested() {
    let filter = JsonPathRequestFilter::new("$.user.password", "[FILTERED]");
    let mut req = RecordedRequest::new("POST", "http://example.com")
        .body(r#"{"user":{"name":"alice","password":"secret"}}"#);
    
    filter.filter(&mut req);
    
    let body = req.body.unwrap();
    assert!(body.contains("[FILTERED]"));
    assert!(!body.contains("secret"));
}

#[test]
fn test_json_path_request_filter_missing_field() {
    let filter = JsonPathRequestFilter::new("$.nonexistent", "[FILTERED]");
    let mut req = RecordedRequest::new("POST", "http://example.com")
        .body(r#"{"existing":"value"}"#);
    
    filter.filter(&mut req);
    
    let body = req.body.unwrap();
    // Should not modify the body if field doesn't exist
    assert!(!body.contains("[FILTERED]"));
    assert!(body.contains("existing"));
}

#[test]
fn test_json_path_request_filter_non_json() {
    let filter = JsonPathRequestFilter::new("$.password", "[FILTERED]");
    let mut req = RecordedRequest::new("POST", "http://example.com")
        .body("not valid json");
    
    filter.filter(&mut req);
    
    // Should leave non-JSON body unchanged
    assert_eq!(req.body.unwrap(), "not valid json");
}

#[test]
fn test_json_path_request_filter_no_body() {
    let filter = JsonPathRequestFilter::new("$.password", "[FILTERED]");
    let mut req = RecordedRequest::new("GET", "http://example.com");
    
    // Should not panic on None body
    filter.filter(&mut req);
    
    assert!(req.body.is_none());
}

#[test]
fn test_json_path_response_filter() {
    let filter = JsonPathResponseFilter::new("$.api_key", "[REDACTED]");
    let mut res = RecordedResponse::new(200)
        .body(r#"{"api_key":"sk-12345","data":"public"}"#);
    
    filter.filter(&mut res);
    
    let body = res.body.unwrap();
    assert!(body.contains("[REDACTED]"));
    assert!(!body.contains("sk-12345"));
    assert_eq!(filter.name(), "json_path_response_filter");
}

#[test]
fn test_json_path_response_filter_deeply_nested() {
    let filter = JsonPathResponseFilter::new("$.config.auth.secret", "[REDACTED]");
    let mut res = RecordedResponse::new(200)
        .body(r#"{"config":{"auth":{"secret":"top-secret","user":"admin"}}}"#);
    
    filter.filter(&mut res);
    
    let body = res.body.unwrap();
    assert!(body.contains("[REDACTED]"));
    assert!(!body.contains("top-secret"));
    assert!(body.contains("admin")); // Other fields unchanged
}

#[test]
fn test_multiple_filters_chain() {
    let filter1 = ReplaceRequestHeader::new("Authorization", "[REDACTED]");
    let filter2 = JsonPathRequestFilter::new("$.password", "[FILTERED]");
    
    let mut req = RecordedRequest::new("POST", "http://example.com")
        .header("Authorization", "Bearer token")
        .body(r#"{"username":"alice","password":"secret"}"#);
    
    filter1.filter(&mut req);
    filter2.filter(&mut req);
    
    assert_eq!(req.headers[0].value, "[REDACTED]");
    assert!(req.body.as_ref().unwrap().contains("[FILTERED]"));
}

#[test]
fn test_replace_request_header_multiple() {
    let filter = ReplaceRequestHeader::new("x-api-key", "[REDACTED]");
    let mut req = RecordedRequest::new("GET", "http://example.com")
        .header("X-Api-Key", "key1")
        .header("Content-Type", "application/json")
        .header("X-Api-Key", "key2"); // Duplicate header
    
    filter.filter(&mut req);
    
    // Both occurrences should be redacted
    for header in &req.headers {
        if header.name.to_lowercase() == "x-api-key" {
            assert_eq!(header.value, "[REDACTED]");
        }
    }
}
