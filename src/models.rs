//! Data models for calendars and events.

use serde::{Deserialize, Serialize};

/// Represents a macOS calendar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Calendar {
    pub id: String,
    pub title: String,
    pub color: String,
    pub is_subscribed: bool,
}

/// Request to create a new calendar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarCreateRequest {
    pub title: String,
}

/// Represents a calendar event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub title: String,
    pub calendar_id: String,
    pub start_date: String,
    pub end_date: String,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub is_all_day: bool,
}

/// Request to create or update an event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRequest {
    pub calendar_id: String,
    pub title: String,
    pub start_date: String,
    pub end_date: String,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub is_all_day: Option<bool>,
}
