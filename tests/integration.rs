//! Integration tests for mockingbird.

use mockingbird::cassette::{Cassette, Interaction, RecordedRequest, RecordedResponse, save_cassette};
use mockingbird::Client;
use tempfile::TempDir;

#[tokio::test]
async fn test_playback_simple() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    // Create cassette
    let mut cassette = Cassette::new();
    let req = RecordedRequest::new("GET", "https://api.example.com/users/1");
    let res = RecordedResponse::new(200)
        .header("Content-Type", "application/json")
        .body(r#"{"id":1,"name":"Alice"}"#);
    cassette.add(Interaction::new(req, res));
    save_cassette(&path, &cassette).unwrap();
    
    // Use client
    let client = Client::playback(&path).build().unwrap();
    let response = client.get("https://api.example.com/users/1").send().await.unwrap();
    
    assert_eq!(response.status(), 200);
    assert_eq!(response.header("content-type"), Some("application/json"));
    
    #[derive(serde::Deserialize)]
    struct User {
        id: i32,
        name: String,
    }
    
    let user: User = response.json().unwrap();
    assert_eq!(user.id, 1);
    assert_eq!(user.name, "Alice");
}

#[tokio::test]
async fn test_playback_not_found() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("empty.json");
    save_cassette(&path, &Cassette::new()).unwrap();
    
    let client = Client::playback(&path).build().unwrap();
    let result = client.get("https://api.example.com/missing").send().await;
    
    assert!(result.is_err());
}

#[tokio::test]
async fn test_playback_method_matters() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    // Create cassette with GET request
    let mut cassette = Cassette::new();
    let req = RecordedRequest::new("GET", "https://api.example.com/data");
    let res = RecordedResponse::new(200).body("get response");
    cassette.add(Interaction::new(req, res));
    save_cassette(&path, &cassette).unwrap();
    
    let client = Client::playback(&path).build().unwrap();
    
    // GET should work
    let response = client.get("https://api.example.com/data").send().await.unwrap();
    assert_eq!(response.text().unwrap(), "get response");
    
    // POST should not match
    let result = client.post("https://api.example.com/data").send().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_playback_multiple_interactions() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    let mut cassette = Cassette::new();
    
    // Add multiple interactions
    cassette.add(Interaction::new(
        RecordedRequest::new("GET", "https://api.example.com/users"),
        RecordedResponse::new(200).body("users list"),
    ));
    cassette.add(Interaction::new(
        RecordedRequest::new("GET", "https://api.example.com/posts"),
        RecordedResponse::new(200).body("posts list"),
    ));
    cassette.add(Interaction::new(
        RecordedRequest::new("POST", "https://api.example.com/users"),
        RecordedResponse::new(201).body("created"),
    ));
    
    save_cassette(&path, &cassette).unwrap();
    
    let client = Client::playback(&path).build().unwrap();
    
    // Each request should match correctly
    let r1 = client.get("https://api.example.com/users").send().await.unwrap();
    assert_eq!(r1.text().unwrap(), "users list");
    
    let r2 = client.get("https://api.example.com/posts").send().await.unwrap();
    assert_eq!(r2.text().unwrap(), "posts list");
    
    let r3 = client.post("https://api.example.com/users").send().await.unwrap();
    assert_eq!(r3.status(), 201);
    assert_eq!(r3.text().unwrap(), "created");
}

#[tokio::test]
async fn test_client_auto_creates_cassette() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("new.json");
    
    // File doesn't exist yet
    assert!(!path.exists());
    
    // Client should create empty cassette
    let _client = Client::auto(&path).build().unwrap();
    
    // Note: cassette isn't saved until an interaction is recorded
}

#[tokio::test]
async fn test_different_status_codes() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    let mut cassette = Cassette::new();
    cassette.add(Interaction::new(
        RecordedRequest::new("GET", "https://api.example.com/ok"),
        RecordedResponse::new(200),
    ));
    cassette.add(Interaction::new(
        RecordedRequest::new("GET", "https://api.example.com/created"),
        RecordedResponse::new(201),
    ));
    cassette.add(Interaction::new(
        RecordedRequest::new("GET", "https://api.example.com/notfound"),
        RecordedResponse::new(404),
    ));
    cassette.add(Interaction::new(
        RecordedRequest::new("GET", "https://api.example.com/error"),
        RecordedResponse::new(500),
    ));
    save_cassette(&path, &cassette).unwrap();
    
    let client = Client::playback(&path).build().unwrap();
    
    assert_eq!(client.get("https://api.example.com/ok").send().await.unwrap().status(), 200);
    assert_eq!(client.get("https://api.example.com/created").send().await.unwrap().status(), 201);
    assert_eq!(client.get("https://api.example.com/notfound").send().await.unwrap().status(), 404);
    assert_eq!(client.get("https://api.example.com/error").send().await.unwrap().status(), 500);
}

