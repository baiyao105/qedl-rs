//! Core types and error definitions for QEDL.

pub mod error;
pub mod event;
pub mod protocol;
pub mod session;
pub mod state;
pub mod types;
pub mod util;

pub use error::{ErrorCode, QedlError, Result};
pub use event::{
    CollectorEvent, Event, EventSink, FirehoseEvent, JobEvent, LogLevel, NoopEventSink, SaharaEvent, emit_event,
    emit_progress,
};
pub use protocol::{
    DEFAULT_CHUNK_SIZE, FirehoseFunction, FirehoseInfo, HELLO_PACKET_SIZE, PROTOCOL_VERSION, PROTOCOL_VERSION_MIN,
    SaharaCommand, SaharaMode,
};
pub use session::Session;
pub use state::{DeviceState, StateError};
pub use types::{DeviceCapabilities, DeviceInfo, NoopProgress, PartitionInfo, ProgressReporter};
