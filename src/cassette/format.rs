//! Cassette file format handling.
//!
//! Supports JSON (default) and YAML (with `yaml` feature).

use std::path::Path;

/// Supported cassette file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    /// JSON format (default).
    #[default]
    Json,
    /// YAML format (requires `yaml` feature).
    Yaml,
}

impl Format {
    /// Detect format from file extension.
    /// 
    /// - `.yaml` or `.yml` -> YAML
    /// - anything else -> JSON
    pub fn from_path<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref();
        match path.extension().and_then(|e| e.to_str()) {
            Some("yaml") | Some("yml") => Format::Yaml,
            _ => Format::Json,
        }
    }
    
    /// Get the conventional file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::Yaml => "yaml",
        }
    }
    
    /// Get the MIME type for this format.
    pub fn mime_type(&self) -> &'static str {
        match self {
            Format::Json => "application/json",
            Format::Yaml => "application/x-yaml",
        }
    }
    
    /// Check if this format is available (YAML requires feature).
    pub fn is_available(&self) -> bool {
        match self {
            Format::Json => true,
            Format::Yaml => cfg!(feature = "yaml"),
        }
    }
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Format::Json => write!(f, "JSON"),
            Format::Yaml => write!(f, "YAML"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_detection() {
        assert_eq!(Format::from_path("test.json"), Format::Json);
        assert_eq!(Format::from_path("test.yaml"), Format::Yaml);
        assert_eq!(Format::from_path("test.yml"), Format::Yaml);
        assert_eq!(Format::from_path("test"), Format::Json);
        assert_eq!(Format::from_path("path/to/cassette.yaml"), Format::Yaml);
        assert_eq!(Format::from_path(""), Format::Json);
    }

    #[test]
    fn test_format_extension() {
        assert_eq!(Format::Json.extension(), "json");
        assert_eq!(Format::Yaml.extension(), "yaml");
    }

    #[test]
    fn test_format_mime_type() {
        assert_eq!(Format::Json.mime_type(), "application/json");
        assert_eq!(Format::Yaml.mime_type(), "application/x-yaml");
    }

    #[test]
    fn test_format_display() {
        assert_eq!(Format::Json.to_string(), "JSON");
        assert_eq!(Format::Yaml.to_string(), "YAML");
    }

    #[test]
    fn test_json_always_available() {
        assert!(Format::Json.is_available());
    }

    #[test]
    fn test_format_default() {
        assert_eq!(Format::default(), Format::Json);
    }
}
