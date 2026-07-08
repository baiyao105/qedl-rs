//! Rawprogram/patch XML parsing and sparse image expansion.
//!
//! Parses `rawprogram*.xml` and `patch*.xml` files into executable task lists,
//! handles sparse image expansion (Android `.s` format), and provides checksum
//! utilities.

pub mod checksum;
pub mod error;
pub mod patch;
pub mod rawprogram;
pub mod sparse;

pub use error::ImageError;
pub use patch::PatchSet;
pub use rawprogram::{TaskEntry, TaskList, TaskType};

pub use qedl_core::util::humanize_size;
