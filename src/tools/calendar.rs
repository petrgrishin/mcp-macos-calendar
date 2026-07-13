//! MCP tool parameter types for calendar operations.
//!
//! These structs are used as `Parameters<T>` arguments in the `#[tool]` methods
//! defined on [`CalendarMcpHandler`](crate::server::CalendarMcpHandler).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ---- Tool 1: getCalendars (no parameters) ----

/// Parameters for `getCalendars` tool (empty — no parameters required).
#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct GetCalendarsParams {}

// ---- Tool 2: getCalendarEvents ----

/// Parameters for `getCalendarEvents` tool.
#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct GetCalendarEventsParams {
    /// Optional when exactly one calendar is available; otherwise required
    pub calendar_id: Option<String>,
    /// Start date for filtering events in ISO8601 format, e.g. 2025-03-09T00:00:00.
    /// If not provided, defaults to 30 days ago
    pub start_date: Option<String>,
    /// End date for filtering events in ISO8601 format, e.g. 2025-03-09T23:59:59.
    /// If not provided, defaults to 30 days from now
    pub end_date: Option<String>,
    /// Maximum number of events to return. Defaults to 100
    pub limit: Option<u32>,
    /// Number of events to skip for pagination. Defaults to 0
    pub offset: Option<u32>,
}

// ---- Tool 3: createCalendar ----

/// Parameters for `createCalendar` tool.
#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct CreateCalendarParams {
    /// The title of the calendar
    pub title: String,
    /// The color of the calendar in hex format, e.g. #FF0000
    pub color: Option<String>,
}

// ---- Tool 4: deleteCalendar ----

/// Parameters for `deleteCalendar` tool.
#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct DeleteCalendarParams {
    /// The ID of the calendar to delete
    pub calendar_id: String,
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    /// S06AC1+S06AC2: getCalendars params is empty struct.
    #[test]
    fn test_S06AC1_getCalendars_params_is_empty() {
        let _params = GetCalendarsParams {};
        // Empty struct — just verify it compiles and defaults
    }

    /// S10AC4: getCalendarEvents accepts an explicit optional calendar_id.
    #[test]
    fn test_S10AC4_getCalendarEvents_accepts_explicit_calendar_id() {
        let json = serde_json::json!({"calendar_id": "cal-1"});
        let params: GetCalendarEventsParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.calendar_id.as_deref(), Some("cal-1"));
        assert!(params.start_date.is_none());
        assert!(params.end_date.is_none());
        assert!(params.limit.is_none());
        assert!(params.offset.is_none());
    }

    #[test]
    fn test_S10AC4_getCalendarEvents_deserializes_without_calendar_id() {
        let empty: GetCalendarEventsParams = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(empty.calendar_id.is_none());

        let filters_only: GetCalendarEventsParams = serde_json::from_value(serde_json::json!({
            "start_date": "2026-07-13T15:00:00",
            "end_date": "2026-07-13T19:00:00",
            "limit": 50,
            "offset": 10
        }))
        .unwrap();
        assert!(filters_only.calendar_id.is_none());
        assert_eq!(filters_only.limit, Some(50));
        assert_eq!(filters_only.offset, Some(10));
    }

    /// S06AC1+S06AC2: createCalendar params has title required, color optional.
    #[test]
    fn test_S06AC1_createCalendar_has_required_title_optional_color() {
        let json = serde_json::json!({"title": "Work"});
        let params: CreateCalendarParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.title, "Work");
        assert!(params.color.is_none());

        let json_with_color = serde_json::json!({"title": "Work", "color": "#FF0000"});
        let params: CreateCalendarParams = serde_json::from_value(json_with_color).unwrap();
        assert_eq!(params.color.as_deref(), Some("#FF0000"));
    }

    /// S06AC1+S06AC2: deleteCalendar params has calendar_id required.
    #[test]
    fn test_S06AC1_deleteCalendar_has_required_calendar_id() {
        let json = serde_json::json!({"calendar_id": "cal-1"});
        let params: DeleteCalendarParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.calendar_id, "cal-1");
    }

    /// S09AC1: GetCalendarEventsParams accepts optional parameters.
    #[test]
    fn test_S09AC1_getCalendarEvents_accepts_optional_params() {
        let json = serde_json::json!({
            "calendar_id": "cal-1",
            "start_date": "2025-03-09T00:00:00",
            "end_date": "2025-03-09T23:59:59",
            "limit": 50,
            "offset": 10
        });
        let params: GetCalendarEventsParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.calendar_id.as_deref(), Some("cal-1"));
        assert_eq!(params.start_date.as_deref(), Some("2025-03-09T00:00:00"));
        assert_eq!(params.end_date.as_deref(), Some("2025-03-09T23:59:59"));
        assert_eq!(params.limit, Some(50));
        assert_eq!(params.offset, Some(10));
    }
}
