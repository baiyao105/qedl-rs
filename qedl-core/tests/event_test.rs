use qedl_core::{DeviceState, Event, EventSink, LogLevel};
use std::sync::{Arc, Mutex};

struct TestEventSink {
    events: Mutex<Vec<Event>>,
}

impl TestEventSink {
    fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }

    fn events(&self) -> Vec<Event> {
        self.events.lock().unwrap().clone()
    }
}

impl EventSink for TestEventSink {
    fn emit(&self, event: Event) {
        self.events.lock().unwrap().push(event);
    }
}

#[tokio::test]
async fn test_event_sink_receives_events() {
    let sink = Arc::new(TestEventSink::new());
    let event = Event::StateChanged {
        from: DeviceState::Disconnected,
        to: DeviceState::Connected,
    };
    sink.emit(event);
    assert_eq!(sink.events().len(), 1);
}

#[tokio::test]
async fn test_event_display() {
    let event = Event::Progress {
        current: 50,
        total: 100,
        message: "flashing".to_string(),
    };
    let debug = format!("{:?}", event);
    assert!(debug.contains("Progress"));
}

#[tokio::test]
async fn test_log_level_equality() {
    assert_eq!(LogLevel::Info, LogLevel::Info);
    assert_ne!(LogLevel::Info, LogLevel::Error);
}