#[tokio::test]
async fn test_request_with_headers() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    let mut cassette = Cassette::new();
    cassette.add(Interaction::new(
        RecordedRequest::new("GET", "https://api.example.com/auth"),
        RecordedResponse::new(200).body("authenticated"),
    ));
    save_cassette(&path, &cassette).unwrap();
    
    let client = Client::playback(&path).build().unwrap();
    
    // Headers on request don't affect matching by default (only method + URL)
    let response = client
        .get("https://api.example.com/auth")
        .header("Authorization", "Bearer token123")
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.text().unwrap(), "authenticated");
}

#[tokio::test]
async fn test_post_with_body() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    let mut cassette = Cassette::new();
    cassette.add(Interaction::new(
        RecordedRequest::new("POST", "https://api.example.com/users"),
        RecordedResponse::new(201).body(r#"{"id":42}"#),
    ));
    save_cassette(&path, &cassette).unwrap();
    
    let client = Client::playback(&path).build().unwrap();
    
    #[derive(serde::Serialize)]
    struct NewUser {
        name: String,
    }
    
    let response = client
        .post("https://api.example.com/users")
        .json(&NewUser { name: "Bob".to_string() })
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 201);
    
    #[derive(serde::Deserialize)]
    struct Created {
        id: i32,
    }
    
    let created: Created = response.json().unwrap();
    assert_eq!(created.id, 42);
}

#[tokio::test]
async fn test_patch_request() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    let mut cassette = Cassette::new();
    cassette.add(Interaction::new(
        RecordedRequest::new("PATCH", "https://api.example.com/users/1"),
        RecordedResponse::new(200).body(r#"{"id":1,"updated":true}"#),
    ));
    save_cassette(&path, &cassette).unwrap();
    
    let client = Client::playback(&path).build().unwrap();
    let response = client
        .patch("https://api.example.com/users/1")
        .json(&serde_json::json!({"name": "Updated"}))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_head_request() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    let mut cassette = Cassette::new();
    cassette.add(Interaction::new(
        RecordedRequest::new("HEAD", "https://api.example.com/status"),
        RecordedResponse::new(200)
            .header("X-Status", "healthy"),
    ));
    save_cassette(&path, &cassette).unwrap();
    
    let client = Client::playback(&path).build().unwrap();
    let response = client
        .head("https://api.example.com/status")
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    assert_eq!(response.header("x-status"), Some("healthy"));
}

#[tokio::test]
async fn test_custom_method() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    let mut cassette = Cassette::new();
    cassette.add(Interaction::new(
        RecordedRequest::new("OPTIONS", "https://api.example.com/cors"),
        RecordedResponse::new(204)
            .header("Access-Control-Allow-Origin", "*"),
    ));
    save_cassette(&path, &cassette).unwrap();
    
    let client = Client::playback(&path).build().unwrap();
    let response = client
        .request("OPTIONS", "https://api.example.com/cors")
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 204);
}

