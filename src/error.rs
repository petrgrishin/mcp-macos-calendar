//! Custom error types for the MCP macOS Calendar server.

use thiserror::Error;

/// Unified error type for the application.
#[derive(Error, Debug)]
pub enum AppError {
    #[error("EventKit access denied: {0}")]
    EventKitAccessDenied(String),

    #[error("Calendar not found: {0}")]
    CalendarNotFound(String),

    #[error("Event not found: {0}")]
    EventNotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("EventKit error: {0}")]
    EventKit(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type AppResult<T> = Result<T, AppError>;
