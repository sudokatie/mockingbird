//! Error types for mockingbird.

use chrono::{DateTime, Duration, Utc};
use std::path::PathBuf;
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

    /// Cassette interaction has expired.
    #[error("Cassette expired: recorded at {recorded_at}, max age {max_age}")]
    CassetteExpired {
        recorded_at: DateTime<Utc>,
        max_age: Duration,
    },

    /// Cassette format error.
    #[error("Invalid cassette format: {0}")]
    InvalidFormat(String),

    /// Failed to read cassette file.
    #[error("Failed to read cassette {path}: {source}")]
    CassetteRead {
        path: PathBuf,
        source: std::io::Error,
    },

    /// Failed to write cassette file.
    #[error("Failed to write cassette {path}: {source}")]
    CassetteWrite {
        path: PathBuf,
        source: std::io::Error,
    },

    /// Invalid JSON path in filter.
    #[error("Invalid JSON path: {0}")]
    InvalidJsonPath(String),

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
    
    // Recorded error variants (replayed from cassette)
    
    /// Recorded timeout error (replayed from cassette).
    #[error("Recorded timeout: {message}")]
    RecordedTimeout { message: String },
    
    /// Recorded connection error (replayed from cassette).
    #[error("Recorded connection error: {message}")]
    RecordedConnection { message: String },
    
    /// Recorded DNS error (replayed from cassette).
    #[error("Recorded DNS error: {message}")]
    RecordedDns { message: String },
    
    /// Recorded TLS error (replayed from cassette).
    #[error("Recorded TLS error: {message}")]
    RecordedTls { message: String },
    
    /// Recorded cancelled request (replayed from cassette).
    #[error("Recorded cancelled: {message}")]
    RecordedCancelled { message: String },
    
    /// Recorded unknown error (replayed from cassette).
    #[error("Recorded error: {message}")]
    RecordedUnknown { message: String },
}

impl Error {
    /// Check if this is a recorded error being replayed.
    pub fn is_recorded_error(&self) -> bool {
        matches!(
            self,
            Error::RecordedTimeout { .. }
                | Error::RecordedConnection { .. }
                | Error::RecordedDns { .. }
                | Error::RecordedTls { .. }
                | Error::RecordedCancelled { .. }
                | Error::RecordedUnknown { .. }
        )
    }
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
    fn test_cassette_read() {
        let err = Error::CassetteRead {
            path: PathBuf::from("/path/to/cassette.json"),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"),
        };
        let msg = err.to_string();
        assert!(msg.contains("cassette"));
        assert!(msg.contains("/path/to/cassette.json"));
    }

    #[test]
    fn test_cassette_write() {
        let err = Error::CassetteWrite {
            path: PathBuf::from("/path/to/cassette.json"),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied"),
        };
        let msg = err.to_string();
        assert!(msg.contains("write"));
        assert!(msg.contains("/path/to/cassette.json"));
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

    #[test]
    fn test_recorded_errors() {
        let timeout = Error::RecordedTimeout { message: "timed out".to_string() };
        assert!(timeout.is_recorded_error());
        assert!(timeout.to_string().contains("timed out"));

        let conn = Error::RecordedConnection { message: "refused".to_string() };
        assert!(conn.is_recorded_error());

        let dns = Error::RecordedDns { message: "lookup failed".to_string() };
        assert!(dns.is_recorded_error());

        let tls = Error::RecordedTls { message: "cert invalid".to_string() };
        assert!(tls.is_recorded_error());

        let cancelled = Error::RecordedCancelled { message: "cancelled".to_string() };
        assert!(cancelled.is_recorded_error());

        let unknown = Error::RecordedUnknown { message: "unknown".to_string() };
        assert!(unknown.is_recorded_error());

        // Non-recorded errors
        assert!(!Error::NoMatch.is_recorded_error());
    }
}