#[tokio::test]
async fn test_query_parameters() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    let mut cassette = Cassette::new();
    cassette.add(Interaction::new(
        RecordedRequest::new("GET", "https://api.example.com/search?q=rust&page=1"),
        RecordedResponse::new(200).body(r#"{"results":[]}"#),
    ));
    save_cassette(&path, &cassette).unwrap();
    
    let client = Client::playback(&path).build().unwrap();
    let response = client
        .get("https://api.example.com/search")
        .query(&[("q", "rust"), ("page", "1")])
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_middleware_layer() {
    use mockingbird::MockingbirdLayer;
    use mockingbird::cassette::RecordedRequest;
    
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    // Record some interactions
    let layer = MockingbirdLayer::record(&path).build().unwrap();
    
    let req = RecordedRequest::new("GET", "https://example.com/data");
    let resp = RecordedResponse::new(200).body("layer data");
    layer.record_interaction(req, resp).unwrap();
    
    // Playback
    let layer2 = MockingbirdLayer::playback(&path).build().unwrap();
    let req2 = RecordedRequest::new("GET", "https://example.com/data");
    let result = layer2.try_playback(&req2).unwrap();
    
    assert!(result.is_some());
    match result.unwrap() {
        mockingbird::middleware::PlaybackResult::Response(resp) => {
            assert_eq!(resp.body, Some("layer data".to_string()));
        }
        mockingbird::middleware::PlaybackResult::Error(_) => panic!("expected response"),
    }
}

#[tokio::test]
async fn test_json_path_filter() {
    use mockingbird::filter::JsonPathRequestFilter;
    
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    // Create cassette
    let mut cassette = Cassette::new();
    cassette.add(Interaction::new(
        RecordedRequest::new("POST", "https://api.example.com/login")
            .body(r#"{"username":"alice","password":"[FILTERED]"}"#),
        RecordedResponse::new(200),
    ));
    save_cassette(&path, &cassette).unwrap();
    
    // Client with filter - this would filter during record
    let _client = Client::auto(&path)
        .request_filter(JsonPathRequestFilter::new("$.password", "[FILTERED]"))
        .build()
        .unwrap();
}

#[tokio::test]
async fn test_indexed_cassette_performance() {
    use mockingbird::cassette::IndexedCassette;
    
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("large.json");
    
    // Create cassette with many interactions
    let mut cassette = Cassette::new();
    for i in 0..100 {
        cassette.add(Interaction::new(
            RecordedRequest::new("GET", format!("https://api.example.com/item/{}", i)),
            RecordedResponse::new(200).body(format!("item {}", i)),
        ));
    }
    save_cassette(&path, &cassette).unwrap();
    
    // Use indexed lookup
    let indexed = IndexedCassette::new(cassette);
    
    // Should find item 50 instantly via index
    let matches = indexed.find_by_method_url("GET", "https://api.example.com/item/50");
    assert_eq!(matches.len(), 1);
    
    let interaction = indexed.get(matches[0]).unwrap();
    assert_eq!(interaction.response.as_ref().unwrap().body, Some("item 50".to_string()));
}

#[tokio::test]
async fn test_normalized_url_matching() {
    use mockingbird::matcher::NormalizedUrlMethodMatcher;
    
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    // Create cassette with query params in one order
    let mut cassette = Cassette::new();
    cassette.add(Interaction::new(
        RecordedRequest::new("GET", "https://api.example.com/search?b=2&a=1"),
        RecordedResponse::new(200).body("found"),
    ));
    save_cassette(&path, &cassette).unwrap();
    
    // Build client with normalized URL matching
    let client = Client::playback(&path)
        .match_by(NormalizedUrlMethodMatcher)
        .build()
        .unwrap();
    
    // Query params in different order should still match
    let resp = client
        .get("https://api.example.com/search")
        .query(&[("a", "1"), ("b", "2")])
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().unwrap(), "found");
}

#[tokio::test]
async fn test_normalized_json_body_matching() {
    use mockingbird::matcher::{AllMatcher, MethodMatcher, UrlMatcher, NormalizedJsonBodyMatcher};
    
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    // Create cassette with JSON body in one key order
    let mut cassette = Cassette::new();
    let mut req = RecordedRequest::new("POST", "https://api.example.com/data");
    req.body = Some(r#"{"z":3,"a":1,"m":2}"#.to_string());
    cassette.add(Interaction::new(req, RecordedResponse::new(201).body("created")));
    save_cassette(&path, &cassette).unwrap();
    
    // Build client with JSON body normalization
    let client = Client::playback(&path)
        .match_by(
            AllMatcher::new()
                .add(MethodMatcher)
                .add(UrlMatcher)
                .add(NormalizedJsonBodyMatcher)
        )
        .build()
        .unwrap();
    
    // JSON with different key order should still match
    let resp = client
        .post("https://api.example.com/data")
        .json(&serde_json::json!({"a": 1, "m": 2, "z": 3}))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 201);
}

#[tokio::test]
async fn test_error_interaction() {
    use mockingbird::cassette::{RecordedError, ErrorKind};
    
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    // Create cassette with an error interaction
    let mut cassette = Cassette::new();
    let req = RecordedRequest::new("GET", "https://api.example.com/timeout");
    let error = RecordedError::timeout("Connection timed out after 30s");
    cassette.add(Interaction::error(req, error));
    save_cassette(&path, &cassette).unwrap();
    
    // Verify the cassette loaded correctly
    let loaded = mockingbird::cassette::load_cassette(&path).unwrap();
    assert_eq!(loaded.len(), 1);
    
    let interaction = &loaded.interactions[0];
    assert!(interaction.is_error());
    assert!(interaction.response.is_none());
    assert!(interaction.error.is_some());
    
    let err = interaction.error.as_ref().unwrap();
    assert_eq!(err.kind, ErrorKind::Timeout);
    assert!(err.message.contains("timed out"));
}

#[tokio::test]
async fn test_cassette_format_detection() {
    use mockingbird::cassette::Format;
    
    assert_eq!(Format::from_path("test.json"), Format::Json);
    assert_eq!(Format::from_path("test.yaml"), Format::Yaml);
    assert_eq!(Format::from_path("test.yml"), Format::Yaml);
    assert_eq!(Format::from_path("path/to/cassette.json"), Format::Json);
    assert_eq!(Format::from_path("no_extension"), Format::Json);
}

#[test]
fn test_load_or_create() {
    use mockingbird::cassette::{load_or_create, save_cassette};
    
    let tmp = TempDir::new().unwrap();
    
    // Non-existent file creates empty cassette
    let path = tmp.path().join("new.json");
    let c = load_or_create(&path).unwrap();
    assert!(c.is_empty());
    
    // Existing file is loaded
    let path2 = tmp.path().join("existing.json");
    let mut c2 = Cassette::new();
    c2.metadata.insert("test".to_string(), "value".to_string());
    save_cassette(&path2, &c2).unwrap();
    
    let loaded = load_or_create(&path2).unwrap();
    assert_eq!(loaded.metadata.get("test"), Some(&"value".to_string()));
}

#[cfg(feature = "yaml")]
#[test]
fn test_yaml_format() {
    use mockingbird::cassette::{load_cassette, save_cassette};
    
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.yaml");
    
    let mut cassette = Cassette::new();
    cassette.add(Interaction::new(
        RecordedRequest::new("GET", "https://example.com/api"),
        RecordedResponse::new(200).body("yaml response"),
    ));
    cassette.metadata.insert("format".to_string(), "yaml".to_string());
    
    // Save as YAML
    save_cassette(&path, &cassette).unwrap();
    
    // Verify it's actually YAML (not JSON)
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(!content.starts_with("{"), "Should be YAML, not JSON");
    assert!(content.contains("recorded_with:"), "Should contain YAML-style keys");
    
    // Load it back
    let loaded = load_cassette(&path).unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded.metadata.get("format"), Some(&"yaml".to_string()));
    assert_eq!(
        loaded.interactions[0].response.as_ref().unwrap().body,
        Some("yaml response".to_string())
    );
}

