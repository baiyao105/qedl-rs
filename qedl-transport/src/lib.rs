//! Transport layer abstraction for Qualcomm EDL devices.
//!
//! This crate provides the [`Transport`] trait for USB/serial communication,
//! [`DeviceEnumerator`] for discovering connected devices, and [`MockTransport`]
//! for testing.

pub mod device;
pub mod error;
pub mod mock;
pub mod port;
pub mod serial;

pub use bytes::{Bytes, BytesMut};
pub use device::{DeviceEnumerator, DeviceEnumeratorTrait, DeviceInfo};
pub use error::TransportError;
pub use mock::MockTransport;
pub use port::Transport;
pub use serial::SerialTransport;
