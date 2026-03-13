//! Mockingbird client.
//!
//! The main client that handles recording and playback of HTTP interactions.

use crate::cassette::{Cassette, Interaction, load_cassette, save_cassette};
use crate::error::{Error, Result};
use crate::filter::{RequestFilter, ResponseFilter};
use crate::matcher::{AllMatcher, Matcher};
use crate::mode::Mode;
use crate::request::Request;
use crate::response::Response;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Mockingbird HTTP client.
pub struct Client {
    mode: Mode,
    cassette_path: PathBuf,
    cassette: Arc<RwLock<Cassette>>,
    matcher: Box<dyn Matcher>,
    request_filters: Vec<Box<dyn RequestFilter>>,
    response_filters: Vec<Box<dyn ResponseFilter>>,
    http_client: reqwest::Client,
}

impl Client {
    /// Create a new client builder for recording.
    pub fn record<P: AsRef<Path>>(cassette_path: P) -> ClientBuilder {
        ClientBuilder::new(cassette_path).mode(Mode::Record)
    }
    
    /// Create a new client builder for playback.
    pub fn playback<P: AsRef<Path>>(cassette_path: P) -> ClientBuilder {
        ClientBuilder::new(cassette_path).mode(Mode::Replay)
    }
    
    /// Create a new client builder with auto mode.
    pub fn auto<P: AsRef<Path>>(cassette_path: P) -> ClientBuilder {
        ClientBuilder::new(cassette_path).mode(Mode::Auto)
    }
    
    /// Create a GET request.
    pub fn get(&self, url: impl Into<String>) -> RequestBuilder {
        RequestBuilder::new(self, Request::get(url))
    }
    
    /// Create a POST request.
    pub fn post(&self, url: impl Into<String>) -> RequestBuilder {
        RequestBuilder::new(self, Request::post(url))
    }
    
    /// Create a PUT request.
    pub fn put(&self, url: impl Into<String>) -> RequestBuilder {
        RequestBuilder::new(self, Request::put(url))
    }
    
    /// Create a DELETE request.
    pub fn delete(&self, url: impl Into<String>) -> RequestBuilder {
        RequestBuilder::new(self, Request::delete(url))
    }
    
    /// Execute a request.
    pub async fn execute(&self, request: Request) -> Result<Response> {
        match self.mode {
            Mode::Replay => self.playback_request(request),
            Mode::Record => self.record_request(request).await,
            Mode::Auto => self.auto_request(request).await,
            Mode::Passthrough => self.passthrough_request(request).await,
        }
    }
    
    /// Save the cassette to disk.
    pub fn save(&self) -> Result<()> {
        let cassette = self.cassette.read().map_err(|_| Error::Config("Lock poisoned".into()))?;
        save_cassette(&self.cassette_path, &cassette)
    }
    
    fn playback_request(&self, request: Request) -> Result<Response> {
        let mut recorded_request = request.to_recorded();
        
        // Apply request filters
        for filter in &self.request_filters {
            filter.filter(&mut recorded_request);
        }
        
        // Find matching interaction
        let cassette = self.cassette.read().map_err(|_| Error::Config("Lock poisoned".into()))?;
        
        for interaction in &cassette.interactions {
            if self.matcher.matches(&interaction.request, &recorded_request) {
                return Ok(Response::from_recorded(interaction.response.clone()));
            }
        }
        
        Err(Error::NoMatch)
    }
    
    async fn record_request(&self, request: Request) -> Result<Response> {
        let mut recorded_request = request.to_recorded();
        
        // Make real request
        let real_response = self.make_real_request(&request).await?;
        let response = Response::from_recorded(real_response.clone());
        
        // Apply filters
        for filter in &self.request_filters {
            filter.filter(&mut recorded_request);
        }
        let mut recorded_response = real_response;
        for filter in &self.response_filters {
            filter.filter(&mut recorded_response);
        }
        
        // Store interaction
        let interaction = Interaction::new(recorded_request, recorded_response);
        {
            let mut cassette = self.cassette.write().map_err(|_| Error::Config("Lock poisoned".into()))?;
            cassette.add(interaction);
        }
        
        // Auto-save
        self.save()?;
        
        Ok(response)
    }
    
    async fn auto_request(&self, request: Request) -> Result<Response> {
        // Try playback first
        if let Ok(response) = self.playback_request(request.clone()) {
            return Ok(response);
        }
        
        // Fall back to recording
        self.record_request(request).await
    }
    
    async fn passthrough_request(&self, request: Request) -> Result<Response> {
        let real_response = self.make_real_request(&request).await?;
        Ok(Response::from_recorded(real_response))
    }
    
