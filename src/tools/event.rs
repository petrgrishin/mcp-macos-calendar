//! MCP tools for event operations.

use rust_mcp_sdk::macros::{JsonSchema, mcp_tool};
use rust_mcp_sdk::schema::{CallToolResult, TextContent, schema_utils::CallToolError};
use serde::{Deserialize, Serialize};

use crate::bridge::eventkit::EventKitBridge;
use crate::models::{EventCreateRequest, EventUpdateRequest};

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

// ---- Tool 5: createCalendarEvent ----

#[mcp_tool(name = "createCalendarEvent", description = "Create a new event in a calendar")]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CreateCalendarEventTool {
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

impl CreateCalendarEventTool {
    pub fn execute(&self, bridge: &EventKitBridge) -> Result<CallToolResult, CallToolError> {
        let service = crate::services::event_service::EventService::new(bridge);
        let request = EventCreateRequest {
            calendar_id: self.calendar_id.clone(),
            title: self.title.clone(),
            start_date: self.start_date.clone(),
            end_date: self.end_date.clone(),
            is_all_day: None,
            location: self.location.clone(),
            notes: self.notes.clone(),
            url: None,
        };
        match service.create_event(request) {
            Ok(event) => {
                let result = serde_json::json!({
                    "success": true,
                    "message": "Event created",
                    "event": event
                });
                Ok(success_json(&result))
            }
            Err(e) => Ok(error_json(&format!("Failed to create event: {}", e))),
        }
    }
}

// ---- Tool 6: updateCalendarEvent ----

#[mcp_tool(name = "updateCalendarEvent", description = "Update an existing event in a calendar")]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct UpdateCalendarEventTool {
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

impl UpdateCalendarEventTool {
    pub fn execute(&self, bridge: &EventKitBridge) -> Result<CallToolResult, CallToolError> {
        let service = crate::services::event_service::EventService::new(bridge);
        let request = EventUpdateRequest {
            calendar_id: self.calendar_id.clone(),
            event_id: self.event_id.clone(),
            title: self.title.clone(),
            start_date: self.start_date.clone(),
            end_date: self.end_date.clone(),
            is_all_day: None,
            location: self.location.clone(),
            notes: self.notes.clone(),
            url: None,
        };
        match service.update_event(request) {
            Ok(event) => {
                let result = serde_json::json!({
                    "success": true,
                    "message": "Event updated",
                    "event": event
                });
                Ok(success_json(&result))
            }
            Err(e) => Ok(error_json(&format!("Failed to update event: {}", e))),
        }
    }
}

// ---- Tool 7: deleteCalendarEvent ----

#[mcp_tool(name = "deleteCalendarEvent", description = "Delete an event from a calendar")]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DeleteCalendarEventTool {
    /// The ID of the calendar the event belongs to
    pub calendar_id: String,
    /// The ID of the event to delete
    pub event_id: String,
}

impl DeleteCalendarEventTool {
    pub fn execute(&self, bridge: &EventKitBridge) -> Result<CallToolResult, CallToolError> {
        let service = crate::services::event_service::EventService::new(bridge);
        match service.delete_event(&self.calendar_id, &self.event_id) {
            Ok(()) => {
                let result = serde_json::json!({
                    "success": true,
                    "message": "Event deleted"
                });
                Ok(success_json(&result))
            }
            Err(e) => Ok(error_json(&format!("Failed to delete event: {}", e))),
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    /// S06AC1+S06AC2: createCalendarEvent tool has correct name, description, and required params.
    #[test]
    fn test_S06AC1_createCalendarEvent_has_correct_name_and_description() {
        let tool = CreateCalendarEventTool::tool();
        assert_eq!(tool.name, "createCalendarEvent");
        assert_eq!(tool.description.as_deref(), Some("Create a new event in a calendar"));
        let schema = &tool.input_schema;
        assert!(schema.required.contains(&"calendar_id".to_string()));
        assert!(schema.required.contains(&"title".to_string()));
        assert!(schema.required.contains(&"start_date".to_string()));
        assert!(schema.required.contains(&"end_date".to_string()));
        // location and notes should NOT be required
        assert!(!schema.required.contains(&"location".to_string()));
        assert!(!schema.required.contains(&"notes".to_string()));
    }

    /// S06AC1+S06AC2: updateCalendarEvent tool has correct name, description, and required params.
    #[test]
    fn test_S06AC1_updateCalendarEvent_has_correct_name_and_description() {
        let tool = UpdateCalendarEventTool::tool();
        assert_eq!(tool.name, "updateCalendarEvent");
        assert_eq!(tool.description.as_deref(), Some("Update an existing event in a calendar"));
        let schema = &tool.input_schema;
        assert!(schema.required.contains(&"calendar_id".to_string()));
        assert!(schema.required.contains(&"event_id".to_string()));
        // All other fields should be optional
        assert!(!schema.required.contains(&"title".to_string()));
        assert!(!schema.required.contains(&"start_date".to_string()));
        assert!(!schema.required.contains(&"end_date".to_string()));
        assert!(!schema.required.contains(&"location".to_string()));
        assert!(!schema.required.contains(&"notes".to_string()));
    }

    /// S06AC1+S06AC2: deleteCalendarEvent tool has correct name, description, and required params.
    #[test]
    fn test_S06AC1_deleteCalendarEvent_has_correct_name_and_description() {
        let tool = DeleteCalendarEventTool::tool();
        assert_eq!(tool.name, "deleteCalendarEvent");
        assert_eq!(tool.description.as_deref(), Some("Delete an event from a calendar"));
        let schema = &tool.input_schema;
        assert!(schema.required.contains(&"calendar_id".to_string()));
        assert!(schema.required.contains(&"event_id".to_string()));
    }
}
