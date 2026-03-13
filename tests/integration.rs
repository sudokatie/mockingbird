//! Integration tests for mockingbird.

use mockingbird::cassette::{Cassette, Interaction, RecordedRequest, RecordedResponse, save_cassette};
use mockingbird::{Client, Mode};
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
