//! Middleware layer for integrating with existing HTTP clients.
//!
//! This module provides a Tower-style layer that can wrap existing
//! reqwest clients to add recording/playback functionality.

use crate::cassette::{Cassette, Interaction, RecordedRequest, RecordedResponse, RecordedError, ErrorKind, Header, BodyEncoding};
use crate::cassette::{load_cassette, save_cassette};
use crate::error::{Error, Result};

/// Result of attempting to playback a recorded interaction.
#[derive(Debug, Clone)]
pub enum PlaybackResult {
    /// A successful recorded response.
    Response(RecordedResponse),
    /// A recorded error to replay.
    Error(RecordedError),
}
use crate::filter::{RequestFilter, ResponseFilter};
use crate::matcher::{AllMatcher, Matcher};
use crate::mode::Mode;
use chrono::{Duration as ChronoDuration, Utc};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// A middleware layer for recording/replaying HTTP interactions.
/// 
/// This can be used to wrap an existing workflow where you need to
/// intercept and record/replay HTTP calls.
pub struct MockingbirdLayer {
    mode: Mode,
    cassette_path: PathBuf,
    cassette: Arc<RwLock<Cassette>>,
    matcher: Arc<dyn Matcher>,
    request_filters: Vec<Arc<dyn RequestFilter>>,
    response_filters: Vec<Arc<dyn ResponseFilter>>,
    expiration: Option<ChronoDuration>,
}

impl MockingbirdLayer {
    /// Create a new layer in playback mode.
    pub fn playback<P: AsRef<Path>>(cassette_path: P) -> LayerBuilder {
        LayerBuilder::new(cassette_path).mode(Mode::Replay)
    }
    
    /// Create a new layer in record mode.
    pub fn record<P: AsRef<Path>>(cassette_path: P) -> LayerBuilder {
        LayerBuilder::new(cassette_path).mode(Mode::Record)
    }
    
    /// Create a new layer in auto mode.
    pub fn auto<P: AsRef<Path>>(cassette_path: P) -> LayerBuilder {
        LayerBuilder::new(cassette_path).mode(Mode::Auto)
    }
    
    /// Try to find a matching recorded interaction for a request.
    /// 
    /// Returns `Ok(Some(PlaybackResult::Response(...)))` for successful responses,
    /// `Ok(Some(PlaybackResult::Error(...)))` for recorded errors,
    /// or `Ok(None)` if no match is found.
    pub fn try_playback(&self, request: &RecordedRequest) -> Result<Option<PlaybackResult>> {
        let mut filtered_request = request.clone();
        for filter in &self.request_filters {
            filter.filter(&mut filtered_request);
        }
        
        let cassette = self.cassette.read()
            .map_err(|_| Error::Config("Lock poisoned".into()))?;
        
        for interaction in &cassette.interactions {
            if self.matcher.matches(&interaction.request, &filtered_request) {
                // Check expiration
                if let Some(max_age) = self.expiration {
                    let age = Utc::now() - interaction.recorded_at;
                    if age > max_age {
                        return Err(Error::CassetteExpired {
                            recorded_at: interaction.recorded_at,
                            max_age,
                        });
                    }
                }
                // Return successful response if present
                if let Some(response) = &interaction.response {
                    return Ok(Some(PlaybackResult::Response(response.clone())));
                }
                // Return recorded error if present
                if let Some(error) = &interaction.error {
                    return Ok(Some(PlaybackResult::Error(error.clone())));
                }
            }
        }
        
        Ok(None)
    }
    
    /// Convert a PlaybackResult to a Result, returning the response or error.
    /// 
    /// Use this to convert a recorded error back into an actual Error.
    pub fn playback_result_to_response(&self, result: PlaybackResult) -> Result<RecordedResponse> {
        match result {
            PlaybackResult::Response(response) => Ok(response),
            PlaybackResult::Error(error) => Err(Self::recorded_error_to_error(&error)),
        }
    }
    
    /// Convert a RecordedError to an Error for replay.
    fn recorded_error_to_error(error: &RecordedError) -> Error {
        match error.kind {
            ErrorKind::Timeout => Error::RecordedTimeout { message: error.message.clone() },
            ErrorKind::Connection => Error::RecordedConnection { message: error.message.clone() },
            ErrorKind::Dns => Error::RecordedDns { message: error.message.clone() },
            ErrorKind::Tls => Error::RecordedTls { message: error.message.clone() },
            ErrorKind::Cancelled => Error::RecordedCancelled { message: error.message.clone() },
            ErrorKind::Unknown => Error::RecordedUnknown { message: error.message.clone() },
        }
    }
    
    /// Record a new interaction.
    pub fn record_interaction(&self, request: RecordedRequest, response: RecordedResponse) -> Result<()> {
        let mut filtered_request = request;
        for filter in &self.request_filters {
            filter.filter(&mut filtered_request);
        }
        
        let mut filtered_response = response;
        for filter in &self.response_filters {
            filter.filter(&mut filtered_response);
        }
        
        let interaction = Interaction::new(filtered_request, filtered_response);
        
        {
            let mut cassette = self.cassette.write()
                .map_err(|_| Error::Config("Lock poisoned".into()))?;
            cassette.add(interaction);
        }
        
        self.save()
    }
    
