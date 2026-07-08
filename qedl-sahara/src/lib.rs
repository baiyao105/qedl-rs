//! Sahara handshake protocol.
//!
//! Handles the initial USB handshake when a Qualcomm device enters EDL mode,
//! including Hello/HelloResponse exchange and optional Firehose loader upload.

pub mod error;
pub mod protocol;
pub mod session;

pub use error::SaharaError;
pub use protocol::{SaharaHello, SaharaHelloResponse, SaharaStatus};
pub use qedl_core::{SaharaCommand, SaharaMode};
pub use session::SaharaSession;
