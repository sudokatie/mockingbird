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
    use crate::cassette::Header;

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
}
