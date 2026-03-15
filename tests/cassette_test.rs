//! Cassette read/write tests.
//!
//! Tests for cassette serialization, storage, and indexing.

use mockingbird::cassette::{
    Cassette, Format, Interaction, RecordedRequest, RecordedResponse, RecordedError, ErrorKind,
    Header, BodyEncoding, IndexedCassette, load_cassette, save_cassette, load_or_create,
};
use mockingbird::Error;
use tempfile::TempDir;

#[test]
fn test_cassette_new() {
    let cassette = Cassette::new();
    assert_eq!(cassette.version, 1);
    assert!(cassette.is_empty());
    assert!(cassette.recorded_with.starts_with("mockingbird/"));
}

#[test]
fn test_cassette_add_interaction() {
    let mut cassette = Cassette::new();
    let req = RecordedRequest::new("GET", "https://example.com");
    let res = RecordedResponse::new(200);
    let interaction = Interaction::new(req, res);
    
    cassette.add(interaction);
    
    assert_eq!(cassette.len(), 1);
    assert!(!cassette.is_empty());
}

#[test]
fn test_recorded_request_builder() {
    let req = RecordedRequest::new("POST", "https://api.example.com/users")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(r#"{"name": "test"}"#);
    
    assert_eq!(req.method, "POST");
    assert_eq!(req.url, "https://api.example.com/users");
    assert_eq!(req.headers.len(), 2);
    assert!(req.body.is_some());
}

#[test]
fn test_recorded_response_builder() {
    let res = RecordedResponse::new(201)
        .header("Content-Type", "application/json")
        .header("Location", "/users/1")
        .body(r#"{"id": 1}"#);
    
    assert_eq!(res.status, 201);
    assert_eq!(res.headers.len(), 2);
    assert_eq!(res.body, Some(r#"{"id": 1}"#.to_string()));
}

#[test]
fn test_header() {
    let header = Header::new("Authorization", "Bearer token");
    assert_eq!(header.name, "Authorization");
    assert_eq!(header.value, "Bearer token");
}

#[test]
fn test_body_encoding_default() {
    assert_eq!(BodyEncoding::default(), BodyEncoding::Text);
}

#[test]
fn test_error_interaction() {
    let req = RecordedRequest::new("GET", "https://example.com/timeout");
    let error = RecordedError::timeout("Connection timed out after 30s");
    let interaction = Interaction::error(req, error);
    
    assert!(interaction.is_error());
    assert!(interaction.response.is_none());
    assert!(interaction.error.is_some());
    
    let err = interaction.get_error().unwrap();
    assert_eq!(err.kind, ErrorKind::Timeout);
    assert!(err.message.contains("timed out"));
}

#[test]
fn test_recorded_error_constructors() {
    let timeout = RecordedError::timeout("timed out");
    assert_eq!(timeout.kind, ErrorKind::Timeout);
    
    let conn = RecordedError::connection("refused");
    assert_eq!(conn.kind, ErrorKind::Connection);
    
    let dns = RecordedError::dns("lookup failed");
    assert_eq!(dns.kind, ErrorKind::Dns);
}

#[test]
fn test_save_and_load() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    let mut cassette = Cassette::new();
    cassette.add(Interaction::new(
        RecordedRequest::new("GET", "https://example.com"),
        RecordedResponse::new(200).body("hello"),
    ));
    
    save_cassette(&path, &cassette).unwrap();
    
    let loaded = load_cassette(&path).unwrap();
    assert_eq!(loaded.version, 1);
    assert_eq!(loaded.len(), 1);
    assert_eq!(
        loaded.interactions[0].response.as_ref().unwrap().body,
        Some("hello".to_string())
    );
}

#[test]
fn test_load_nonexistent() {
    let result = load_cassette("/nonexistent/cassette.json");
    assert!(matches!(result, Err(Error::CassetteNotFound(_))));
}

#[test]
fn test_save_creates_parent_dirs() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("subdir/nested/test.json");
    
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
    assert_eq!(Format::from_path("no_extension"), Format::Json);
}

#[test]
fn test_format_properties() {
    assert_eq!(Format::Json.extension(), "json");
    assert_eq!(Format::Yaml.extension(), "yaml");
    assert_eq!(Format::Json.mime_type(), "application/json");
    assert_eq!(Format::Yaml.mime_type(), "application/x-yaml");
    assert!(Format::Json.is_available());
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

#[test]
fn test_indexed_cassette_new() {
    let cassette = Cassette::new();
    let indexed = IndexedCassette::new(cassette);
    assert!(indexed.is_empty());
    assert_eq!(indexed.len(), 0);
}

#[test]
fn test_indexed_cassette_find() {
    let mut cassette = Cassette::new();
    cassette.add(Interaction::new(
        RecordedRequest::new("GET", "https://example.com/a"),
        RecordedResponse::new(200),
    ));
    cassette.add(Interaction::new(
        RecordedRequest::new("POST", "https://example.com/a"),
        RecordedResponse::new(201),
    ));
    cassette.add(Interaction::new(
        RecordedRequest::new("GET", "https://example.com/b"),
        RecordedResponse::new(200),
    ));
    
    let indexed = IndexedCassette::new(cassette);
    
    // Find by method and URL
    let matches = indexed.find_by_method_url("GET", "https://example.com/a");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0], 0);
    
    let matches = indexed.find_by_method_url("POST", "https://example.com/a");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0], 1);
    
    let matches = indexed.find_by_method_url("DELETE", "https://example.com/a");
    assert!(matches.is_empty());
}

