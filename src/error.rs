//! Error types for mockingbird.

use thiserror::Error;

/// Result type alias for mockingbird operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur in mockingbird.
#[derive(Debug, Error)]
pub enum Error {
    /// Cassette file not found.
    #[error("Cassette not found: {0}")]
    CassetteNotFound(String),

    /// No matching interaction found in cassette.
    #[error("No matching interaction found for request")]
    NoMatch,

    /// Cassette format error.
    #[error("Invalid cassette format: {0}")]
    InvalidFormat(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// HTTP request error.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// URL parsing error.
    #[error("URL error: {0}")]
    Url(#[from] url::ParseError),

    /// Proxy server error.
    #[error("Proxy error: {0}")]
    Proxy(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cassette_not_found() {
        let err = Error::CassetteNotFound("test.json".to_string());
        assert!(err.to_string().contains("test.json"));
    }

    #[test]
    fn test_no_match() {
        let err = Error::NoMatch;
        assert!(err.to_string().contains("No matching"));
    }

    #[test]
    fn test_invalid_format() {
        let err = Error::InvalidFormat("bad json".to_string());
        assert!(err.to_string().contains("bad json"));
    }

    #[test]
    fn test_proxy_error() {
        let err = Error::Proxy("connection failed".to_string());
        assert!(err.to_string().contains("connection failed"));
    }

    #[test]
    fn test_config_error() {
        let err = Error::Config("missing field".to_string());
        assert!(err.to_string().contains("missing field"));
    }
}
