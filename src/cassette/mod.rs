//! Cassette storage and types.
//!
//! Cassettes store recorded HTTP interactions for replay.

mod types;
mod storage;

pub use types::{
    Cassette, Interaction, RecordedRequest, RecordedResponse, Header, BodyEncoding, 
    IndexedCassette, RecordedError, ErrorKind
};
pub use storage::{load_cassette, save_cassette, load_or_create, Format};