#[test]
fn test_indexed_cassette_add() {
    let cassette = Cassette::new();
    let mut indexed = IndexedCassette::new(cassette);
    
    indexed.add(Interaction::new(
        RecordedRequest::new("GET", "https://example.com/new"),
        RecordedResponse::new(200),
    ));
    
    assert_eq!(indexed.len(), 1);
    
    let matches = indexed.find_by_method_url("GET", "https://example.com/new");
    assert_eq!(matches.len(), 1);
}

#[test]
fn test_indexed_cassette_multiple_same_url() {
    let mut cassette = Cassette::new();
    cassette.add(Interaction::new(
        RecordedRequest::new("GET", "https://example.com/api"),
        RecordedResponse::new(200),
    ));
    cassette.add(Interaction::new(
        RecordedRequest::new("GET", "https://example.com/api"),
        RecordedResponse::new(201),
    ));
    
    let indexed = IndexedCassette::new(cassette);
    let matches = indexed.find_by_method_url("GET", "https://example.com/api");
    
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0], 0);
    assert_eq!(matches[1], 1);
}

#[test]
fn test_indexed_cassette_case_insensitive_method() {
    let mut cassette = Cassette::new();
    cassette.add(Interaction::new(
        RecordedRequest::new("get", "https://example.com/api"),
        RecordedResponse::new(200),
    ));
    
    let indexed = IndexedCassette::new(cassette);
    
    // Should find regardless of case
    let matches = indexed.find_by_method_url("GET", "https://example.com/api");
    assert_eq!(matches.len(), 1);
    
    let matches = indexed.find_by_method_url("get", "https://example.com/api");
    assert_eq!(matches.len(), 1);
}

#[test]
fn test_cassette_serialization() {
    let mut cassette = Cassette::new();
    cassette.add(Interaction::new(
        RecordedRequest::new("GET", "https://example.com")
            .header("Accept", "application/json"),
        RecordedResponse::new(200)
            .header("Content-Type", "application/json")
            .body(r#"{"test": true}"#),
    ));
    
    let json = serde_json::to_string(&cassette).unwrap();
    assert!(json.contains("\"version\":1"));
    assert!(json.contains("\"method\":\"GET\""));
    assert!(json.contains("application/json"));
    
    let parsed: Cassette = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.len(), 1);
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
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(!content.starts_with("{"), "Should be YAML, not JSON");
}

#[cfg(feature = "yaml")]
#[test]
fn test_yml_extension() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.yml");
    
    let cassette = Cassette::new();
    save_cassette(&path, &cassette).unwrap();
    
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(!content.starts_with("{"), "Should be YAML, not JSON");
    
    let loaded = load_cassette(&path).unwrap();
    assert!(loaded.is_empty());
}
