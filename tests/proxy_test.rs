//! Proxy server tests.
//!
//! Tests for the standalone HTTP proxy server functionality.

use mockingbird::mode::Mode;
use mockingbird::ProxyConfig;

#[test]
fn test_proxy_config_builder() {
    let config = ProxyConfig::new(8080, Mode::Record, "cassettes/test.json");
    
    assert_eq!(config.port, 8080);
    assert!(matches!(config.mode, Mode::Record));
    assert!(config.target_url.is_none());
}

#[test]
fn test_proxy_config_with_target() {
    let config = ProxyConfig::new(8080, Mode::Record, "cassettes/test.json")
        .target("https://api.example.com");
    
    assert_eq!(config.port, 8080);
    assert!(matches!(config.mode, Mode::Record));
    assert_eq!(config.target_url, Some("https://api.example.com".to_string()));
}

#[test]
fn test_proxy_config_modes() {
    // Record mode
    let config = ProxyConfig::new(8080, Mode::Record, "test.json");
    assert!(config.mode.records());
    assert!(!config.mode.replays());
    
    // Replay mode
    let config = ProxyConfig::new(8080, Mode::Replay, "test.json");
    assert!(!config.mode.records());
    assert!(config.mode.replays());
    
    // Auto mode
    let config = ProxyConfig::new(8080, Mode::Auto, "test.json");
    assert!(config.mode.records());
    assert!(config.mode.replays());
    
    // Passthrough mode
    let config = ProxyConfig::new(8080, Mode::Passthrough, "test.json");
    assert!(!config.mode.records());
    assert!(!config.mode.replays());
    assert!(config.mode.allows_real_requests());
}

#[test]
fn test_proxy_config_cassette_path() {
    let config = ProxyConfig::new(8080, Mode::Replay, "fixtures/api.json");
    
    assert_eq!(config.cassette_path.to_string_lossy(), "fixtures/api.json");
}

// Integration tests for proxy server would typically use wiremock
// to create a mock target server. These are more complex and would
// be in a separate integration test file.

#[cfg(test)]
mod integration_tests {
    use super::*;
    use mockingbird::cassette::{Cassette, Interaction, RecordedRequest, RecordedResponse, save_cassette};
    use tempfile::TempDir;
    
    // Note: Full proxy integration tests require actually running the server
    // and making HTTP requests. These are placeholder tests that verify
    // configuration works correctly.
    
    #[test]
    fn test_proxy_cassette_loading() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        // Create a cassette
        let mut cassette = Cassette::new();
        cassette.add(Interaction::new(
            RecordedRequest::new("GET", "https://api.example.com/users"),
            RecordedResponse::new(200).body("[]"),
        ));
        save_cassette(&path, &cassette).unwrap();
        
        // Create proxy config pointing to it
        let config = ProxyConfig::new(0, Mode::Replay, &path);
        
        assert!(config.cassette_path.exists());
    }
    
    #[tokio::test]
    async fn test_proxy_config_allows_non_existent_cassette() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("new.json");
        
        // Should be able to create config with non-existent cassette
        // (will be created when first interaction is recorded)
        let _config = ProxyConfig::new(0, Mode::Record, &path);
        
        assert!(!path.exists());
    }
}

#[cfg(test)]
mod wiremock_tests {
    // These tests use wiremock to create a mock server for testing
    // the proxy's recording functionality.
    
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, path};
    use mockingbird::cassette::{Cassette, Interaction, RecordedRequest, RecordedResponse, save_cassette, load_cassette};
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_mock_server_setup() {
        // Verify wiremock is working
        let mock_server = MockServer::start().await;
        
        Mock::given(method("GET"))
            .and(path("/api/users"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": []
            })))
            .mount(&mock_server)
            .await;
        
        // Make a request to the mock server
        let client = reqwest::Client::new();
        let response = client
            .get(format!("{}/api/users", mock_server.uri()))
            .send()
            .await
            .unwrap();
        
        assert_eq!(response.status(), 200);
        
        let body: serde_json::Value = response.json().await.unwrap();
        assert!(body.get("users").is_some());
    }
    
    #[tokio::test]
    async fn test_cassette_can_store_mock_response() {
        let mock_server = MockServer::start().await;
        
        Mock::given(method("GET"))
            .and(path("/api/data"))
            .respond_with(ResponseTemplate::new(200).set_body_string("mock data"))
            .mount(&mock_server)
            .await;
        
        // Create cassette from mock response
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("mock.json");
        
        let mut cassette = Cassette::new();
        cassette.add(Interaction::new(
            RecordedRequest::new("GET", format!("{}/api/data", mock_server.uri())),
            RecordedResponse::new(200).body("mock data"),
        ));
        save_cassette(&path, &cassette).unwrap();
        
        // Verify cassette was saved correctly
        let loaded = load_cassette(&path).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(
            loaded.interactions[0].response.as_ref().unwrap().body,
            Some("mock data".to_string())
        );
    }
}
