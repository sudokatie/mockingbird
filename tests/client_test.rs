//! Client API tests.
//!
//! Tests for the main Client struct and its methods.

use mockingbird::cassette::{Cassette, Interaction, RecordedRequest, RecordedResponse, save_cassette};
use mockingbird::{Client, Error};
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn test_client_builders() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    // auto() builder
    let _client = Client::auto(&path).build().unwrap();
    
    // playback() builder
    let _client = Client::playback(&path).build().unwrap();
    
    // record() builder
    let _client = Client::record(&path).build().unwrap();
}

#[tokio::test]
async fn test_playback_no_match() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("empty.json");
    save_cassette(&path, &Cassette::new()).unwrap();
    
    let client = Client::playback(&path).build().unwrap();
    let result = client.get("https://example.com/missing").send().await;
    
    assert!(matches!(result, Err(Error::NoMatch)));
}

#[tokio::test]
async fn test_playback_returns_recorded() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    let mut cassette = Cassette::new();
    let req = RecordedRequest::new("GET", "https://example.com/api");
    let res = RecordedResponse::new(200).body("recorded response");
    cassette.add(Interaction::new(req, res));
    save_cassette(&path, &cassette).unwrap();
    
    let client = Client::playback(&path).build().unwrap();
    let resp = client.get("https://example.com/api").send().await.unwrap();
    
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().unwrap(), "recorded response");
}

#[tokio::test]
async fn test_all_http_methods() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    let mut cassette = Cassette::new();
    
    // Add interactions for each method
    for method in ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD"] {
        let req = RecordedRequest::new(method, format!("https://example.com/{}", method.to_lowercase()));
        let res = RecordedResponse::new(200).body(method);
        cassette.add(Interaction::new(req, res));
    }
    save_cassette(&path, &cassette).unwrap();
    
    let client = Client::playback(&path).build().unwrap();
    
    // Test each method
    assert_eq!(client.get("https://example.com/get").send().await.unwrap().text().unwrap(), "GET");
    assert_eq!(client.post("https://example.com/post").send().await.unwrap().text().unwrap(), "POST");
    assert_eq!(client.put("https://example.com/put").send().await.unwrap().text().unwrap(), "PUT");
    assert_eq!(client.delete("https://example.com/delete").send().await.unwrap().text().unwrap(), "DELETE");
    assert_eq!(client.patch("https://example.com/patch").send().await.unwrap().text().unwrap(), "PATCH");
    assert_eq!(client.head("https://example.com/head").send().await.unwrap().status(), 200);
}

#[tokio::test]
async fn test_custom_method() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    let mut cassette = Cassette::new();
    cassette.add(Interaction::new(
        RecordedRequest::new("OPTIONS", "https://example.com/cors"),
        RecordedResponse::new(204),
    ));
    save_cassette(&path, &cassette).unwrap();
    
    let client = Client::playback(&path).build().unwrap();
    let resp = client.request("OPTIONS", "https://example.com/cors").send().await.unwrap();
    
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_request_with_headers() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    let mut cassette = Cassette::new();
    cassette.add(Interaction::new(
        RecordedRequest::new("GET", "https://example.com/auth"),
        RecordedResponse::new(200).body("authenticated"),
    ));
    save_cassette(&path, &cassette).unwrap();
    
    let client = Client::playback(&path).build().unwrap();
    
    // Headers don't affect default matching
    let resp = client
        .get("https://example.com/auth")
        .header("Authorization", "Bearer token")
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.text().unwrap(), "authenticated");
}

#[tokio::test]
async fn test_request_with_json_body() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    let mut cassette = Cassette::new();
    cassette.add(Interaction::new(
        RecordedRequest::new("POST", "https://example.com/users"),
        RecordedResponse::new(201).body(r#"{"id":42}"#),
    ));
    save_cassette(&path, &cassette).unwrap();
    
    let client = Client::playback(&path).build().unwrap();
    
    #[derive(serde::Serialize)]
    struct User { name: String }
    
    let resp = client
        .post("https://example.com/users")
        .json(&User { name: "Alice".to_string() })
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 201);
}

#[tokio::test]
async fn test_request_with_query_params() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    let mut cassette = Cassette::new();
    cassette.add(Interaction::new(
        RecordedRequest::new("GET", "https://example.com/search?q=rust&page=1"),
        RecordedResponse::new(200).body("results"),
    ));
    save_cassette(&path, &cassette).unwrap();
    
    let client = Client::playback(&path).build().unwrap();
    
    let resp = client
        .get("https://example.com/search")
        .query(&[("q", "rust"), ("page", "1")])
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.text().unwrap(), "results");
}

#[test]
fn test_expire_after() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    let _client = Client::auto(&path)
        .expire_after(Duration::from_secs(3600))
        .build()
        .unwrap();
}

#[tokio::test]
async fn test_cassette_expired() {
    use chrono::{Duration as ChronoDuration, Utc};
    
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    let mut cassette = Cassette::new();
    let req = RecordedRequest::new("GET", "https://example.com/api");
    let res = RecordedResponse::new(200);
    let mut interaction = Interaction::new(req, res);
    interaction.recorded_at = Utc::now() - ChronoDuration::days(2);
    cassette.interactions.push(interaction);
    save_cassette(&path, &cassette).unwrap();
    
    let client = Client::playback(&path)
        .expire_after(Duration::from_secs(24 * 60 * 60)) // 1 day
        .build()
        .unwrap();
    
    let result = client.get("https://example.com/api").send().await;
    assert!(matches!(result, Err(Error::CassetteExpired { .. })));
}

#[test]
fn test_filter_convenience_methods() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    let _client = Client::auto(&path)
        .filter_request_header("Authorization", "[REDACTED]")
        .filter_response_header("Set-Cookie", "[REDACTED]")
        .filter_request_body_json("$.password", "[FILTERED]")
        .filter_response_body_json("$.api_key", "[FILTERED]")
        .build()
        .unwrap();
}

#[test]
fn test_record_errors_option() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    let _client = Client::auto(&path)
        .record_errors(true)
        .build()
        .unwrap();
}

#[test]
fn test_follow_redirects_option() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.json");
    
    // Disabled
    let _client = Client::auto(&path)
        .follow_redirects(false)
        .build()
        .unwrap();
    
    // Enabled (default)
    let _client = Client::auto(&path)
        .follow_redirects(true)
        .build()
        .unwrap();
}
