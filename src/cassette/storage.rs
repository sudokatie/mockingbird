//! Cassette file storage.

use super::Cassette;
use crate::error::{Error, Result};
use std::fs;
use std::path::Path;

/// Load a cassette from a file.
pub fn load_cassette<P: AsRef<Path>>(path: P) -> Result<Cassette> {
    let path = path.as_ref();
    
    if !path.exists() {
        return Err(Error::CassetteNotFound(path.display().to_string()));
    }
    
    let content = fs::read_to_string(path)?;
    let cassette: Cassette = serde_json::from_str(&content)?;
    
    Ok(cassette)
}

/// Save a cassette to a file.
pub fn save_cassette<P: AsRef<Path>>(path: P, cassette: &Cassette) -> Result<()> {
    let path = path.as_ref();
    
    // Create parent directory if needed
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    
    let content = serde_json::to_string_pretty(cassette)?;
    fs::write(path, content)?;
    
    Ok(())
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
}
