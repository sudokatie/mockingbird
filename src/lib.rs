//! mockingbird - HTTP request recorder and replayer for deterministic tests.
//!
//! Record real API responses once, replay them forever. No more flaky tests
//! from external APIs.

pub mod cassette;
pub mod client;
pub mod error;
pub mod filter;
pub mod matcher;
pub mod mode;
pub mod proxy;
pub mod request;
pub mod response;

pub use client::{Client, ClientBuilder};
pub use error::{Error, Result};
pub use mode::Mode;
pub use proxy::{run as run_proxy, ProxyConfig};
pub use request::Request;
pub use response::Response;
