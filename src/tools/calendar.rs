//! MCP tools for calendar operations.

use rust_mcp_sdk::macros::{JsonSchema, mcp_tool};
use rust_mcp_sdk::schema::{CallToolResult, TextContent, schema_utils::CallToolError};
use serde::{Deserialize, Serialize};

use crate::bridge::eventkit::EventKitBridge;
use crate::services::calendar_service::CalendarService;

/// Helper: create a successful JSON CallToolResult.
fn success_json(data: &serde_json::Value) -> CallToolResult {
    CallToolResult {
        content: vec![TextContent::from(data.to_string()).into()],
        is_error: Some(false),
        structured_content: None,
        meta: None,
    }
}

/// Helper: create an error JSON CallToolResult with `is_error: true`.
fn error_json(message: &str) -> CallToolResult {
    let error = serde_json::json!({"error": message});
    CallToolResult {
        content: vec![TextContent::from(error.to_string()).into()],
        is_error: Some(true),
        structured_content: None,
        meta: None,
    }
}

// ---- Tool 1: getCalendars ----

#[mcp_tool(name = "getCalendars", description = "List all available macOS calendars")]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GetCalendarsTool {}

impl GetCalendarsTool {
    pub fn execute(&self, bridge: &EventKitBridge) -> Result<CallToolResult, CallToolError> {
        let service = CalendarService::new(bridge);
        match service.list_calendars() {
            Ok(calendars) => {
                let result = serde_json::json!({"calendars": calendars});
                Ok(success_json(&result))
            }
            Err(e) => Ok(error_json(&format!("Failed to get calendars: {}", e))),
        }
    }
}

// ---- Tool 2: getCalendarEvents ----

#[mcp_tool(name = "getCalendarEvents", description = "Get events from a specific calendar, optionally filtered by date range with pagination")]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GetCalendarEventsTool {
    /// The ID of the calendar to get events from
    pub calendar_id: String,
    /// Start date for filtering events in ISO8601 format, e.g. 2025-03-09T00:00:00. If not provided, defaults to 30 days ago
    pub start_date: Option<String>,
    /// End date for filtering events in ISO8601 format, e.g. 2025-03-09T23:59:59. If not provided, defaults to 30 days from now
    pub end_date: Option<String>,
    /// Maximum number of events to return. Defaults to 100
    pub limit: Option<u32>,
    /// Number of events to skip for pagination. Defaults to 0
    pub offset: Option<u32>,
}

impl GetCalendarEventsTool {
    pub fn execute(&self, bridge: &EventKitBridge) -> Result<CallToolResult, CallToolError> {
        let service = crate::services::event_service::EventService::new(bridge);
        match service.list_events(
            &self.calendar_id,
            self.start_date.as_deref(),
            self.end_date.as_deref(),
            self.limit,
            self.offset,
        ) {
            Ok(result) => Ok(success_json(&serde_json::to_value(result).unwrap())),
            Err(e) => Ok(error_json(&format!(
                "Failed to get events from calendar: {}",
                e
            ))),
        }
    }
}

// ---- Tool 3: createCalendar ----

#[mcp_tool(name = "createCalendar", description = "Create a new calendar in macOS")]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CreateCalendarTool {
    /// The title of the calendar
    pub title: String,
    /// The color of the calendar in hex format, e.g. #FF0000
    pub color: Option<String>,
}

impl CreateCalendarTool {
    pub fn execute(&self, bridge: &EventKitBridge) -> Result<CallToolResult, CallToolError> {
        let service = CalendarService::new(bridge);
        match service.create_calendar(&self.title, self.color.as_deref()) {
            Ok(calendar) => {
                let result = serde_json::json!({
                    "success": true,
                    "message": "Calendar created",
                    "calendar": calendar
                });
                Ok(success_json(&result))
            }
            Err(e) => Ok(error_json(&format!("Failed to create calendar: {}", e))),
        }
    }
}

// ---- Tool 4: deleteCalendar ----

#[mcp_tool(name = "deleteCalendar", description = "Delete a calendar from macOS")]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DeleteCalendarTool {
    /// The ID of the calendar to delete
    pub calendar_id: String,
}

