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
