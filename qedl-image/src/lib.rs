//! Rawprogram/patch XML parsing and sparse image expansion.

pub mod checksum;
pub mod error;
pub mod patch;
pub mod rawprogram;
pub mod sparse;

pub use error::ImageError;
pub use patch::PatchSet;
pub use rawprogram::{TaskEntry, TaskList, TaskType};

pub use qedl_core::util::humanize_size;
