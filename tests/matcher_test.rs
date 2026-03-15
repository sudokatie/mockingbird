//! Matching strategy tests.
//!
//! Tests for all built-in matchers and custom matching.

use mockingbird::cassette::RecordedRequest;
use mockingbird::matcher::{
    Matcher, MethodMatcher, UrlMatcher, UrlMethodMatcher, PathMatcher,
    BodyMatcher, HeaderMatcher, ExactMatcher, AllMatcher,
    NormalizedUrlMethodMatcher, NormalizedJsonBodyMatcher, CustomMatcher,
};

fn req(method: &str, url: &str) -> RecordedRequest {
    RecordedRequest::new(method, url)
}

#[test]
fn test_method_matcher() {
    let matcher = MethodMatcher;
    
    // Same method, different URL - should match
    assert!(matcher.matches(&req("GET", "http://a.com"), &req("GET", "http://b.com")));
    
    // Case insensitive
    assert!(matcher.matches(&req("GET", "http://a.com"), &req("get", "http://b.com")));
    
    // Different methods - should not match
    assert!(!matcher.matches(&req("GET", "http://a.com"), &req("POST", "http://a.com")));
    
    assert_eq!(matcher.name(), "method");
}

#[test]
fn test_url_matcher() {
    let matcher = UrlMatcher;
    
    // Same URL
    assert!(matcher.matches(&req("GET", "http://a.com"), &req("GET", "http://a.com")));
    
    // Different URL
    assert!(!matcher.matches(&req("GET", "http://a.com"), &req("GET", "http://b.com")));
    
    // Method doesn't matter
    assert!(matcher.matches(&req("GET", "http://a.com"), &req("POST", "http://a.com")));
    
    assert_eq!(matcher.name(), "url");
}

#[test]
fn test_url_method_matcher() {
    let matcher = UrlMethodMatcher;
    
    // Same method and URL
    assert!(matcher.matches(&req("GET", "http://a.com"), &req("GET", "http://a.com")));
    
    // Case insensitive method
    assert!(matcher.matches(&req("GET", "http://a.com"), &req("get", "http://a.com")));
    
    // Different method
    assert!(!matcher.matches(&req("GET", "http://a.com"), &req("POST", "http://a.com")));
    
    // Different URL
    assert!(!matcher.matches(&req("GET", "http://a.com"), &req("GET", "http://b.com")));
    
    assert_eq!(matcher.name(), "url_method");
}

#[test]
fn test_path_matcher() {
    let matcher = PathMatcher;
    
    // Same path, different host
    assert!(matcher.matches(
        &req("GET", "http://a.com/path"),
        &req("GET", "http://b.com/path")
    ));
    
    // Same path, different query
    assert!(matcher.matches(
        &req("GET", "http://a.com/path?a=1"),
        &req("GET", "http://a.com/path?b=2")
    ));
    
    // Different path
    assert!(!matcher.matches(
        &req("GET", "http://a.com/path1"),
        &req("GET", "http://a.com/path2")
    ));
    
    assert_eq!(matcher.name(), "path");
}

#[test]
fn test_body_matcher() {
    let matcher = BodyMatcher;
    
    let mut r1 = req("POST", "http://a.com");
    r1.body = Some("hello".to_string());
    let mut r2 = req("POST", "http://a.com");
    r2.body = Some("hello".to_string());
    let mut r3 = req("POST", "http://a.com");
    r3.body = Some("world".to_string());
    let r4 = req("POST", "http://a.com"); // No body
    
    // Same body
    assert!(matcher.matches(&r1, &r2));
    
    // Different body
    assert!(!matcher.matches(&r1, &r3));
    
    // Both no body
    assert!(matcher.matches(&r4, &req("POST", "http://a.com")));
    
    // One with body, one without
    assert!(!matcher.matches(&r1, &r4));
    
    assert_eq!(matcher.name(), "body");
}

#[test]
fn test_header_matcher() {
    let matcher = HeaderMatcher::new("content-type");
    
    let r1 = req("POST", "http://a.com")
        .header("Content-Type", "application/json");
    let r2 = req("POST", "http://a.com")
        .header("content-type", "application/json");
    let r3 = req("POST", "http://a.com")
        .header("Content-Type", "text/plain");
    let r4 = req("POST", "http://a.com"); // No header
    
    // Same header value (case insensitive header name)
    assert!(matcher.matches(&r1, &r2));
    
    // Different value
    assert!(!matcher.matches(&r1, &r3));
    
    // Both missing header
    assert!(matcher.matches(&r4, &req("POST", "http://a.com")));
    
    assert_eq!(matcher.name(), "header");
}

#[test]
fn test_exact_matcher_basic() {
    let matcher = ExactMatcher;
    
    let r1 = req("GET", "http://a.com");
    let r2 = req("GET", "http://a.com");
    
    assert!(matcher.matches(&r1, &r2));
    assert_eq!(matcher.name(), "exact");
}

#[test]
fn test_exact_matcher_headers() {
    let matcher = ExactMatcher;
    
    let r1 = req("GET", "http://a.com")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json");
    let r2 = req("GET", "http://a.com")
        .header("Accept", "application/json")
        .header("Content-Type", "application/json");
    let r3 = req("GET", "http://a.com")
        .header("Content-Type", "application/json");
    
    // Same headers, different order - should match
    assert!(matcher.matches(&r1, &r2));
    
    // Missing header - should not match
    assert!(!matcher.matches(&r1, &r3));
}