    async fn make_real_request(&self, request: &Request) -> Result<crate::cassette::RecordedResponse> {
        let mut builder = self.http_client.request(
            request.method().parse().unwrap_or(reqwest::Method::GET),
            request.url(),
        );
        
        // Add headers
        for (name, value) in request.headers() {
            builder = builder.header(name, value);
        }
        
        // Add body
        if let Some(body) = request.get_body() {
            builder = builder.body(body.to_vec());
        }
        
        let response = builder.send().await?;
        
        // Convert to recorded response
        let status = response.status().as_u16();
        let headers: Vec<_> = response
            .headers()
            .iter()
            .map(|(k, v)| crate::cassette::Header::new(k.as_str(), v.to_str().unwrap_or("")))
            .collect();
        let body = response.bytes().await?;
        
        let mut recorded = crate::cassette::RecordedResponse::new(status);
        recorded.headers = headers;
        if !body.is_empty() {
            if let Ok(text) = String::from_utf8(body.to_vec()) {
                recorded.body = Some(text);
            } else {
                use base64::Engine;
                recorded.body = Some(base64::engine::general_purpose::STANDARD.encode(&body));
                recorded.body_encoding = crate::cassette::BodyEncoding::Base64;
            }
        }
        
        Ok(recorded)
    }
}

/// Builder for configuring a Client.
pub struct ClientBuilder {
    cassette_path: PathBuf,
    mode: Mode,
    matcher: Option<Box<dyn Matcher>>,
    request_filters: Vec<Box<dyn RequestFilter>>,
    response_filters: Vec<Box<dyn ResponseFilter>>,
}

impl ClientBuilder {
    /// Create a new builder.
    pub fn new<P: AsRef<Path>>(cassette_path: P) -> Self {
        Self {
            cassette_path: cassette_path.as_ref().to_path_buf(),
            mode: Mode::Auto,
            matcher: None,
            request_filters: Vec::new(),
            response_filters: Vec::new(),
        }
    }
    
    /// Set the operating mode.
    pub fn mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
    }
    
    /// Set a custom matcher.
    pub fn matcher(mut self, matcher: impl Matcher + 'static) -> Self {
        self.matcher = Some(Box::new(matcher));
        self
    }
    
    /// Add a request filter.
    pub fn request_filter(mut self, filter: impl RequestFilter + 'static) -> Self {
        self.request_filters.push(Box::new(filter));
        self
    }
    
    /// Add a response filter.
    pub fn response_filter(mut self, filter: impl ResponseFilter + 'static) -> Self {
        self.response_filters.push(Box::new(filter));
        self
    }
    
    /// Build the client.
    pub fn build(self) -> Result<Client> {
        let cassette = if self.cassette_path.exists() {
            load_cassette(&self.cassette_path)?
        } else {
            Cassette::new()
        };
        
        let matcher = self.matcher.unwrap_or_else(|| Box::new(AllMatcher::default_matchers()));
        
        Ok(Client {
            mode: self.mode,
            cassette_path: self.cassette_path,
            cassette: Arc::new(RwLock::new(cassette)),
            matcher,
            request_filters: self.request_filters,
            response_filters: self.response_filters,
            http_client: reqwest::Client::new(),
        })
    }
}

/// Builder for a single request.
pub struct RequestBuilder<'a> {
    client: &'a Client,
    request: Request,
}

impl<'a> RequestBuilder<'a> {
    fn new(client: &'a Client, request: Request) -> Self {
        Self { client, request }
    }
    
    /// Add a header.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.request = self.request.header(name, value);
        self
    }
    
    /// Set the body.
    pub fn body(mut self, body: impl Into<bytes::Bytes>) -> Self {
        self.request = self.request.body(body);
        self
    }
    
    /// Set JSON body.
    pub fn json<T: serde::Serialize>(mut self, value: &T) -> Self {
        self.request = self.request.json(value);
        self
    }
    
    /// Send the request.
    pub async fn send(self) -> Result<Response> {
        self.client.execute(self.request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_client_builder() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let client = Client::auto(&path).build().unwrap();
        assert!(matches!(client.mode, Mode::Auto));
    }

    #[test]
    fn test_playback_mode() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let client = Client::playback(&path).build().unwrap();
        assert!(matches!(client.mode, Mode::Replay));
    }

    #[test]
    fn test_record_mode() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        
        let client = Client::record(&path).build().unwrap();
        assert!(matches!(client.mode, Mode::Record));
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
        
        // Create cassette with one interaction
        let mut cassette = Cassette::new();
        let req = crate::cassette::RecordedRequest::new("GET", "https://example.com/api");
        let res = crate::cassette::RecordedResponse::new(200).body("recorded response");
        cassette.add(Interaction::new(req, res));
        save_cassette(&path, &cassette).unwrap();
        
        // Playback
        let client = Client::playback(&path).build().unwrap();
        let resp = client.get("https://example.com/api").send().await.unwrap();
        
        assert_eq!(resp.status(), 200);
        assert_eq!(resp.text().unwrap(), "recorded response");
    }
}
