//! MCP tools for event operations.

use rust_mcp_sdk::macros::{JsonSchema, mcp_tool};
use rust_mcp_sdk::schema::{CallToolResult, schema_utils::CallToolError};
use serde::{Deserialize, Serialize};

// ---- Tool 5: createCalendarEvent ----

#[mcp_tool(name = "createCalendarEvent", description = "Create a new event in a calendar")]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CreateCalendarEventTool {
    pub calendar_id: String,
    pub title: String,
    pub start_date: String,
    pub end_date: String,
    pub location: Option<String>,
    pub notes: Option<String>,
}

impl CreateCalendarEventTool {
    pub fn execute(&self) -> Result<CallToolResult, CallToolError> {
        Ok(CallToolResult::text_content(vec!["not implemented".into()]))
    }
}

// ---- Tool 6: updateCalendarEvent ----

#[mcp_tool(name = "updateCalendarEvent", description = "Update an existing event in a calendar")]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct UpdateCalendarEventTool {
    pub calendar_id: String,
    pub event_id: String,
    pub title: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub location: Option<String>,
    pub notes: Option<String>,
}

impl UpdateCalendarEventTool {
    pub fn execute(&self) -> Result<CallToolResult, CallToolError> {
        Ok(CallToolResult::text_content(vec!["not implemented".into()]))
    }
}

// ---- Tool 7: deleteCalendarEvent ----

#[mcp_tool(name = "deleteCalendarEvent", description = "Delete an event from a calendar")]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DeleteCalendarEventTool {
    pub calendar_id: String,
    pub event_id: String,
}

impl DeleteCalendarEventTool {
    pub fn execute(&self) -> Result<CallToolResult, CallToolError> {
        Ok(CallToolResult::text_content(vec!["not implemented".into()]))
    }
}
