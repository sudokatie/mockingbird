//! Proxy server for record/replay mode.
//!
//! Runs an HTTP server that proxies requests to a target server (record mode)
//! or serves responses from a cassette (playback mode).

use crate::cassette::{Cassette, Interaction, RecordedRequest, RecordedResponse, Header, BodyEncoding};
use crate::cassette::{load_cassette, save_cassette};
use crate::error::{Error, Result};
use crate::matcher::{AllMatcher, Matcher};
use crate::mode::Mode;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tokio::net::TcpListener;

/// Proxy server configuration.
pub struct ProxyConfig {
    /// Port to listen on.
    pub port: u16,
    /// Operating mode.
    pub mode: Mode,
    /// Path to cassette file.
    pub cassette_path: PathBuf,
    /// Target URL for record mode (optional).
    pub target_url: Option<String>,
}

impl ProxyConfig {
    /// Create a new proxy config.
    pub fn new<P: AsRef<Path>>(port: u16, mode: Mode, cassette_path: P) -> Self {
        Self {
            port,
            mode,
            cassette_path: cassette_path.as_ref().to_path_buf(),
            target_url: None,
        }
    }
    
    /// Set the target URL for record mode.
    pub fn target(mut self, url: impl Into<String>) -> Self {
        self.target_url = Some(url.into());
        self
    }
}

/// Shared state for the proxy server.
struct ProxyState {
    mode: Mode,
    cassette: RwLock<Cassette>,
    cassette_path: PathBuf,
    target_url: Option<String>,
    matcher: Box<dyn Matcher>,
    http_client: reqwest::Client,
}

/// Run the proxy server.
pub async fn run(config: ProxyConfig) -> Result<()> {
    let cassette = if config.cassette_path.exists() {
        load_cassette(&config.cassette_path)?
    } else {
        Cassette::new()
    };
    
    let state = Arc::new(ProxyState {
        mode: config.mode,
        cassette: RwLock::new(cassette),
        cassette_path: config.cassette_path,
        target_url: config.target_url,
        matcher: Box::new(AllMatcher::default_matchers()),
        http_client: reqwest::Client::new(),
    });
    
    let addr = SocketAddr::from(([127, 0, 0, 1], config.port));
    let listener = TcpListener::bind(addr).await
        .map_err(|e| Error::Proxy(format!("Failed to bind to {}: {}", addr, e)))?;
    
    println!("Proxy listening on http://{}", addr);
    println!("Mode: {}", config.mode);
    
    loop {
        let (stream, _) = listener.accept().await
            .map_err(|e| Error::Proxy(format!("Accept failed: {}", e)))?;
        
        let io = TokioIo::new(stream);
        let state = Arc::clone(&state);
        
        tokio::spawn(async move {
            let service = service_fn(move |req| {
                let state = Arc::clone(&state);
                handle_request(state, req)
            });
            
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service)
                .await
            {
                eprintln!("Connection error: {}", err);
            }
        });
    }
}

async fn handle_request(
    state: Arc<ProxyState>,
    req: Request<hyper::body::Incoming>,
) -> std::result::Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let result = match state.mode {
        Mode::Replay => handle_playback(&state, req).await,
        Mode::Record => handle_record(&state, req).await,
        Mode::Auto => handle_auto(&state, req).await,
        Mode::Passthrough => handle_passthrough(&state, req).await,
    };
    
    match result {
        Ok(resp) => Ok(resp),
        Err(e) => {
            eprintln!("Error: {}", e);
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::new(Bytes::from(format!("Error: {}", e))))
                .unwrap())
        }
    }
}