impl DeleteCalendarTool {
    pub fn execute(&self, bridge: &EventKitBridge) -> Result<CallToolResult, CallToolError> {
        let service = CalendarService::new(bridge);
        match service.delete_calendar(&self.calendar_id) {
            Ok(()) => {
                let result = serde_json::json!({
                    "success": true,
                    "message": "Calendar deleted"
                });
                Ok(success_json(&result))
            }
            Err(e) => Ok(error_json(&format!("Failed to delete calendar: {}", e))),
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    /// S06AC1+S06AC2: getCalendars tool has correct name and description.
    #[test]
    fn test_S06AC1_getCalendars_has_correct_name_and_description() {
        let tool = GetCalendarsTool::tool();
        assert_eq!(tool.name, "getCalendars");
        assert_eq!(tool.description.as_deref(), Some("List all available macOS calendars"));
    }

    /// S06AC1+S06AC2: getCalendarEvents tool has correct name, description, and calendar_id param.
    #[test]
    fn test_S06AC1_getCalendarEvents_has_correct_name_and_description() {
        let tool = GetCalendarEventsTool::tool();
        assert_eq!(tool.name, "getCalendarEvents");
        assert_eq!(
            tool.description.as_deref(),
            Some("Get events from a specific calendar, optionally filtered by date range with pagination")
        );
        // Verify input_schema has calendar_id as required
        let schema = &tool.input_schema;
        assert!(schema.required.contains(&"calendar_id".to_string()));
    }

    /// S06AC1+S06AC2: createCalendar tool has correct name, description, title (required), color (optional).
    #[test]
    fn test_S06AC1_createCalendar_has_correct_name_and_description() {
        let tool = CreateCalendarTool::tool();
        assert_eq!(tool.name, "createCalendar");
        assert_eq!(tool.description.as_deref(), Some("Create a new calendar in macOS"));
        let schema = &tool.input_schema;
        assert!(schema.required.contains(&"title".to_string()));
        assert!(!schema.required.contains(&"color".to_string()));
    }

    /// S06AC1+S06AC2: deleteCalendar tool has correct name, description, and calendar_id param.
    #[test]
    fn test_S06AC1_deleteCalendar_has_correct_name_and_description() {
        let tool = DeleteCalendarTool::tool();
        assert_eq!(tool.name, "deleteCalendar");
        assert_eq!(tool.description.as_deref(), Some("Delete a calendar from macOS"));
        let schema = &tool.input_schema;
        assert!(schema.required.contains(&"calendar_id".to_string()));
    }

    // ---- Spec 09 tests ----

    /// S09AC1: GetCalendarEventsTool accepts optional parameters startDate, endDate, limit, offset.
    #[test]
    fn test_S09AC1_getCalendarEvents_accepts_optional_params() {
        let tool = GetCalendarEventsTool::tool();
        let schema = &tool.input_schema;

        // calendar_id is still required
        assert!(schema.required.contains(&"calendar_id".to_string()));

        // New optional params must NOT be in required
        assert!(
            !schema.required.contains(&"start_date".to_string()),
            "start_date should be optional"
        );
        assert!(
            !schema.required.contains(&"end_date".to_string()),
            "end_date should be optional"
        );
        assert!(
            !schema.required.contains(&"limit".to_string()),
            "limit should be optional"
        );
        assert!(
            !schema.required.contains(&"offset".to_string()),
            "offset should be optional"
        );
    }

    /// S09AC8: JSON Schema tool correctly shows all 4 parameters as optional.
    #[test]
    fn test_S09AC8_getCalendarEvents_json_schema_has_all_optional_params() {
        let tool = GetCalendarEventsTool::tool();
        let schema = &tool.input_schema;

        // Verify description mentions date range and pagination
        assert_eq!(
            tool.name, "getCalendarEvents",
            "tool name should be getCalendarEvents"
        );

        // Verify the description was updated
        let desc = tool.description.as_deref().unwrap_or("");
        assert!(
            desc.contains("date range"),
            "description should mention 'date range': got '{}'",
            desc
        );
        assert!(
            desc.contains("pagination"),
            "description should mention 'pagination': got '{}'",
            desc
        );

        // Verify all 5 properties exist in schema (calendar_id + 4 new)
        let properties = schema.properties.as_ref().expect("properties should exist");
        assert!(properties.contains_key("calendar_id"), "schema should have calendar_id");
        assert!(properties.contains_key("start_date"), "schema should have start_date");
        assert!(properties.contains_key("end_date"), "schema should have end_date");
        assert!(properties.contains_key("limit"), "schema should have limit");
        assert!(properties.contains_key("offset"), "schema should have offset");
    }
}