#[test]
fn test_exact_matcher_body() {
    let matcher = ExactMatcher;
    
    let mut r1 = req("POST", "http://a.com");
    r1.body = Some("hello".to_string());
    let mut r2 = req("POST", "http://a.com");
    r2.body = Some("hello".to_string());
    let mut r3 = req("POST", "http://a.com");
    r3.body = Some("world".to_string());
    
    assert!(matcher.matches(&r1, &r2));
    assert!(!matcher.matches(&r1, &r3));
}

#[test]
fn test_all_matcher_default() {
    let matcher = AllMatcher::default_matchers();
    
    assert!(matcher.matches(&req("GET", "http://a.com"), &req("GET", "http://a.com")));
    assert!(!matcher.matches(&req("GET", "http://a.com"), &req("POST", "http://a.com")));
    assert!(!matcher.matches(&req("GET", "http://a.com"), &req("GET", "http://b.com")));
    
    assert_eq!(matcher.name(), "all");
}

#[test]
fn test_all_matcher_custom() {
    let matcher = AllMatcher::new()
        .add(MethodMatcher)
        .add(PathMatcher);
    
    // Same method and path (different host) - should match
    assert!(matcher.matches(
        &req("GET", "http://a.com/api"),
        &req("GET", "http://b.com/api")
    ));
    
    // Different method - should not match
    assert!(!matcher.matches(
        &req("GET", "http://a.com/api"),
        &req("POST", "http://a.com/api")
    ));
}

#[test]
fn test_normalized_url_method_matcher() {
    let matcher = NormalizedUrlMethodMatcher;
    
    // Different query param order should match
    assert!(matcher.matches(
        &req("GET", "http://example.com?b=2&a=1"),
        &req("GET", "http://example.com?a=1&b=2")
    ));
    
    // Same URL should match
    assert!(matcher.matches(
        &req("GET", "http://example.com?a=1"),
        &req("GET", "http://example.com?a=1")
    ));
    
    // Different method should not match
    assert!(!matcher.matches(
        &req("GET", "http://example.com?a=1"),
        &req("POST", "http://example.com?a=1")
    ));
    
    // Different values should not match
    assert!(!matcher.matches(
        &req("GET", "http://example.com?a=1"),
        &req("GET", "http://example.com?a=2")
    ));
    
    assert_eq!(matcher.name(), "normalized_url_method");
}

#[test]
fn test_normalized_json_body_matcher() {
    let matcher = NormalizedJsonBodyMatcher;
    
    // Same JSON, different key order
    let mut r1 = req("POST", "http://a.com");
    r1.body = Some(r#"{"b":2,"a":1}"#.to_string());
    let mut r2 = req("POST", "http://a.com");
    r2.body = Some(r#"{"a":1,"b":2}"#.to_string());
    assert!(matcher.matches(&r1, &r2));
    
    // Same JSON, different whitespace
    let mut r3 = req("POST", "http://a.com");
    r3.body = Some(r#"{"a": 1, "b": 2}"#.to_string());
    let mut r4 = req("POST", "http://a.com");
    r4.body = Some(r#"{"a":1,"b":2}"#.to_string());
    assert!(matcher.matches(&r3, &r4));
    
    // Different values should not match
    let mut r5 = req("POST", "http://a.com");
    r5.body = Some(r#"{"a":1}"#.to_string());
    let mut r6 = req("POST", "http://a.com");
    r6.body = Some(r#"{"a":2}"#.to_string());
    assert!(!matcher.matches(&r5, &r6));
    
    // Non-JSON falls back to exact match
    let mut r7 = req("POST", "http://a.com");
    r7.body = Some("hello world".to_string());
    let mut r8 = req("POST", "http://a.com");
    r8.body = Some("hello world".to_string());
    assert!(matcher.matches(&r7, &r8));
    
    // Both None should match
    let r9 = req("POST", "http://a.com");
    let r10 = req("POST", "http://a.com");
    assert!(matcher.matches(&r9, &r10));
    
    // One None, one Some should not match
    let r11 = req("POST", "http://a.com");
    let mut r12 = req("POST", "http://a.com");
    r12.body = Some("{}".to_string());
    assert!(!matcher.matches(&r11, &r12));
    
    assert_eq!(matcher.name(), "normalized_json_body");
}

#[test]
fn test_custom_matcher() {
    // Custom matcher that only checks the URL without query string
    let matcher = CustomMatcher::new(|recorded, incoming| {
        recorded.url.split('?').next() == incoming.url.split('?').next()
    });
    
    // Same base URL, different query strings - should match
    assert!(matcher.matches(
        &req("GET", "http://a.com/path?a=1"),
        &req("POST", "http://a.com/path?b=2")
    ));
    
    // Different base URLs - should not match
    assert!(!matcher.matches(
        &req("GET", "http://a.com/path1"),
        &req("GET", "http://a.com/path2")
    ));
    
    assert_eq!(matcher.name(), "custom");
}

#[test]
fn test_custom_matcher_closure() {
    // Match by request body containing a specific string
    let matcher = CustomMatcher::new(|recorded, incoming| {
        match (&recorded.body, &incoming.body) {
            (Some(r), Some(i)) => r.contains("test") == i.contains("test"),
            (None, None) => true,
            _ => false,
        }
    });
    
    let mut r1 = req("POST", "http://a.com");
    r1.body = Some("this is a test".to_string());
    let mut r2 = req("POST", "http://a.com");
    r2.body = Some("another test here".to_string());
    
    assert!(matcher.matches(&r1, &r2));
}
