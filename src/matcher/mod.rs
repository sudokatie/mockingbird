//! Request matching.
//!
//! Matchers determine if a recorded request matches an incoming request.

use crate::cassette::RecordedRequest;

/// Trait for request matching.
pub trait Matcher: Send + Sync {
    /// Check if the recorded request matches the incoming request.
    fn matches(&self, recorded: &RecordedRequest, incoming: &RecordedRequest) -> bool;
    
    /// Get the name of this matcher.
    fn name(&self) -> &'static str;
}

/// Matches requests by HTTP method.
#[derive(Debug, Default)]
pub struct MethodMatcher;

impl Matcher for MethodMatcher {
    fn matches(&self, recorded: &RecordedRequest, incoming: &RecordedRequest) -> bool {
        recorded.method.eq_ignore_ascii_case(&incoming.method)
    }
    
    fn name(&self) -> &'static str {
        "method"
    }
}

/// Matches requests by exact URL.
#[derive(Debug, Default)]
pub struct UrlMatcher;

impl Matcher for UrlMatcher {
    fn matches(&self, recorded: &RecordedRequest, incoming: &RecordedRequest) -> bool {
        recorded.url == incoming.url
    }
    
    fn name(&self) -> &'static str {
        "url"
    }
}

/// Matches requests by URL path only (ignores query string).
#[derive(Debug, Default)]
pub struct PathMatcher;

impl Matcher for PathMatcher {
    fn matches(&self, recorded: &RecordedRequest, incoming: &RecordedRequest) -> bool {
        let recorded_path = extract_path(&recorded.url);
        let incoming_path = extract_path(&incoming.url);
        recorded_path == incoming_path
    }
    
    fn name(&self) -> &'static str {
        "path"
    }
}

/// Matches requests by body content.
#[derive(Debug, Default)]
pub struct BodyMatcher;

impl Matcher for BodyMatcher {
    fn matches(&self, recorded: &RecordedRequest, incoming: &RecordedRequest) -> bool {
        recorded.body == incoming.body
    }
    
    fn name(&self) -> &'static str {
        "body"
    }
}

/// Matches requests by specific header.
#[derive(Debug)]
pub struct HeaderMatcher {
    header_name: String,
}

impl HeaderMatcher {
    /// Create a new header matcher.
    pub fn new(header_name: impl Into<String>) -> Self {
        Self {
            header_name: header_name.into().to_lowercase(),
        }
    }
}

impl Matcher for HeaderMatcher {
    fn matches(&self, recorded: &RecordedRequest, incoming: &RecordedRequest) -> bool {
        let recorded_value = recorded.headers.iter()
            .find(|h| h.name.to_lowercase() == self.header_name)
            .map(|h| &h.value);
        let incoming_value = incoming.headers.iter()
            .find(|h| h.name.to_lowercase() == self.header_name)
            .map(|h| &h.value);
        
        recorded_value == incoming_value
    }
    
    fn name(&self) -> &'static str {
        "header"
    }
}

/// Matches by URL and method only (default matcher).
/// 
/// This is the recommended default - headers and body are ignored
/// since they often contain nonces or varying values.
#[derive(Debug, Default)]
pub struct UrlMethodMatcher;

impl Matcher for UrlMethodMatcher {
    fn matches(&self, recorded: &RecordedRequest, incoming: &RecordedRequest) -> bool {
        recorded.method.eq_ignore_ascii_case(&incoming.method) && recorded.url == incoming.url
    }
    
    fn name(&self) -> &'static str {
        "url_method"
    }
}

/// Matches by URL and method, with URL normalization.
/// 
/// Query parameters are sorted alphabetically before comparison,
/// so `?b=2&a=1` matches `?a=1&b=2`.
#[derive(Debug, Default)]
pub struct NormalizedUrlMethodMatcher;

impl Matcher for NormalizedUrlMethodMatcher {
    fn matches(&self, recorded: &RecordedRequest, incoming: &RecordedRequest) -> bool {
        if !recorded.method.eq_ignore_ascii_case(&incoming.method) {
            return false;
        }
        normalize_url(&recorded.url) == normalize_url(&incoming.url)
    }
    
    fn name(&self) -> &'static str {
        "normalized_url_method"
    }
}

/// Matches JSON bodies with normalization.
/// 
/// JSON bodies are parsed and compared structurally, ignoring:
/// - Key order
/// - Whitespace differences
/// 
/// Non-JSON bodies fall back to exact string comparison.
#[derive(Debug, Default)]
pub struct NormalizedJsonBodyMatcher;

impl Matcher for NormalizedJsonBodyMatcher {
    fn matches(&self, recorded: &RecordedRequest, incoming: &RecordedRequest) -> bool {
        match (&recorded.body, &incoming.body) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                // Try to parse as JSON and compare structurally
                match (serde_json::from_str::<serde_json::Value>(a), 
                       serde_json::from_str::<serde_json::Value>(b)) {
                    (Ok(json_a), Ok(json_b)) => json_a == json_b,
                    // If either isn't valid JSON, fall back to string comparison
                    _ => a == b,
                }
            }
            _ => false,
        }
    }
    
    fn name(&self) -> &'static str {
        "normalized_json_body"
    }
}

