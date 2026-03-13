# mockingbird

HTTP request recorder and replayer for deterministic tests. Record real API responses once, replay them forever. No more flaky tests from external APIs.

## Why This Exists?

External API calls in tests are slow, flaky, and annoying. Rate limits, network issues, API changes - all of these break your CI at the worst possible time.

mockingbird records HTTP interactions to "cassettes" (JSON files) and replays them deterministically. Your tests become fast, reliable, and work offline.

Think VCR for Ruby, but Rust-fast.

## Features

- Record mode: captures real HTTP interactions
- Replay mode: serves recorded responses
- Flexible matching: URL, method, headers, body
- Request/response filters: sanitize sensitive data
- Multiple cassette formats: JSON (default), YAML (optional)
- Proxy server for language-agnostic recording
- CLI for cassette management

## Quick Start

```rust
use mockingbird::{Mockingbird, Mode};

#[tokio::test]
async fn test_api_call() {
    let mock = Mockingbird::new()
        .cassette("tests/cassettes/api_test.json")
        .mode(Mode::Record)
        .build();

    let client = mock.client();
    let response = client
        .get("https://api.example.com/users/1")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
}
```

## Modes

- `Record`: Make real requests, save to cassette
- `Replay`: Serve from cassette, fail if not found
- `Auto`: Replay if cassette exists, record otherwise
- `Passthrough`: Make real requests, don't record

## CLI Usage

```bash
# Record requests through proxy
mockingbird record --port 8080 --cassette api.json

# Replay from cassette
mockingbird replay --port 8080 --cassette api.json

# List recorded interactions
mockingbird list api.json

# Sanitize cassette (remove secrets)
mockingbird sanitize api.json --header Authorization
```

## License

MIT

---

Katie
