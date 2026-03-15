//! Cassette storage and types.
//!
//! Cassettes store recorded HTTP interactions for replay.

mod format;
mod storage;
mod types;

pub use format::Format;
pub use storage::{load_cassette, save_cassette, load_or_create};
pub use types::{
    Cassette, Interaction, RecordedRequest, RecordedResponse, Header, BodyEncoding, 
    IndexedCassette, RecordedError, ErrorKind
};