/// Normalize a URL by sorting query parameters alphabetically.
fn normalize_url(url: &str) -> String {
    // Find query string start
    let (base, query) = match url.find('?') {
        Some(pos) => (&url[..pos], Some(&url[pos + 1..])),
        None => (url, None),
    };
    
    match query {
        None | Some("") => base.to_string(),
        Some(q) => {
            // Parse and sort query params
            let mut params: Vec<(&str, &str)> = q
                .split('&')
                .filter_map(|pair| {
                    let mut parts = pair.splitn(2, '=');
                    Some((parts.next()?, parts.next().unwrap_or("")))
                })
                .collect();
            params.sort_by(|a, b| a.0.cmp(b.0).then_with(|| a.1.cmp(b.1)));
            
            let sorted_query: String = params
                .iter()
                .map(|(k, v)| {
                    if v.is_empty() {
                        k.to_string()
                    } else {
                        format!("{}={}", k, v)
                    }
                })
                .collect::<Vec<_>>()
                .join("&");
            
            format!("{}?{}", base, sorted_query)
        }
    }
}

/// Matches everything exactly: URL, method, headers, and body.
/// 
/// Use this when you need strict matching. Note that header order
/// doesn't matter, but all headers must be present with exact values.
#[derive(Debug, Default)]
pub struct ExactMatcher;

impl Matcher for ExactMatcher {
    fn matches(&self, recorded: &RecordedRequest, incoming: &RecordedRequest) -> bool {
        // Method must match (case-insensitive)
        if !recorded.method.eq_ignore_ascii_case(&incoming.method) {
            return false;
        }
        
        // URL must match exactly
        if recorded.url != incoming.url {
            return false;
        }
        
        // Body must match
        if recorded.body != incoming.body {
            return false;
        }
        
        // All headers must match (order doesn't matter)
        if recorded.headers.len() != incoming.headers.len() {
            return false;
        }
        
        for recorded_header in &recorded.headers {
            let found = incoming.headers.iter().any(|h| {
                h.name.eq_ignore_ascii_case(&recorded_header.name) && h.value == recorded_header.value
            });
            if !found {
                return false;
            }
        }
        
        true
    }
    
    fn name(&self) -> &'static str {
        "exact"
    }
}

/// Custom matcher using a user-provided function.
pub struct CustomMatcher<F>
where
    F: Fn(&RecordedRequest, &RecordedRequest) -> bool + Send + Sync,
{
    matcher_fn: F,
}

impl<F> CustomMatcher<F>
where
    F: Fn(&RecordedRequest, &RecordedRequest) -> bool + Send + Sync,
{
    /// Create a new custom matcher with a matching function.
    pub fn new(matcher_fn: F) -> Self {
        Self { matcher_fn }
    }
}

impl<F> Matcher for CustomMatcher<F>
where
    F: Fn(&RecordedRequest, &RecordedRequest) -> bool + Send + Sync,
{
    fn matches(&self, recorded: &RecordedRequest, incoming: &RecordedRequest) -> bool {
        (self.matcher_fn)(recorded, incoming)
    }
    
    fn name(&self) -> &'static str {
        "custom"
    }
}

impl<F> std::fmt::Debug for CustomMatcher<F>
where
    F: Fn(&RecordedRequest, &RecordedRequest) -> bool + Send + Sync,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomMatcher").finish()
    }
}

/// Composite matcher that requires all sub-matchers to match.
#[derive(Default)]
pub struct AllMatcher {
    matchers: Vec<Box<dyn Matcher>>,
}

impl std::fmt::Debug for AllMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AllMatcher")
            .field("matchers_count", &self.matchers.len())
            .finish()
    }
}

impl AllMatcher {
    /// Create a new composite matcher.
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Add a matcher.
    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, matcher: impl Matcher + 'static) -> Self {
        self.matchers.push(Box::new(matcher));
        self
    }
    
    /// Create a default matcher (method + URL).
    pub fn default_matchers() -> Self {
        Self::new()
            .add(MethodMatcher)
            .add(UrlMatcher)
    }
}

impl Matcher for AllMatcher {
    fn matches(&self, recorded: &RecordedRequest, incoming: &RecordedRequest) -> bool {
        self.matchers.iter().all(|m| m.matches(recorded, incoming))
    }
    
    fn name(&self) -> &'static str {
        "all"
    }
}

/// Extract the path from a URL.
fn extract_path(url: &str) -> &str {
    // Find the path after scheme://host
    if let Some(start) = url.find("://") {
        let rest = &url[start + 3..];
        if let Some(path_start) = rest.find('/') {
            let path = &rest[path_start..];
            // Remove query string
            if let Some(query) = path.find('?') {
                return &path[..query];
            }
            return path;
        }
        return "/";
    }
    url
}

