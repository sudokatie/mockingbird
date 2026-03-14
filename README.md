# mockingbird

HTTP request recorder and replayer for deterministic tests. Record real API responses once, replay them forever.

## Why This Exists?

External API calls in tests are the worst. They're slow, flaky, and break at 3am when some third-party service hiccups. Rate limits murder your CI. Network timeouts turn green builds red.

mockingbird records HTTP interactions to "cassettes" (JSON files) and replays them deterministically. Your tests become fast, reliable, and work offline. No more blaming Stripe's sandbox for your failed deploy.

## Features

- Record mode: capture real HTTP responses to cassettes
- Replay mode: serve recorded responses, fail if no match
- Auto mode: replay if found, record if not
- Flexible matching: URL, method, headers, body - mix and match
- URL normalization: query param order doesn't affect matching
- JSON body normalization: key order and whitespace don't affect matching
- Request/response filters: sanitize sensitive data before recording
- Cassette expiration: force re-recording after a duration
- JSON path filtering: scrub nested JSON fields
- Error recording: optionally record timeouts and connection errors
- Proxy server: record any HTTP client, language-agnostic
- YAML cassettes: optional YAML format support (feature flag)
- CLI tools: list, show, prune, refresh, check (with glob), delete

## Quick Start

```rust
use mockingbird::Client;

#[tokio::test]
async fn test_api_call() {
    // First run: records real responses
    // Subsequent runs: replays from cassette
    let client = Client::auto("tests/cassettes/api_test.json")
        .build()
        .unwrap();

    let response = client
        .get("https://api.example.com/users/1")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    
    let user: User = response.json().unwrap();
    assert_eq!(user.name, "Alice");
}
```

Or use the test attribute macro for cleaner tests:

```rust
#[mockingbird::test(cassette = "tests/cassettes/api_test.json", mode = "auto")]
async fn test_api_call() {
    // `client` is automatically available
    let response = client
        .get("https://api.example.com/users/1")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
}
```

## Modes

- `Record`: make real requests, save to cassette
- `Replay`: serve from cassette, fail if not found
- `Auto`: replay if exists, record otherwise (recommended for tests)
- `Passthrough`: make real requests, don't record

```rust
// Explicit mode selection
let client = Client::playback("cassette.json").build()?;
let client = Client::record("cassette.json").build()?;
let client = Client::auto("cassette.json").build()?;
```

## Filtering Sensitive Data

Don't commit your API keys to git:

```rust
let client = Client::auto("cassette.json")
    .filter_request_header("Authorization", "[REDACTED]")
    .filter_request_body_json("$.password", "[FILTERED]")
    .filter_response_body_json("$.api_key", "[FILTERED]")
    .build()?;
```

## Cassette Expiration

Force re-recording after 30 days:

```rust
use std::time::Duration;

let client = Client::auto("cassette.json")
    .expire_after(Duration::from_secs(30 * 24 * 60 * 60))
    .build()?;
```

## Recording Errors

Optionally record timeouts and connection errors so they replay deterministically:

```rust
let client = Client::auto("cassette.json")
    .record_errors(true)
    .build()?;

// If a request times out during recording, that timeout
// will be replayed on subsequent runs instead of making
// a real request.
```

## Redirect Handling

By default, redirects are followed and only the final response is recorded. To capture redirects as-is:

```rust
let client = Client::auto("cassette.json")
    .follow_redirects(false)
    .build()?;

// Now 301/302 responses are recorded directly
// instead of following them to the final destination.
```

## Custom Matching

By default, mockingbird matches on URL and method. Customize it:

