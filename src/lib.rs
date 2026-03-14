//! mockingbird - HTTP request recorder and replayer for deterministic tests.
//!
//! Record real API responses once, replay them forever. No more flaky tests
//! from external APIs.
//!
//! # Example
//!
//! ```ignore
//! use mockingbird::Client;
//!
//! #[tokio::test]
//! async fn test_api() {
//!     let client = Client::auto("cassettes/api.json").build().unwrap();
//!     let response = client.get("https://api.example.com/users").send().await.unwrap();
//!     assert_eq!(response.status(), 200);
//! }
//! ```
//!
//! Or use the test attribute macro:
//!
//! ```ignore
//! #[mockingbird::test(cassette = "cassettes/api.json", mode = "auto")]
//! async fn test_api() {
//!     let response = client.get("https://api.example.com/users").send().await.unwrap();
//!     assert_eq!(response.status(), 200);
//! }
//! ```

pub mod cassette;
pub mod client;
pub mod error;
pub mod filter;
pub mod matcher;
pub mod middleware;
pub mod mode;
pub mod proxy;
pub mod request;
pub mod response;

pub use client::{Client, ClientBuilder};
pub use error::{Error, Result};
pub use middleware::{MockingbirdLayer, LayerBuilder, PlaybackResult};
pub use mode::Mode;
pub use proxy::{run as run_proxy, ProxyConfig};
pub use request::Request;
pub use response::Response;

// Re-export the test macro
pub use mockingbird_macros::test;