#[cfg(test)]
mod tests {
    use super::*;
    

    fn req(method: &str, url: &str) -> RecordedRequest {
        RecordedRequest::new(method, url)
    }

    #[test]
    fn test_method_matcher() {
        let matcher = MethodMatcher;
        
        assert!(matcher.matches(&req("GET", "http://a.com"), &req("GET", "http://b.com")));
        assert!(matcher.matches(&req("GET", "http://a.com"), &req("get", "http://b.com")));
        assert!(!matcher.matches(&req("GET", "http://a.com"), &req("POST", "http://a.com")));
    }

    #[test]
    fn test_url_matcher() {
        let matcher = UrlMatcher;
        
        assert!(matcher.matches(&req("GET", "http://a.com"), &req("GET", "http://a.com")));
        assert!(!matcher.matches(&req("GET", "http://a.com"), &req("GET", "http://b.com")));
    }

    #[test]
    fn test_path_matcher() {
        let matcher = PathMatcher;
        
        assert!(matcher.matches(
            &req("GET", "http://a.com/path"),
            &req("GET", "http://b.com/path")
        ));
        assert!(matcher.matches(
            &req("GET", "http://a.com/path?a=1"),
            &req("GET", "http://a.com/path?b=2")
        ));
        assert!(!matcher.matches(
            &req("GET", "http://a.com/path1"),
            &req("GET", "http://a.com/path2")
        ));
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
        
        assert!(matcher.matches(&r1, &r2));
        assert!(!matcher.matches(&r1, &r3));
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
        
        assert!(matcher.matches(&r1, &r2));
        assert!(!matcher.matches(&r1, &r3));
    }

    #[test]
    fn test_all_matcher() {
        let matcher = AllMatcher::default_matchers();
        
        assert!(matcher.matches(&req("GET", "http://a.com"), &req("GET", "http://a.com")));
        assert!(!matcher.matches(&req("GET", "http://a.com"), &req("POST", "http://a.com")));
        assert!(!matcher.matches(&req("GET", "http://a.com"), &req("GET", "http://b.com")));
    }

    #[test]
    fn test_extract_path() {
        assert_eq!(extract_path("http://example.com/path"), "/path");
        assert_eq!(extract_path("http://example.com/path?query=1"), "/path");
        assert_eq!(extract_path("https://example.com/a/b/c"), "/a/b/c");
        assert_eq!(extract_path("http://example.com"), "/");
    }

    #[test]
    fn test_url_method_matcher() {
        let matcher = UrlMethodMatcher;
        
        // Same method and URL
        assert!(matcher.matches(&req("GET", "http://a.com"), &req("GET", "http://a.com")));
        
        // Different method
        assert!(!matcher.matches(&req("GET", "http://a.com"), &req("POST", "http://a.com")));
        
        // Different URL
        assert!(!matcher.matches(&req("GET", "http://a.com"), &req("GET", "http://b.com")));
        
        // Case insensitive method
        assert!(matcher.matches(&req("GET", "http://a.com"), &req("get", "http://a.com")));
    }

    #[test]
    fn test_exact_matcher_basic() {
        let matcher = ExactMatcher;
        
        let r1 = req("GET", "http://a.com");
        let r2 = req("GET", "http://a.com");
        
        assert!(matcher.matches(&r1, &r2));
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
    fn test_custom_matcher() {
        // Custom matcher that only checks the path
        let matcher = CustomMatcher::new(|recorded, incoming| {
            extract_path(&recorded.url) == extract_path(&incoming.url)
        });
        
        assert!(matcher.matches(
            &req("GET", "http://a.com/path"),
            &req("POST", "http://b.com/path")  // Different method and host, same path
        ));
        
        assert!(!matcher.matches(
            &req("GET", "http://a.com/path1"),
            &req("GET", "http://a.com/path2")
        ));
    }

    #[test]
    fn test_normalize_url() {
        // Basic sorting
        assert_eq!(
            normalize_url("http://example.com?b=2&a=1"),
            "http://example.com?a=1&b=2"
        );
        
        // Already sorted
        assert_eq!(
            normalize_url("http://example.com?a=1&b=2"),
            "http://example.com?a=1&b=2"
        );
        
        // No query string
        assert_eq!(
            normalize_url("http://example.com/path"),
            "http://example.com/path"
        );
        
        // Empty query
        assert_eq!(
            normalize_url("http://example.com?"),
            "http://example.com"
        );
        
        // Multiple values for same key
        assert_eq!(
            normalize_url("http://example.com?z=1&a=2&a=1"),
            "http://example.com?a=1&a=2&z=1"
        );
        
        // Keys without values
        assert_eq!(
            normalize_url("http://example.com?c&b=1&a"),
            "http://example.com?a&b=1&c"
        );
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
    }
}
