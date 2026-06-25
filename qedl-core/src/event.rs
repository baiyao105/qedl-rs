use crate::state::DeviceState;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum Event {
    StateChanged {
        from: DeviceState,
        to: DeviceState,
    },
    Progress {
        current: u64,
        total: u64,
        message: String,
    },
    Error {
        code: crate::error::ErrorCode,
        message: String,
    },
    Log {
        level: LogLevel,
        message: String,
    },
    Sahara(SaharaEvent),
    Firehose(FirehoseEvent),
    Job(JobEvent),
    Collector(CollectorEvent),
}

#[derive(Debug, Clone)]
pub enum SaharaEvent {
    HelloReceived,
    HandshakeStarted,
    LoaderTransferring { sent: u64, total: u64 },
    HandshakeComplete,
    AlreadyInFirehoseMode,
}

#[derive(Debug, Clone)]
pub enum FirehoseEvent {
    ConfigureStarted,
    ConfigureComplete,
    ReadStarted { lun: u8, start: u64, count: u64 },
    ReadProgress { current: u64, total: u64 },
    ReadComplete,
    WriteStarted { lun: u8, start: u64, count: u64 },
    WriteProgress { current: u64, total: u64 },
    WriteComplete,
    EraseStarted { lun: u8, start: u64, count: u64 },
    EraseComplete,
}

#[derive(Debug, Clone)]
pub enum JobEvent {
    Started {
        name: String,
    },
    Progress {
        step: usize,
        total_steps: usize,
        message: String,
    },
    Complete {
        success: bool,
        message: String,
    },
}

#[derive(Debug, Clone)]
pub enum CollectorEvent {
    ClientConnected { port: String },
    ClientDisconnected,
    GptReadStarted { lun: u8 },
    GptReadComplete { partitions: usize },
    StorageInfoQueried { memory_type: Option<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// Event sink trait for receiving SDK events.
///
/// Implement this to receive events from the SDK.
/// Useful for GUI integration (Tauri, egui, Iced, etc.)
pub trait EventSink: Send + Sync {
    fn emit(&self, event: Event);
}

pub struct NoopEventSink;

impl EventSink for NoopEventSink {
    fn emit(&self, _event: Event) {}
}

pub fn emit_event(sink: &Option<Arc<dyn EventSink>>, event: Event) {
    if let Some(s) = sink {
        s.emit(event);
    }
}

pub fn emit_progress(sink: &Option<Arc<dyn EventSink>>, current: u64, total: u64, message: impl Into<String>) {
    emit_event(
        sink,
        Event::Progress {
            current,
            total,
            message: message.into(),
        },
    );
}