#[cfg(feature = "yaml")]
#[test]
fn test_yml_extension() {
    use mockingbird::cassette::{load_cassette, save_cassette};
    
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.yml");
    
    let cassette = Cassette::new();
    save_cassette(&path, &cassette).unwrap();
    
    // Verify .yml is treated as YAML
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(!content.starts_with("{"), "Should be YAML, not JSON");
    
    // Should load without error
    let loaded = load_cassette(&path).unwrap();
    assert!(loaded.is_empty());
}

#[test]
fn test_follow_redirects_option() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    // Test that follow_redirects(false) can be set
    let _client = Client::auto(&path)
        .follow_redirects(false)
        .build()
        .unwrap();
    
    // Test that follow_redirects(true) can be set (default)
    let _client2 = Client::auto(&path)
        .follow_redirects(true)
        .build()
        .unwrap();
}

#[test]
fn test_combined_options() {
    use mockingbird::matcher::NormalizedUrlMethodMatcher;
    use std::time::Duration;
    
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    // Test combining multiple options
    let _client = Client::auto(&path)
        .match_by(NormalizedUrlMethodMatcher)
        .filter_request_header("Authorization", "[REDACTED]")
        .filter_response_body_json("$.secret", "[FILTERED]")
        .expire_after(Duration::from_secs(86400))
        .record_errors(true)
        .follow_redirects(false)
        .build()
        .unwrap();
}

// Note: CLI prune command now supports duration strings like "30d", "1w", "24h", "2m"
// and uses --dry-run instead of --execute for showing what would be deleted
