//! Cassette file storage.

use super::Cassette;
use crate::error::{Error, Result};
use std::fs;
use std::path::Path;

/// Supported cassette file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// JSON format (default).
    Json,
    /// YAML format (requires `yaml` feature).
    Yaml,
}

impl Format {
    /// Detect format from file extension.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref();
        match path.extension().and_then(|e| e.to_str()) {
            Some("yaml") | Some("yml") => Format::Yaml,
            _ => Format::Json,
        }
    }
}

/// Load a cassette from a file.
/// 
/// Format is auto-detected from file extension:
/// - `.yaml` or `.yml` -> YAML (requires `yaml` feature)
/// - anything else -> JSON
pub fn load_cassette<P: AsRef<Path>>(path: P) -> Result<Cassette> {
    let path = path.as_ref();
    
    if !path.exists() {
        return Err(Error::CassetteNotFound(path.display().to_string()));
    }
    
    let content = fs::read_to_string(path)?;
    let format = Format::from_path(path);
    
    let cassette = match format {
        Format::Json => serde_json::from_str(&content)?,
        Format::Yaml => {
            #[cfg(feature = "yaml")]
            {
                serde_yaml::from_str(&content).map_err(|e| Error::InvalidFormat(e.to_string()))?
            }
            #[cfg(not(feature = "yaml"))]
            {
                return Err(Error::InvalidFormat(
                    "YAML support requires the 'yaml' feature. Use JSON or enable the feature.".to_string()
                ));
            }
        }
    };
    
    Ok(cassette)
}

/// Save a cassette to a file.
/// 
/// Format is auto-detected from file extension:
/// - `.yaml` or `.yml` -> YAML (requires `yaml` feature)
/// - anything else -> JSON
pub fn save_cassette<P: AsRef<Path>>(path: P, cassette: &Cassette) -> Result<()> {
    let path = path.as_ref();
    
    // Create parent directory if needed
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    
    let format = Format::from_path(path);
    
    let content = match format {
        Format::Json => serde_json::to_string_pretty(cassette)?,
        Format::Yaml => {
            #[cfg(feature = "yaml")]
            {
                serde_yaml::to_string(cassette).map_err(|e| Error::InvalidFormat(e.to_string()))?
            }
            #[cfg(not(feature = "yaml"))]
            {
                return Err(Error::InvalidFormat(
                    "YAML support requires the 'yaml' feature. Use JSON or enable the feature.".to_string()
                ));
            }
        }
    };
    
    // Atomic write via temp file
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, &content)?;
    fs::rename(&tmp_path, path)?;
    
    Ok(())
}

/// Load a cassette or create a new empty one if file doesn't exist.
pub fn load_or_create<P: AsRef<Path>>(path: P) -> Result<Cassette> {
    let path = path.as_ref();
    if path.exists() {
        load_cassette(path)
    } else {
        Ok(Cassette::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let cassette = Cassette::new();
        save_cassette(&path, &cassette).unwrap();
        
        let loaded = load_cassette(&path).unwrap();
        assert_eq!(loaded.version, 1);
    }

    #[test]
    fn test_load_nonexistent() {
        let result = load_cassette("/nonexistent/cassette.json");
        assert!(matches!(result, Err(Error::CassetteNotFound(_))));
    }

    #[test]
    fn test_save_creates_parent_dir() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("subdir/test.json");
        
        let cassette = Cassette::new();
        save_cassette(&path, &cassette).unwrap();
        
        assert!(path.exists());
    }

    #[test]
    fn test_format_detection() {
        assert_eq!(Format::from_path("test.json"), Format::Json);
        assert_eq!(Format::from_path("test.yaml"), Format::Yaml);
        assert_eq!(Format::from_path("test.yml"), Format::Yaml);
        assert_eq!(Format::from_path("test"), Format::Json);
        assert_eq!(Format::from_path("path/to/cassette.yaml"), Format::Yaml);
    }

    #[test]
    fn test_load_or_create_missing() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("missing.json");
        
        let cassette = load_or_create(&path).unwrap();
        assert!(cassette.is_empty());
    }

    #[test]
    fn test_load_or_create_existing() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("existing.json");
        
        let mut cassette = Cassette::new();
        cassette.metadata.insert("test".to_string(), "value".to_string());
        save_cassette(&path, &cassette).unwrap();
        
        let loaded = load_or_create(&path).unwrap();
        assert_eq!(loaded.metadata.get("test"), Some(&"value".to_string()));
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn test_yaml_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.yaml");
        
        let mut cassette = Cassette::new();
        cassette.metadata.insert("format".to_string(), "yaml".to_string());
        save_cassette(&path, &cassette).unwrap();
        
        let loaded = load_cassette(&path).unwrap();
        assert_eq!(loaded.metadata.get("format"), Some(&"yaml".to_string()));
        
        // Verify it's actually YAML
        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.starts_with("{"));  // Not JSON
    }

    #[test]
    fn test_atomic_write() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("atomic.json");
        
        let cassette = Cassette::new();
        save_cassette(&path, &cassette).unwrap();
        
        // Temp file should not exist after successful write
        let tmp_path = path.with_extension("tmp");
        assert!(!tmp_path.exists());
    }
}