async fn handle_playback(
    state: &ProxyState,
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>> {
    let recorded_request = hyper_to_recorded(req).await?;
    
    let cassette = state.cassette.read()
        .map_err(|_| Error::Config("Lock poisoned".into()))?;
    
    for interaction in &cassette.interactions {
        if state.matcher.matches(&interaction.request, &recorded_request) {
            // Handle error interactions
            if interaction.is_error() {
                if let Some(err) = &interaction.error {
                    return Err(Error::Proxy(format!("Recorded error: {}", err.message)));
                }
            }
            // Handle successful interactions
            if let Some(response) = &interaction.response {
                return recorded_to_hyper(response);
            }
        }
    }
    
    Err(Error::NoMatch)
}

async fn handle_record(
    state: &ProxyState,
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>> {
    let recorded_request = hyper_to_recorded(req).await?;
    
    // Use target URL from config, or require full URL in path
    let target_url = state.target_url.as_ref().map(|s| s.as_str());
    
    // If URL is already fully qualified (from path extraction), we can use it directly
    let is_full_url = recorded_request.url.starts_with("http://") || recorded_request.url.starts_with("https://");
    if !is_full_url && target_url.is_none() {
        return Err(Error::Config(
            "Target URL required. Use --target or send requests to /https://target.com/path".into()
        ));
    }
    
    // Forward to target (use full URL from path, or build with target_url)
    let recorded_response = forward_request_smart(&state.http_client, target_url, &recorded_request).await?;
    
    // Store interaction
    {
        let mut cassette = state.cassette.write()
            .map_err(|_| Error::Config("Lock poisoned".into()))?;
        cassette.add(Interaction::new(recorded_request.clone(), recorded_response.clone()));
    }
    
    // Save cassette
    {
        let cassette = state.cassette.read()
            .map_err(|_| Error::Config("Lock poisoned".into()))?;
        save_cassette(&state.cassette_path, &cassette)?;
    }
    
    println!("Recorded: {} {}", recorded_request.method, recorded_request.url);
    
    recorded_to_hyper(&recorded_response)
}

async fn handle_auto(
    state: &ProxyState,
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>> {
    // Try playback first
    let recorded_request = hyper_to_recorded(req).await?;
    
    {
        let cassette = state.cassette.read()
            .map_err(|_| Error::Config("Lock poisoned".into()))?;
        
        for interaction in &cassette.interactions {
            if state.matcher.matches(&interaction.request, &recorded_request) {
                println!("Replaying: {} {}", recorded_request.method, recorded_request.url);
                // Handle error interactions
                if interaction.is_error() {
                    if let Some(err) = &interaction.error {
                        return Err(Error::Proxy(format!("Recorded error: {}", err.message)));
                    }
                }
                // Handle successful interactions
                if let Some(response) = &interaction.response {
                    return recorded_to_hyper(response);
                }
            }
        }
    }
    
    // Fall back to recording - use target URL from config or require full URL in path
    let target_url = state.target_url.as_ref().map(|s| s.as_str());
    let is_full_url = recorded_request.url.starts_with("http://") || recorded_request.url.starts_with("https://");
    if !is_full_url && target_url.is_none() {
        return Err(Error::Config(
            "Target URL required for recording. Use --target or send requests to /https://target.com/path".into()
        ));
    }
    
    let recorded_response = forward_request_smart(&state.http_client, target_url, &recorded_request).await?;
    
    // Store interaction
    {
        let mut cassette = state.cassette.write()
            .map_err(|_| Error::Config("Lock poisoned".into()))?;
        cassette.add(Interaction::new(recorded_request.clone(), recorded_response.clone()));
    }
    
    // Save cassette
    {
        let cassette = state.cassette.read()
            .map_err(|_| Error::Config("Lock poisoned".into()))?;
        save_cassette(&state.cassette_path, &cassette)?;
    }
    
    println!("Recorded: {} {}", recorded_request.method, recorded_request.url);
    
    recorded_to_hyper(&recorded_response)
}

async fn handle_passthrough(
    state: &ProxyState,
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>> {
    let recorded_request = hyper_to_recorded(req).await?;
    
    let target_url = state.target_url.as_ref().map(|s| s.as_str());
    let is_full_url = recorded_request.url.starts_with("http://") || recorded_request.url.starts_with("https://");
    if !is_full_url && target_url.is_none() {
        return Err(Error::Config(
            "Target URL required. Use --target or send requests to /https://target.com/path".into()
        ));
    }
    
    let recorded_response = forward_request_smart(&state.http_client, target_url, &recorded_request).await?;
    
    recorded_to_hyper(&recorded_response)
}

/// Extract target URL from path if it contains a full URL.
/// 
/// Supports path-based URL extraction per spec:
/// - `/https://api.example.com/users` -> `https://api.example.com/users`
/// - `/http://api.example.com/users` -> `http://api.example.com/users`
/// 
/// Returns None if path doesn't contain a full URL.
fn extract_url_from_path(path: &str) -> Option<String> {
    // Remove leading slash and check for protocol
    let trimmed = path.trim_start_matches('/');
    if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
        Some(trimmed.to_string())
    } else {
        None
    }
}

async fn hyper_to_recorded(req: Request<hyper::body::Incoming>) -> Result<RecordedRequest> {
    let method = req.method().to_string();
    let uri = req.uri();
    
    // Try to extract full URL from path (per spec section 8.4)
    // e.g., /https://api.example.com/users -> https://api.example.com/users
    let url = if let Some(path) = uri.path_and_query().map(|pq| pq.as_str()) {
        extract_url_from_path(path).unwrap_or_else(|| uri.to_string())
    } else {
        uri.to_string()
    };
    
    let headers: Vec<Header> = req
        .headers()
        .iter()
        .map(|(k, v)| Header::new(k.as_str(), v.to_str().unwrap_or("")))
        .collect();
    
    let body_bytes = req.collect().await
        .map_err(|e| Error::Proxy(format!("Failed to read request body: {}", e)))?
        .to_bytes();
    
    let mut recorded = RecordedRequest::new(&method, &url);
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
    
    Ok(recorded)
}

fn recorded_to_hyper(resp: &RecordedResponse) -> Result<Response<Full<Bytes>>> {
    let mut builder = Response::builder()
        .status(StatusCode::from_u16(resp.status).unwrap_or(StatusCode::OK));
    
    for header in &resp.headers {
        builder = builder.header(&header.name, &header.value);
    }
    
    let body = match &resp.body {
        Some(text) => {
            match resp.body_encoding {
                BodyEncoding::Text => Full::new(Bytes::from(text.clone())),
                BodyEncoding::Base64 => {
                    use base64::Engine;
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(text)
                        .map_err(|e| Error::Proxy(format!("Failed to decode base64: {}", e)))?;
                    Full::new(Bytes::from(bytes))
                }
            }
        }
        None => Full::new(Bytes::new()),
    };
    
    builder.body(body)
        .map_err(|e| Error::Proxy(format!("Failed to build response: {}", e)))
}

/// Forward a request, using full URL from request or prepending target_url.
/// 
/// This function handles both:
/// - Requests with full URLs (from path-based extraction)
/// - Requests with paths that need target_url prepended
async fn forward_request_smart(
    client: &reqwest::Client,
    target_url: Option<&str>,
    request: &RecordedRequest,
) -> Result<RecordedResponse> {
    // Build full URL - use request URL if it's already full, otherwise prepend target
    let full_url = if request.url.starts_with("http://") || request.url.starts_with("https://") {
        request.url.clone()
    } else if let Some(target) = target_url {
        format!("{}{}", target.trim_end_matches('/'), request.url)
    } else {
        return Err(Error::Proxy(format!(
            "Cannot forward request to '{}': not a full URL and no target configured",
            request.url
        )));
    };
    
    forward_request_to_url(client, &full_url, request).await
}

#[allow(dead_code)]
async fn forward_request(
    client: &reqwest::Client,
    target_url: &str,
    request: &RecordedRequest,
) -> Result<RecordedResponse> {
    // Build full URL
    let full_url = if request.url.starts_with("http") {
        request.url.clone()
    } else {
        format!("{}{}", target_url.trim_end_matches('/'), request.url)
    };
    
    forward_request_to_url(client, &full_url, request).await
}

async fn forward_request_to_url(
    client: &reqwest::Client,
    full_url: impl reqwest::IntoUrl,
    request: &RecordedRequest,
) -> Result<RecordedResponse> {
    
    let method: reqwest::Method = request.method.parse()
        .unwrap_or(reqwest::Method::GET);
    
    let mut builder = client.request(method, full_url);
    
    // Add headers
    for header in &request.headers {
        // Skip host header (reqwest sets it)
        if header.name.to_lowercase() != "host" {
            builder = builder.header(&header.name, &header.value);
        }
    }
    
    // Add body
    if let Some(body) = &request.body {
        let bytes = match request.body_encoding {
            BodyEncoding::Text => body.as_bytes().to_vec(),
            BodyEncoding::Base64 => {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD
                    .decode(body)
                    .map_err(|e| Error::Proxy(format!("Failed to decode request body: {}", e)))?
            }
        };
        builder = builder.body(bytes);
    }
    
    let response = builder.send().await
        .map_err(|e| Error::Proxy(format!("Forward request failed: {}", e)))?;
    
    // Convert to recorded response
    let status = response.status().as_u16();
    
    // Strip transport-layer headers that are modified by decompression
    const HEADERS_TO_STRIP: &[&str] = &["content-encoding", "transfer-encoding", "content-length"];
    let headers: Vec<Header> = response
        .headers()
        .iter()
        .filter(|(k, _)| {
            let name = k.as_str().to_lowercase();
            !HEADERS_TO_STRIP.contains(&name.as_str())
        })
        .map(|(k, v)| Header::new(k.as_str(), v.to_str().unwrap_or("")))
        .collect();
    
    let body_bytes = response.bytes().await
        .map_err(|e| Error::Proxy(format!("Failed to read response body: {}", e)))?;
    
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
    
    Ok(recorded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_config_builder() {
        let config = ProxyConfig::new(8080, Mode::Record, "cassettes/test.json")
            .target("https://api.example.com");
        
        assert_eq!(config.port, 8080);
        assert!(matches!(config.mode, Mode::Record));
        assert_eq!(config.target_url, Some("https://api.example.com".to_string()));
    }

    #[tokio::test]
    async fn recorded_to_hyper_simple() {
        let resp = RecordedResponse::new(200).body("Hello");
        let hyper_resp = recorded_to_hyper(&resp).unwrap();
        
        assert_eq!(hyper_resp.status(), StatusCode::OK);
        
        let body = hyper_resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(body.as_ref(), b"Hello");
    }

    #[tokio::test]
    async fn recorded_to_hyper_with_headers() {
        let mut resp = RecordedResponse::new(201);
        resp.headers = vec![
            Header::new("content-type", "application/json"),
            Header::new("x-custom", "value"),
        ];
        resp.body = Some(r#"{"ok":true}"#.to_string());
        
        let hyper_resp = recorded_to_hyper(&resp).unwrap();
        
        assert_eq!(hyper_resp.status(), StatusCode::CREATED);
        assert_eq!(
            hyper_resp.headers().get("content-type").unwrap(),
            "application/json"
        );
    }

    #[test]
    fn test_extract_url_from_path() {
        // HTTPS URL in path
        assert_eq!(
            extract_url_from_path("/https://api.example.com/users"),
            Some("https://api.example.com/users".to_string())
        );
        
        // HTTP URL in path
        assert_eq!(
            extract_url_from_path("/http://api.example.com/data"),
            Some("http://api.example.com/data".to_string())
        );
        
        // URL with query string
        assert_eq!(
            extract_url_from_path("/https://api.example.com/search?q=test"),
            Some("https://api.example.com/search?q=test".to_string())
        );
        
        // Regular path (not a full URL)
        assert_eq!(extract_url_from_path("/api/users"), None);
        assert_eq!(extract_url_from_path("/"), None);
        assert_eq!(extract_url_from_path(""), None);
        
        // Without leading slash
        assert_eq!(
            extract_url_from_path("https://api.example.com/test"),
            Some("https://api.example.com/test".to_string())
        );
    }
}