    /// Save the cassette to disk.
    pub fn save(&self) -> Result<()> {
        let cassette = self.cassette.read()
            .map_err(|_| Error::Config("Lock poisoned".into()))?;
        save_cassette(&self.cassette_path, &cassette)
    }
    
    /// Get the current mode.
    pub fn mode(&self) -> Mode {
        self.mode
    }
    
    /// Check if playback should be attempted.
    pub fn should_playback(&self) -> bool {
        self.mode.replays()
    }
    
    /// Check if recording should be performed.
    pub fn should_record(&self) -> bool {
        self.mode.records()
    }
    
    /// Process a request through the layer.
    /// 
    /// Returns `Some(response)` if a cached response was found (playback mode),
    /// or `None` if the request should be forwarded to the real server.
    /// 
    /// If a recorded error is found, it will be returned as an `Err`.
    pub fn process_request(&self, request: &RecordedRequest) -> Result<Option<RecordedResponse>> {
        match self.mode {
            Mode::Replay => {
                match self.try_playback(request)? {
                    Some(PlaybackResult::Response(response)) => Ok(Some(response)),
                    Some(PlaybackResult::Error(error)) => Err(Self::recorded_error_to_error(&error)),
                    None => Err(Error::NoMatch),
                }
            }
            Mode::Auto => {
                match self.try_playback(request)? {
                    Some(PlaybackResult::Response(response)) => Ok(Some(response)),
                    Some(PlaybackResult::Error(error)) => Err(Self::recorded_error_to_error(&error)),
                    None => Ok(None),
                }
            }
            Mode::Record | Mode::Passthrough => {
                Ok(None)
            }
        }
    }
    
    /// Record a response after forwarding (for record/auto modes).
    pub fn process_response(&self, request: RecordedRequest, response: RecordedResponse) -> Result<RecordedResponse> {
        if self.should_record() {
            self.record_interaction(request, response.clone())?;
        }
        Ok(response)
    }
}

/// Builder for MockingbirdLayer.
pub struct LayerBuilder {
    cassette_path: PathBuf,
    mode: Mode,
    matcher: Option<Arc<dyn Matcher>>,
    request_filters: Vec<Arc<dyn RequestFilter>>,
    response_filters: Vec<Arc<dyn ResponseFilter>>,
    expiration: Option<ChronoDuration>,
}

impl LayerBuilder {
    /// Create a new builder.
    pub fn new<P: AsRef<Path>>(cassette_path: P) -> Self {
        Self {
            cassette_path: cassette_path.as_ref().to_path_buf(),
            mode: Mode::Auto,
            matcher: None,
            request_filters: Vec::new(),
            response_filters: Vec::new(),
            expiration: None,
        }
    }
    
    /// Set the operating mode.
    pub fn mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
    }
    
    /// Set a custom matcher.
    pub fn matcher(mut self, matcher: impl Matcher + 'static) -> Self {
        self.matcher = Some(Arc::new(matcher));
        self
    }
    
    /// Add a request filter.
    pub fn request_filter(mut self, filter: impl RequestFilter + 'static) -> Self {
        self.request_filters.push(Arc::new(filter));
        self
    }
    
    /// Add a response filter.
    pub fn response_filter(mut self, filter: impl ResponseFilter + 'static) -> Self {
        self.response_filters.push(Arc::new(filter));
        self
    }
    
    /// Set cassette expiration.
    pub fn expire_after(mut self, duration: Duration) -> Self {
        self.expiration = Some(ChronoDuration::from_std(duration).unwrap_or(ChronoDuration::MAX));
        self
    }
    
    /// Build the layer.
    pub fn build(self) -> Result<MockingbirdLayer> {
        let cassette = if self.cassette_path.exists() {
            load_cassette(&self.cassette_path)?
        } else {
            Cassette::new()
        };
        
        let matcher = self.matcher
            .unwrap_or_else(|| Arc::new(AllMatcher::default_matchers()));
        
        Ok(MockingbirdLayer {
            mode: self.mode,
            cassette_path: self.cassette_path,
            cassette: Arc::new(RwLock::new(cassette)),
            matcher,
            request_filters: self.request_filters,
            response_filters: self.response_filters,
            expiration: self.expiration,
        })
    }
}

/// Helper functions for converting reqwest types to recorded types.
pub mod convert {
    use super::*;
    
    /// Convert a reqwest Request to a RecordedRequest.
    pub fn request_to_recorded(req: &reqwest::Request) -> RecordedRequest {
        let method = req.method().to_string();
        let url = req.url().to_string();
        
        let headers: Vec<Header> = req
            .headers()
            .iter()
            .map(|(k, v)| Header::new(k.as_str(), v.to_str().unwrap_or("")))
            .collect();
        
        let mut recorded = RecordedRequest::new(&method, &url);
        recorded.headers = headers;
        
        // Note: reqwest::Request body is not easily accessible
        // In practice, you'd capture the body before building the request
        
        recorded
    }
    
