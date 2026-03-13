//! Cassette storage and types.
//!
//! Cassettes store recorded HTTP interactions for replay.

mod types;
mod storage;

pub use types::{Cassette, Interaction, RecordedRequest, RecordedResponse, Header, BodyEncoding};
pub use storage::{load_cassette, save_cassette};
