//! MCP tool parameter types for event operations.
//!
//! These structs are used as `Parameters<T>` arguments in the `#[tool]` methods
//! defined on [`CalendarMcpHandler`](crate::server::CalendarMcpHandler).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ---- Tool 5: createCalendarEvent ----

/// Parameters for `createCalendarEvent` tool.
#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct CreateCalendarEventParams {
    /// The ID of the calendar to create the event in
    pub calendar_id: String,
    /// The title of the event
    pub title: String,
    /// Start date in ISO8601 format, e.g. 2025-03-09T10:00:00.000Z
    pub start_date: String,
    /// End date in ISO8601 format, e.g. 2025-03-09T11:00:00.000Z
    pub end_date: String,
    /// Location of the event
    pub location: Option<String>,
    /// Notes for the event
    pub notes: Option<String>,
}

// ---- Tool 6: updateCalendarEvent ----

/// Parameters for `updateCalendarEvent` tool.
#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct UpdateCalendarEventParams {
    /// The ID of the calendar the event belongs to
    pub calendar_id: String,
    /// The ID of the event to update
    pub event_id: String,
    /// New title for the event
    pub title: Option<String>,
    /// New start date in ISO8601 format
    pub start_date: Option<String>,
    /// New end date in ISO8601 format
    pub end_date: Option<String>,
    /// New location for the event
    pub location: Option<String>,
    /// New notes for the event
    pub notes: Option<String>,
}

// ---- Tool 7: deleteCalendarEvent ----

/// Parameters for `deleteCalendarEvent` tool.
#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct DeleteCalendarEventParams {
    /// The ID of the calendar the event belongs to
    pub calendar_id: String,
    /// The ID of the event to delete
    pub event_id: String,
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    /// S06AC1+S06AC2: createCalendarEvent params has required fields.
    #[test]
    fn test_S06AC1_createCalendarEvent_has_required_params() {
        let json = serde_json::json!({
            "calendar_id": "cal-1",
            "title": "Meeting",
            "start_date": "2025-03-09T10:00:00",
            "end_date": "2025-03-09T11:00:00"
        });
        let params: CreateCalendarEventParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.calendar_id, "cal-1");
        assert_eq!(params.title, "Meeting");
        assert!(params.location.is_none());
        assert!(params.notes.is_none());
    }

    /// S06AC1+S06AC2: updateCalendarEvent params has required calendar_id and event_id.
    #[test]
    fn test_S06AC1_updateCalendarEvent_has_required_ids() {
        let json = serde_json::json!({
            "calendar_id": "cal-1",
            "event_id": "evt-1"
        });
        let params: UpdateCalendarEventParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.calendar_id, "cal-1");
        assert_eq!(params.event_id, "evt-1");
        assert!(params.title.is_none());
        assert!(params.start_date.is_none());
        assert!(params.end_date.is_none());
        assert!(params.location.is_none());
        assert!(params.notes.is_none());
    }

    /// S06AC1+S06AC2: deleteCalendarEvent params has required calendar_id and event_id.
    #[test]
    fn test_S06AC1_deleteCalendarEvent_has_required_ids() {
        let json = serde_json::json!({
            "calendar_id": "cal-1",
            "event_id": "evt-1"
        });
        let params: DeleteCalendarEventParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.calendar_id, "cal-1");
        assert_eq!(params.event_id, "evt-1");
    }
}