    /// Convert a reqwest Response to a RecordedResponse.
    pub async fn response_to_recorded(resp: reqwest::Response) -> Result<(RecordedResponse, bytes::Bytes)> {
        let status = resp.status().as_u16();
        let headers: Vec<Header> = resp
            .headers()
            .iter()
            .map(|(k, v)| Header::new(k.as_str(), v.to_str().unwrap_or("")))
            .collect();
        
        let body_bytes = resp.bytes().await.map_err(Error::Http)?;
        
        let mut recorded = RecordedResponse::new(status);
        recorded.headers = headers;
        
        if !body_bytes.is_empty() {
            if let Ok(text) = String::from_utf8(body_bytes.to_vec()) {
                recorded.body = Some(text);
            } else {
                use base64::Engine;
                recorded.body = Some(base64::engine::general_purpose::STANDARD.encode(&body_bytes));
                recorded.body_encoding = BodyEncoding::Base64;
            }
        }
        
        Ok((recorded, body_bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_layer_builder() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let layer = MockingbirdLayer::auto(&path).build().unwrap();
        assert!(matches!(layer.mode(), Mode::Auto));
    }

    #[test]
    fn test_playback_layer() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let layer = MockingbirdLayer::playback(&path).build().unwrap();
        assert!(matches!(layer.mode(), Mode::Replay));
        assert!(layer.should_playback());
        assert!(!layer.should_record());
    }

    #[test]
    fn test_record_layer() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let layer = MockingbirdLayer::record(&path).build().unwrap();
        assert!(matches!(layer.mode(), Mode::Record));
        assert!(!layer.should_playback());
        assert!(layer.should_record());
    }

    #[test]
    fn test_try_playback_empty() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let layer = MockingbirdLayer::playback(&path).build().unwrap();
        let req = RecordedRequest::new("GET", "https://example.com");
        
        let result = layer.try_playback(&req).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_record_and_playback() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        // Record
        {
            let layer = MockingbirdLayer::record(&path).build().unwrap();
            let req = RecordedRequest::new("GET", "https://example.com/api");
            let resp = RecordedResponse::new(200).body("recorded");
            layer.record_interaction(req, resp).unwrap();
        }
        
        // Playback
        {
            let layer = MockingbirdLayer::playback(&path).build().unwrap();
            let req = RecordedRequest::new("GET", "https://example.com/api");
            let result = layer.try_playback(&req).unwrap();
            
            assert!(result.is_some());
            match result.unwrap() {
                PlaybackResult::Response(resp) => {
                    assert_eq!(resp.status, 200);
                    assert_eq!(resp.body, Some("recorded".to_string()));
                }
                PlaybackResult::Error(_) => panic!("expected response, got error"),
            }
        }
    }

    #[test]
    fn test_process_request_replay_mode() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        // Create cassette with interaction
        let mut cassette = Cassette::new();
        cassette.add(Interaction::new(
            RecordedRequest::new("GET", "https://example.com"),
            RecordedResponse::new(200).body("cached"),
        ));
        save_cassette(&path, &cassette).unwrap();
        
        let layer = MockingbirdLayer::playback(&path).build().unwrap();
        let req = RecordedRequest::new("GET", "https://example.com");
        
        let result = layer.process_request(&req).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_process_request_record_mode() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let layer = MockingbirdLayer::record(&path).build().unwrap();
        let req = RecordedRequest::new("GET", "https://example.com");
        
        // In record mode, process_request returns None (forward to real server)
        let result = layer.process_request(&req).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_auto_mode_fallback() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let layer = MockingbirdLayer::auto(&path).build().unwrap();
        let req = RecordedRequest::new("GET", "https://example.com/new");
        
        // Auto mode returns None when no match (should forward)
        let result = layer.process_request(&req).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_error_interaction_playback() {
        use crate::cassette::RecordedError;
        
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        // Create cassette with error interaction
        let mut cassette = Cassette::new();
        cassette.add(Interaction::error(
            RecordedRequest::new("GET", "https://timeout.example.com"),
            RecordedError::timeout("connection timed out"),
        ));
        save_cassette(&path, &cassette).unwrap();
        
        let layer = MockingbirdLayer::playback(&path).build().unwrap();
        let req = RecordedRequest::new("GET", "https://timeout.example.com");
        
        // try_playback should return the error
        let result = layer.try_playback(&req).unwrap();
        assert!(result.is_some());
        match result.unwrap() {
            PlaybackResult::Error(err) => {
                assert_eq!(err.kind, ErrorKind::Timeout);
                assert!(err.message.contains("timed out"));
            }
            PlaybackResult::Response(_) => panic!("expected error, got response"),
        }
        
        // process_request should convert to Error
        let result = layer.process_request(&req);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timed out"));
    }
}
