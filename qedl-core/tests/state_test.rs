use qedl_core::DeviceState;

#[tokio::test]
async fn test_valid_transitions() {
    let state = DeviceState::Disconnected;
    let next = state.transition(DeviceState::Connected).unwrap();
    assert_eq!(next, DeviceState::Connected);

    let next = next.transition(DeviceState::Ready).unwrap();
    assert_eq!(next, DeviceState::Ready);

    let next = next.transition(DeviceState::Busy).unwrap();
    assert_eq!(next, DeviceState::Busy);

    let next = next.transition(DeviceState::Ready).unwrap();
    assert_eq!(next, DeviceState::Ready);
}

#[tokio::test]
async fn test_invalid_transition() {
    let state = DeviceState::Disconnected;
    let result = state.transition(DeviceState::Ready);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_error_recovery() {
    let state = DeviceState::Error;
    let next = state.transition(DeviceState::Disconnected).unwrap();
    assert_eq!(next, DeviceState::Disconnected);
}

#[tokio::test]
async fn test_can_execute() {
    assert!(!DeviceState::Disconnected.can_execute());
    assert!(!DeviceState::Connected.can_execute());
    assert!(DeviceState::Ready.can_execute());
    assert!(!DeviceState::Busy.can_execute());
}
