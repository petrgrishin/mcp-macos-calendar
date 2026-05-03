//! Services module — business logic for calendars and events.

pub mod calendar_service;
pub mod event_service;

use thiserror::Error;

use crate::bridge::eventkit::BridgeError;

/// Errors produced by service layer.
#[derive(Error, Debug)]
pub enum ServiceError {
    /// Input validation failed.
    #[error("Validation error: {0}")]
    Validation(String),

    /// An error from the EventKit bridge layer.
    #[error("{0}")]
    Bridge(#[from] BridgeError),
}

/// Convenience alias for service results.
pub type ServiceResult<T> = Result<T, ServiceError>;

#[cfg(test)]
mod spec07_tests {
    #![allow(non_snake_case)]

    use super::*;
    use crate::bridge::eventkit::BridgeError;

    /// S07AC2: ServiceError wraps BridgeError through #[from].
    #[test]
    fn test_S07AC2_service_error_wraps_bridge_error_via_from() {
        let bridge_err = BridgeError::CalendarNotFound("test-cal".into());
        let service_err: ServiceError = bridge_err.into();

        // Verify it's the Bridge variant
        match &service_err {
            ServiceError::Bridge(be) => {
                let msg = format!("{}", be);
                assert!(
                    msg.contains("test-cal"),
                    "Wrapped BridgeError should contain 'test-cal': got '{}'",
                    msg
                );
            }
            ServiceError::Validation(_) => {
                panic!("Expected ServiceError::Bridge variant, got Validation");
            }
        }

        // Verify Display includes the bridge error message
        let display = format!("{}", service_err);
        assert!(
            display.contains("test-cal"),
            "ServiceError display should contain bridge error detail: got '{}'",
            display
        );
    }
}