```rust
use mockingbird::matcher::{UrlMethodMatcher, ExactMatcher, CustomMatcher, AllMatcher};

// URL + method only (default)
let client = Client::auto("cassette.json")
    .match_by(UrlMethodMatcher)
    .build()?;

// Exact matching (URL, method, headers, body)
let client = Client::auto("cassette.json")
    .match_by(ExactMatcher)
    .build()?;

// Custom matching logic
let client = Client::auto("cassette.json")
    .match_by(CustomMatcher::new(|recorded, incoming| {
        recorded.url.contains(&incoming.url)
    }))
    .build()?;

// Composite matching
use mockingbird::matcher::{MethodMatcher, UrlMatcher, BodyMatcher};
let client = Client::auto("cassette.json")
    .match_by(
        AllMatcher::new()
            .add(MethodMatcher)
            .add(UrlMatcher)
            .add(BodyMatcher)
    )
    .build()?;

// URL normalization (query param order ignored)
use mockingbird::matcher::NormalizedUrlMethodMatcher;
let client = Client::auto("cassette.json")
    .match_by(NormalizedUrlMethodMatcher)
    .build()?;
// Now ?a=1&b=2 matches ?b=2&a=1

// JSON body normalization (key order ignored)
use mockingbird::matcher::NormalizedJsonBodyMatcher;
let client = Client::auto("cassette.json")
    .match_by(
        AllMatcher::new()
            .add(MethodMatcher)
            .add(UrlMatcher)
            .add(NormalizedJsonBodyMatcher)
    )
    .build()?;
// Now {"a":1,"b":2} matches {"b":2,"a":1}
```

## Middleware Layer

Integrate with existing code without replacing your HTTP client:

```rust
use mockingbird::{MockingbirdLayer, Mode};
use mockingbird::cassette::RecordedRequest;

let layer = MockingbirdLayer::auto("cassette.json").build()?;

// Check for cached response
let request = RecordedRequest::new("GET", "https://api.example.com/data");
if let Some(cached) = layer.process_request(&request)? {
    // Use cached response
} else {
    // Make real request, then record it
    let response = make_real_request();
    layer.process_response(request, response)?;
}
```

## CLI Usage

```bash
# Unified serve command (recommended)
mockingbird serve --cassette api.json --mode playback --port 8080
mockingbird serve --cassette api.json --mode record --port 8080 --target https://api.example.com
mockingbird serve --cassette api.json --mode auto --port 8080 --target https://api.example.com

# Legacy commands (still supported)
mockingbird record --port 8080 --cassette api.json --target https://api.example.com
mockingbird replay --port 8080 --cassette api.json
mockingbird auto --port 8080 --cassette api.json --target https://api.example.com

# List recorded interactions
mockingbird list api.json

# Show specific interaction
mockingbird show api.json --index 1

# Remove old interactions (supports durations: 30d, 1w, 24h, 2m)
mockingbird prune api.json --older-than 30d
mockingbird prune api.json --older-than 1w --dry-run  # Preview without deleting

# Re-record all interactions
mockingbird refresh api.json --target https://api.example.com

# Validate cassette format (supports glob patterns)
mockingbird check api.json
mockingbird check "cassettes/*.json"
mockingbird check "tests/**/fixtures/*.json"

# Delete specific interactions
mockingbird delete api.json --indices 1,3,5
```

## Cassette Format

Cassettes are human-readable JSON (or YAML with the `yaml` feature):

```toml
# Cargo.toml - enable YAML support
[dependencies]
mockingbird = { version = "0.1", features = ["yaml"] }
```

Format is auto-detected from file extension:
- `.json` - JSON format (default)
- `.yaml` or `.yml` - YAML format (requires `yaml` feature)

JSON example:

```json
{
  "version": 1,
  "created_at": "2024-01-15T10:30:00Z",
  "modified_at": "2024-01-15T10:30:00Z",
  "interactions": [
    {
      "request": {
        "method": "GET",
        "url": "https://api.example.com/users/1",
        "headers": [
          {"name": "Accept", "value": "application/json"}
        ]
      },
      "response": {
        "status": 200,
        "headers": [
          {"name": "Content-Type", "value": "application/json"}
        ],
        "body": "{\"id\": 1, \"name\": \"Alice\"}"
      },
      "recorded_at": "2024-01-15T10:30:00Z"
    }
  ]
}
```

## Request Methods

Full reqwest-compatible API:

```rust
client.get("url").send().await?;
client.post("url").json(&data).send().await?;
client.put("url").body("content").send().await?;
client.patch("url").send().await?;
client.delete("url").send().await?;
client.head("url").send().await?;
client.request("OPTIONS", "url").send().await?;

// With options
client.post("url")
    .header("X-Custom", "value")
    .json(&payload)
    .query(&[("page", "1")])
    .timeout(Duration::from_secs(30))
    .send()
    .await?;
```

## License

MIT

---

Katie
