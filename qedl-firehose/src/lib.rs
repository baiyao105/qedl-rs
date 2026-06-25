//! Firehose XML command engine.

pub mod client;
pub mod command;
pub mod error;
pub mod response;

pub use client::FirehoseClient;
pub use command::FirehoseCommand;
pub use error::FirehoseError;
pub use response::FirehoseResponse;
