use crate::error::{ErrorCode, QedlError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceState {
    Disconnected,
    Connected,
    Ready,
    Busy,
    Resetting,
    Error,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid state transition from {from:?} to {to:?}")]
pub struct StateError {
    pub from: DeviceState,
    pub to: DeviceState,
}

impl StateError {
    pub fn to_qedl_error(&self) -> QedlError {
        QedlError::WithCode {
            code: ErrorCode::InvalidArgument,
            message: format!("invalid state transition from {:?} to {:?}", self.from, self.to),
        }
    }
}

const VALID_TRANSITIONS: &[(DeviceState, DeviceState)] = &[
    // Normal flow
    (DeviceState::Disconnected, DeviceState::Connected),
    (DeviceState::Connected, DeviceState::Ready),
    (DeviceState::Ready, DeviceState::Busy),
    (DeviceState::Busy, DeviceState::Ready),
    (DeviceState::Ready, DeviceState::Resetting),
    (DeviceState::Resetting, DeviceState::Connected),
    (DeviceState::Ready, DeviceState::Disconnected),
    // Error recovery
    (DeviceState::Connected, DeviceState::Error),
    (DeviceState::Busy, DeviceState::Error),
    (DeviceState::Error, DeviceState::Disconnected),
    (DeviceState::Error, DeviceState::Resetting),
    (DeviceState::Error, DeviceState::Connected),
    // Mid-operation reconnect
    (DeviceState::Busy, DeviceState::Connected),
    (DeviceState::Ready, DeviceState::Connected),
];

impl DeviceState {
    pub fn transition(self, target: DeviceState) -> Result<DeviceState, StateError> {
        if VALID_TRANSITIONS.iter().any(|&(from, to)| from == self && to == target) {
            Ok(target)
        } else {
            Err(StateError { from: self, to: target })
        }
    }

    pub fn can_transition(self, target: DeviceState) -> bool {
        self.transition(target).is_ok()
    }

    pub fn can_execute(self) -> bool {
        self == Self::Ready
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Disconnected => "device not connected",
            Self::Connected => "device connected, waiting for handshake",
            Self::Ready => "device ready for operations",
            Self::Busy => "device is busy with an operation",
            Self::Resetting => "device is resetting",
            Self::Error => "device is in error state",
        }
    }
}

impl std::fmt::Display for DeviceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
