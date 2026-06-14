//! Data models for calendars and events.

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

/// Represents a macOS calendar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Calendar {
    pub id: String,
    pub title: String,
    pub color: String,
    pub is_default: bool,
    pub allows_modifications: bool,
}

/// Request to create a new calendar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarCreateRequest {
    pub title: String,
    pub color: Option<String>,
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
    pub url: Option<String>,
    pub is_all_day: bool,
}

/// Request to create an event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventCreateRequest {
    pub calendar_id: String,
    pub title: String,
    pub start_date: String,
    pub end_date: String,
    pub is_all_day: Option<bool>,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub url: Option<String>,
}

/// Request to update an event. All fields except calendar_id and event_id are optional.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventUpdateRequest {
    pub calendar_id: String,
    pub event_id: String,
    pub title: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub is_all_day: Option<bool>,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub url: Option<String>,
}

/// Result of listing events with pagination metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventListResult {
    pub events: Vec<Event>,
    pub total: usize,
    pub limit: u32,
    pub offset: u32,
    pub has_more: bool,
}

/// Generic MCP tool response wrapper.
#[derive(Debug, Serialize)]
pub struct ToolResponse<T: Serialize> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Parse a flexible date string into a NaiveDateTime.
/// Supports formats:
/// - `2025-03-09T10:00:00.000Z` (ISO8601 with milliseconds and UTC)
/// - `2025-03-09T10:00:00` (ISO8601 without milliseconds)
/// - `2025-03-09 10:00:00` (space instead of T)
pub fn parse_flexible_date(input: &str) -> Result<NaiveDateTime, String> {
    let input = input.trim();

    if input.is_empty() {
        return Err("Empty date string".to_string());
    }

    // Format 1: ISO8601 with milliseconds and UTC — "2025-03-09T10:00:00.000Z"
    if let Ok(dt) = NaiveDateTime::parse_from_str(input, "%Y-%m-%dT%H:%M:%S%.3fZ") {
        return Ok(dt);
    }

    // Format 2: ISO8601 without milliseconds — "2025-03-09T10:00:00"
    if let Ok(dt) = NaiveDateTime::parse_from_str(input, "%Y-%m-%dT%H:%M:%S") {
        return Ok(dt);
    }

    // Format 3: Space separator — "2025-03-09 10:00:00"
    if let Ok(dt) = NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M:%S") {
        return Ok(dt);
    }

    Err(format!(
        "Invalid date format: '{}'. Expected ISO8601 (e.g., 2025-03-09T10:00:00)",
        input
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Timelike};
    use serde_json::json;

    // ---- S03AC1: Calendar serialization ----

    #[test]
    fn test_s03ac1_calendar_serializes_to_json_with_all_fields() {
        let cal = Calendar {
            id: "cal-123".to_string(),
            title: "Work".to_string(),
            color: "#FF0000".to_string(),
            is_default: true,
            allows_modifications: false,
        };

        let json = serde_json::to_value(&cal).unwrap();
        let obj = json.as_object().unwrap();

        assert_eq!(obj.get("id").unwrap().as_str().unwrap(), "cal-123");
        assert_eq!(obj.get("title").unwrap().as_str().unwrap(), "Work");
        assert_eq!(obj.get("color").unwrap().as_str().unwrap(), "#FF0000");
        assert_eq!(obj.get("is_default").unwrap().as_bool().unwrap(), true);
        assert_eq!(
            obj.get("allows_modifications").unwrap().as_bool().unwrap(),
            false
        );

        // Verify exact field set
        let keys: std::collections::BTreeSet<_> = obj.keys().map(|s| s.as_str()).collect();
        let expected_keys: std::collections::BTreeSet<_> =
            ["id", "title", "color", "is_default", "allows_modifications"]
                .into_iter()
                .collect();
        assert_eq!(keys, expected_keys);
    }

    // ---- S03AC2: Event serialization with ISO8601 dates ----

    #[test]
    fn test_s03ac2_event_serializes_with_iso8601_dates() {
        let evt = Event {
            id: "evt-456".to_string(),
            title: "Meeting".to_string(),
            calendar_id: "cal-123".to_string(),
            start_date: "2025-03-09T10:00:00".to_string(),
            end_date: "2025-03-09T11:00:00".to_string(),
            location: Some("Office".to_string()),
            notes: None,
            url: None,
            is_all_day: false,
        };

        let json = serde_json::to_value(&evt).unwrap();
        let obj = json.as_object().unwrap();

        assert_eq!(
            obj.get("start_date").unwrap().as_str().unwrap(),
            "2025-03-09T10:00:00"
        );
        assert_eq!(
            obj.get("end_date").unwrap().as_str().unwrap(),
            "2025-03-09T11:00:00"
        );
        assert_eq!(obj.get("id").unwrap().as_str().unwrap(), "evt-456");
        assert_eq!(obj.get("title").unwrap().as_str().unwrap(), "Meeting");
        assert_eq!(obj.get("calendar_id").unwrap().as_str().unwrap(), "cal-123");
        assert_eq!(obj.get("is_all_day").unwrap().as_bool().unwrap(), false);
        assert_eq!(obj.get("location").unwrap().as_str().unwrap(), "Office");
        assert!(obj.get("notes").unwrap().is_null());
        assert!(obj.get("url").unwrap().is_null());
    }

    // ---- S03AC3: CalendarCreateRequest deserialization with optional color ----

    #[test]
    fn test_s03ac3_calendar_create_request_with_color() {
        let json = json!({
            "title": "Personal",
            "color": "#00FF00"
        });
        let req: CalendarCreateRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.title, "Personal");
        assert_eq!(req.color.as_deref(), Some("#00FF00"));
    }

    #[test]
    fn test_s03ac3_calendar_create_request_without_color() {
        let json = json!({
            "title": "Personal"
        });
        let req: CalendarCreateRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.title, "Personal");
        assert!(req.color.is_none());
    }

    // ---- S03AC4: EventCreateRequest deserialization with optional fields ----

    #[test]
    fn test_s03ac4_event_create_request_all_fields() {
        let json = json!({
            "calendar_id": "cal-1",
            "title": "Sprint Planning",
            "start_date": "2025-03-09T10:00:00",
            "end_date": "2025-03-09T11:00:00",
            "is_all_day": false,
            "location": "Room 42",
            "notes": "Bring laptop",
            "url": "https://meet.example.com/42"
        });
        let req: EventCreateRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.calendar_id, "cal-1");
        assert_eq!(req.title, "Sprint Planning");
        assert_eq!(req.start_date, "2025-03-09T10:00:00");
        assert_eq!(req.end_date, "2025-03-09T11:00:00");
        assert_eq!(req.is_all_day, Some(false));
        assert_eq!(req.location.as_deref(), Some("Room 42"));
        assert_eq!(req.notes.as_deref(), Some("Bring laptop"));
        assert_eq!(req.url.as_deref(), Some("https://meet.example.com/42"));
    }

    #[test]
    fn test_s03ac4_event_create_request_only_required_fields() {
        let json = json!({
            "calendar_id": "cal-1",
            "title": "Quick sync",
            "start_date": "2025-03-09T10:00:00",
            "end_date": "2025-03-09T10:15:00"
        });
        let req: EventCreateRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.calendar_id, "cal-1");
        assert_eq!(req.title, "Quick sync");
        assert!(req.is_all_day.is_none());
        assert!(req.location.is_none());
        assert!(req.notes.is_none());
        assert!(req.url.is_none());
    }

    // ---- S03AC5: EventUpdateRequest — all fields except calendar_id and event_id optional ----

    #[test]
    fn test_s03ac5_event_update_request_only_required_fields() {
        let json = json!({
            "calendar_id": "cal-1",
            "event_id": "evt-1"
        });
        let req: EventUpdateRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.calendar_id, "cal-1");
        assert_eq!(req.event_id, "evt-1");
        assert!(req.title.is_none());
        assert!(req.start_date.is_none());
        assert!(req.end_date.is_none());
        assert!(req.is_all_day.is_none());
        assert!(req.location.is_none());
        assert!(req.notes.is_none());
        assert!(req.url.is_none());
    }

    #[test]
    fn test_s03ac5_event_update_request_with_some_optional_fields() {
        let json = json!({
            "calendar_id": "cal-1",
            "event_id": "evt-1",
            "title": "Updated Meeting",
            "location": "New Room"
        });
        let req: EventUpdateRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.calendar_id, "cal-1");
        assert_eq!(req.event_id, "evt-1");
        assert_eq!(req.title.as_deref(), Some("Updated Meeting"));
        assert!(req.start_date.is_none());
        assert!(req.end_date.is_none());
        assert_eq!(req.location.as_deref(), Some("New Room"));
    }

    #[test]
    fn test_s03ac5_event_update_request_missing_calendar_id_fails() {
        let json = json!({
            "event_id": "evt-1",
            "title": "No calendar"
        });
        let result = serde_json::from_value::<EventUpdateRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_s03ac5_event_update_request_missing_event_id_fails() {
        let json = json!({
            "calendar_id": "cal-1",
            "title": "No event"
        });
        let result = serde_json::from_value::<EventUpdateRequest>(json);
        assert!(result.is_err());
    }

    // ---- S03AC6: parse_flexible_date parses all 3 formats ----

    #[test]
    fn test_s03ac6_parse_iso8601_with_milliseconds_and_utc() {
        let result = parse_flexible_date("2025-03-09T10:00:00.000Z");
        assert!(result.is_ok());
        let dt = result.unwrap();
        assert_eq!(dt.year(), 2025);
        assert_eq!(dt.month(), 3);
        assert_eq!(dt.day(), 9);
        assert_eq!(dt.hour(), 10);
        assert_eq!(dt.minute(), 0);
        assert_eq!(dt.second(), 0);
    }

    #[test]
    fn test_s03ac6_parse_iso8601_without_milliseconds() {
        let result = parse_flexible_date("2025-03-09T10:00:00");
        assert!(result.is_ok());
        let dt = result.unwrap();
        assert_eq!(dt.year(), 2025);
        assert_eq!(dt.month(), 3);
        assert_eq!(dt.day(), 9);
        assert_eq!(dt.hour(), 10);
    }

    #[test]
    fn test_s03ac6_parse_space_separated_datetime() {
        let result = parse_flexible_date("2025-03-09 10:00:00");
        assert!(result.is_ok());
        let dt = result.unwrap();
        assert_eq!(dt.year(), 2025);
        assert_eq!(dt.month(), 3);
        assert_eq!(dt.day(), 9);
        assert_eq!(dt.hour(), 10);
    }

    // ---- S03AC7: parse_flexible_date returns error for invalid date ----

    #[test]
    fn test_s03ac7_parse_invalid_date_returns_error() {
        let result = parse_flexible_date("not-a-date");
        assert!(result.is_err());
    }

    #[test]
    fn test_s03ac7_parse_empty_string_returns_error() {
        let result = parse_flexible_date("");
        assert!(result.is_err());
    }

    #[test]
    fn test_s03ac7_parse_partial_date_returns_error() {
        let result = parse_flexible_date("2025-03-09");
        assert!(result.is_err());
    }
}
