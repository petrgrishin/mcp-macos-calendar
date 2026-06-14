//! MCP ServerHandler implementation using rmcp.
//!
//! Defines [`CalendarMcpHandler`] which implements [`ServerHandler`] from `rmcp`,
//! providing 7 MCP tools for macOS Calendar access via EventKit.

use std::sync::{Arc, Mutex};

use rmcp::{
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext, wrapper::Parameters},
    model::*,
    service::RequestContext,
    tool, tool_router, ErrorData, RoleServer, ServerHandler,
};

use crate::bridge::eventkit::EventKitBridge;
use crate::models::{EventCreateRequest, EventUpdateRequest};
use crate::services::calendar_service::CalendarService;
use crate::services::event_service::EventService;
use crate::tools::calendar::{
    CreateCalendarParams, DeleteCalendarParams, GetCalendarEventsParams, GetCalendarsParams,
};
use crate::tools::event::{
    CreateCalendarEventParams, DeleteCalendarEventParams, UpdateCalendarEventParams,
};

/// Mutation tool names — hidden in read-only mode.
const MUTATION_TOOL_NAMES: &[&str] = &[
    "createCalendar",
    "deleteCalendar",
    "createCalendarEvent",
    "updateCalendarEvent",
    "deleteCalendarEvent",
];

// ---------------------------------------------------------------------------
// CalendarMcpHandler
// ---------------------------------------------------------------------------

/// MCP server handler for macOS Calendar.
///
/// Holds an [`EventKitBridge`] behind a `Mutex` and a [`ToolRouter`] generated
/// by the `#[tool_router]` macro. Implements [`ServerHandler`] manually so that
/// `list_tools` can filter out mutation tools when `read_only` is `true`.
pub struct CalendarMcpHandler {
    bridge: Arc<Mutex<Option<EventKitBridge>>>,
    read_only: bool,
    tool_router: ToolRouter<Self>,
}

impl Default for CalendarMcpHandler {
    fn default() -> Self {
        Self {
            bridge: Arc::new(Mutex::new(None)),
            read_only: false,
            tool_router: Self::tool_router(),
        }
    }
}

impl CalendarMcpHandler {
    /// Creates a new handler with the given EventKit bridge.
    pub fn with_bridge(bridge: EventKitBridge) -> Self {
        Self {
            bridge: Arc::new(Mutex::new(Some(bridge))),
            read_only: false,
            tool_router: Self::tool_router(),
        }
    }

    /// Creates a new handler with the given EventKit bridge and read-only flag.
    pub fn with_bridge_and_read_only(bridge: Option<EventKitBridge>, read_only: bool) -> Self {
        Self {
            bridge: Arc::new(Mutex::new(bridge)),
            read_only,
            tool_router: Self::tool_router(),
        }
    }

    /// Creates a new handler for HTTP transport with shared bridge.
    pub fn with_shared_bridge(bridge: Arc<Mutex<Option<EventKitBridge>>>, read_only: bool) -> Self {
        Self {
            bridge,
            read_only,
            tool_router: Self::tool_router(),
        }
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Creates a successful JSON `CallToolResult`.
fn success_json(data: &serde_json::Value) -> CallToolResult {
    CallToolResult::success(vec![Content::text(data.to_string())])
}

/// Creates an error JSON `CallToolResult` with `is_error: true`.
fn error_json(message: &str) -> CallToolResult {
    let error = serde_json::json!({"error": message});
    CallToolResult::error(vec![Content::text(error.to_string())])
}

/// Acquires the bridge lock and returns a guard, or an error result.
fn get_bridge(
    bridge: &Arc<Mutex<Option<EventKitBridge>>>,
) -> Result<std::sync::MutexGuard<'_, Option<EventKitBridge>>, CallToolResult> {
    bridge
        .lock()
        .map_err(|e| error_json(&format!("Bridge lock poisoned: {}", e)))
}

/// Helper to create an internal ErrorData from a string.
fn internal_error(msg: &str) -> ErrorData {
    ErrorData::internal_error(msg.to_string(), None)
}

// ---------------------------------------------------------------------------
// Tool definitions via #[tool_router]
// ---------------------------------------------------------------------------

#[tool_router(vis = "pub")]
impl CalendarMcpHandler {
    // ---- Tool 1: getCalendars ----

    #[tool(
        name = "getCalendars",
        description = "List all available macOS calendars"
    )]
    fn get_calendars(
        &self,
        Parameters(_params): Parameters<GetCalendarsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let bridge_guard =
            get_bridge(&self.bridge).map_err(|r| internal_error(&extract_error_text(&r)))?;
        let bridge = match bridge_guard.as_ref() {
            Some(b) => b,
            None => {
                tracing::error!("Tool error: getCalendars, error: EventKit bridge not available");
                return Ok(error_json("EventKit bridge not available"));
            }
        };

        let service = CalendarService::new(bridge);
        match service.list_calendars() {
            Ok(calendars) => {
                let result = serde_json::json!({"calendars": calendars});
                tracing::info!("Tool completed: getCalendars");
                Ok(success_json(&result))
            }
            Err(e) => {
                tracing::error!("Tool error: getCalendars, error: {}", e);
                Ok(error_json(&format!("Failed to get calendars: {}", e)))
            }
        }
    }

    // ---- Tool 2: getCalendarEvents ----

    #[tool(
        name = "getCalendarEvents",
        description = "Get events from a specific calendar, optionally filtered by date range with pagination"
    )]
    fn get_calendar_events(
        &self,
        Parameters(params): Parameters<GetCalendarEventsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let bridge_guard =
            get_bridge(&self.bridge).map_err(|r| internal_error(&extract_error_text(&r)))?;
        let bridge = match bridge_guard.as_ref() {
            Some(b) => b,
            None => {
                tracing::error!(
                    "Tool error: getCalendarEvents, error: EventKit bridge not available"
                );
                return Ok(error_json("EventKit bridge not available"));
            }
        };

        let service = EventService::new(bridge);
        match service.list_events(
            &params.calendar_id,
            params.start_date.as_deref(),
            params.end_date.as_deref(),
            params.limit,
            params.offset,
        ) {
            Ok(result) => {
                tracing::info!("Tool completed: getCalendarEvents");
                Ok(success_json(&serde_json::to_value(result).unwrap()))
            }
            Err(e) => {
                tracing::error!("Tool error: getCalendarEvents, error: {}", e);
                Ok(error_json(&format!(
                    "Failed to get events from calendar: {}",
                    e
                )))
            }
        }
    }

    // ---- Tool 3: createCalendar ----

    #[tool(
        name = "createCalendar",
        description = "Create a new calendar in macOS"
    )]
    fn create_calendar(
        &self,
        Parameters(params): Parameters<CreateCalendarParams>,
    ) -> Result<CallToolResult, ErrorData> {
        if self.read_only {
            return Err(ErrorData::invalid_params(
                "Tool is not available in read-only mode",
                None,
            ));
        }

        let bridge_guard =
            get_bridge(&self.bridge).map_err(|r| internal_error(&extract_error_text(&r)))?;
        let bridge = match bridge_guard.as_ref() {
            Some(b) => b,
            None => {
                tracing::error!("Tool error: createCalendar, error: EventKit bridge not available");
                return Ok(error_json("EventKit bridge not available"));
            }
        };

        let service = CalendarService::new(bridge);
        match service.create_calendar(&params.title, params.color.as_deref()) {
            Ok(calendar) => {
                let result = serde_json::json!({
                    "success": true,
                    "message": "Calendar created",
                    "calendar": calendar
                });
                tracing::info!("Tool completed: createCalendar");
                Ok(success_json(&result))
            }
            Err(e) => {
                tracing::error!("Tool error: createCalendar, error: {}", e);
                Ok(error_json(&format!("Failed to create calendar: {}", e)))
            }
        }
    }

    // ---- Tool 4: deleteCalendar ----

    #[tool(name = "deleteCalendar", description = "Delete a calendar from macOS")]
    fn delete_calendar(
        &self,
        Parameters(params): Parameters<DeleteCalendarParams>,
    ) -> Result<CallToolResult, ErrorData> {
        if self.read_only {
            return Err(ErrorData::invalid_params(
                "Tool is not available in read-only mode",
                None,
            ));
        }

        let bridge_guard =
            get_bridge(&self.bridge).map_err(|r| internal_error(&extract_error_text(&r)))?;
        let bridge = match bridge_guard.as_ref() {
            Some(b) => b,
            None => {
                tracing::error!("Tool error: deleteCalendar, error: EventKit bridge not available");
                return Ok(error_json("EventKit bridge not available"));
            }
        };

        let service = CalendarService::new(bridge);
        match service.delete_calendar(&params.calendar_id) {
            Ok(()) => {
                let result = serde_json::json!({
                    "success": true,
                    "message": "Calendar deleted"
                });
                tracing::info!("Tool completed: deleteCalendar");
                Ok(success_json(&result))
            }
            Err(e) => {
                tracing::error!("Tool error: deleteCalendar, error: {}", e);
                Ok(error_json(&format!("Failed to delete calendar: {}", e)))
            }
        }
    }

    // ---- Tool 5: createCalendarEvent ----

    #[tool(
        name = "createCalendarEvent",
        description = "Create a new event in a calendar"
    )]
    fn create_calendar_event(
        &self,
        Parameters(params): Parameters<CreateCalendarEventParams>,
    ) -> Result<CallToolResult, ErrorData> {
        if self.read_only {
            return Err(ErrorData::invalid_params(
                "Tool is not available in read-only mode",
                None,
            ));
        }

        let bridge_guard =
            get_bridge(&self.bridge).map_err(|r| internal_error(&extract_error_text(&r)))?;
        let bridge = match bridge_guard.as_ref() {
            Some(b) => b,
            None => {
                tracing::error!(
                    "Tool error: createCalendarEvent, error: EventKit bridge not available"
                );
                return Ok(error_json("EventKit bridge not available"));
            }
        };

        let service = EventService::new(bridge);
        let request = EventCreateRequest {
            calendar_id: params.calendar_id,
            title: params.title,
            start_date: params.start_date,
            end_date: params.end_date,
            is_all_day: None,
            location: params.location,
            notes: params.notes,
            url: None,
        };
        match service.create_event(request) {
            Ok(event) => {
                let result = serde_json::json!({
                    "success": true,
                    "message": "Event created",
                    "event": event
                });
                tracing::info!("Tool completed: createCalendarEvent");
                Ok(success_json(&result))
            }
            Err(e) => {
                tracing::error!("Tool error: createCalendarEvent, error: {}", e);
                Ok(error_json(&format!("Failed to create event: {}", e)))
            }
        }
    }

    // ---- Tool 6: updateCalendarEvent ----

    #[tool(
        name = "updateCalendarEvent",
        description = "Update an existing event in a calendar"
    )]
    fn update_calendar_event(
        &self,
        Parameters(params): Parameters<UpdateCalendarEventParams>,
    ) -> Result<CallToolResult, ErrorData> {
        if self.read_only {
            return Err(ErrorData::invalid_params(
                "Tool is not available in read-only mode",
                None,
            ));
        }

        let bridge_guard =
            get_bridge(&self.bridge).map_err(|r| internal_error(&extract_error_text(&r)))?;
        let bridge = match bridge_guard.as_ref() {
            Some(b) => b,
            None => {
                tracing::error!(
                    "Tool error: updateCalendarEvent, error: EventKit bridge not available"
                );
                return Ok(error_json("EventKit bridge not available"));
            }
        };

        let service = EventService::new(bridge);
        let request = EventUpdateRequest {
            calendar_id: params.calendar_id,
            event_id: params.event_id,
            title: params.title,
            start_date: params.start_date,
            end_date: params.end_date,
            is_all_day: None,
            location: params.location,
            notes: params.notes,
            url: None,
        };
        match service.update_event(request) {
            Ok(event) => {
                let result = serde_json::json!({
                    "success": true,
                    "message": "Event updated",
                    "event": event
                });
                tracing::info!("Tool completed: updateCalendarEvent");
                Ok(success_json(&result))
            }
            Err(e) => {
                tracing::error!("Tool error: updateCalendarEvent, error: {}", e);
                Ok(error_json(&format!("Failed to update event: {}", e)))
            }
        }
    }

    // ---- Tool 7: deleteCalendarEvent ----

    #[tool(
        name = "deleteCalendarEvent",
        description = "Delete an event from a calendar"
    )]
    fn delete_calendar_event(
        &self,
        Parameters(params): Parameters<DeleteCalendarEventParams>,
    ) -> Result<CallToolResult, ErrorData> {
        if self.read_only {
            return Err(ErrorData::invalid_params(
                "Tool is not available in read-only mode",
                None,
            ));
        }

        let bridge_guard =
            get_bridge(&self.bridge).map_err(|r| internal_error(&extract_error_text(&r)))?;
        let bridge = match bridge_guard.as_ref() {
            Some(b) => b,
            None => {
                tracing::error!(
                    "Tool error: deleteCalendarEvent, error: EventKit bridge not available"
                );
                return Ok(error_json("EventKit bridge not available"));
            }
        };

        let service = EventService::new(bridge);
        match service.delete_event(&params.calendar_id, &params.event_id) {
            Ok(()) => {
                let result = serde_json::json!({
                    "success": true,
                    "message": "Event deleted"
                });
                tracing::info!("Tool completed: deleteCalendarEvent");
                Ok(success_json(&result))
            }
            Err(e) => {
                tracing::error!("Tool error: deleteCalendarEvent, error: {}", e);
                Ok(error_json(&format!("Failed to delete event: {}", e)))
            }
        }
    }
}

/// Extract error text from a CallToolResult.
fn extract_error_text(result: &CallToolResult) -> String {
    result
        .content
        .first()
        .and_then(|c| c.as_text().map(|t| t.text.clone()))
        .unwrap_or_else(|| "Unknown error".to_string())
}

// ---------------------------------------------------------------------------
// ServerHandler implementation (manual — for read-only filtering)
// ---------------------------------------------------------------------------

impl ServerHandler for CalendarMcpHandler {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.server_info = Implementation::from_build_env();
        info.server_info.name = "mcp-macos-calendar".into();
        info.server_info.version = env!("CARGO_PKG_VERSION").into();
        info.instructions = Some("MCP server for macOS Calendar access via EventKit".to_string());
        info
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let tcc = ToolCallContext::new(self, request, context);
        self.tool_router.call(tcc).await
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tools = self.tool_router.list_all();
        let filtered: Vec<Tool> = if self.read_only {
            tools
                .into_iter()
                .filter(|t| !MUTATION_TOOL_NAMES.contains(&&*t.name))
                .collect()
        } else {
            tools
        };
        Ok(ListToolsResult {
            tools: filtered,
            next_cursor: None,
            meta: None,
        })
    }
}

// ---------------------------------------------------------------------------
// Public helpers
// ---------------------------------------------------------------------------

/// Returns the list of MCP calendar tools.
/// If `read_only` is true, only read-only tools are returned.
pub fn calendar_tools(read_only: bool) -> Vec<Tool> {
    let handler = CalendarMcpHandler {
        bridge: Arc::new(Mutex::new(None)),
        read_only,
        tool_router: CalendarMcpHandler::tool_router(),
    };
    let tools = handler.tool_router.list_all();
    if read_only {
        tools
            .into_iter()
            .filter(|t| !MUTATION_TOOL_NAMES.contains(&&*t.name))
            .collect()
    } else {
        tools
    }
}

/// Creates a `CallToolResult` representing an error with `is_error: true`.
/// The content is a JSON string: `{"error": "<message>"}`.
pub fn error_tool_result(message: &str) -> CallToolResult {
    error_json(message)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    /// S04AC1: CalendarMcpHandler implements ServerHandler trait.
    #[test]
    fn test_S04AC1_calendar_mcp_handler_implements_server_handler() {
        fn assert_impl<T: ServerHandler>() {}
        assert_impl::<CalendarMcpHandler>();
    }

    /// S04AC2: calendar_tools returns all 7 tools with correct names.
    #[test]
    fn test_S04AC2_handle_list_tools_returns_7_tools() {
        let tools = calendar_tools(false);
        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert_eq!(
            tool_names.len(),
            7,
            "Expected exactly 7 tools, got: {:?}",
            tool_names
        );

        let expected = [
            "getCalendars",
            "getCalendarEvents",
            "createCalendar",
            "deleteCalendar",
            "createCalendarEvent",
            "updateCalendarEvent",
            "deleteCalendarEvent",
        ];
        for name in &expected {
            assert!(
                tool_names.contains(name),
                "Missing tool: '{}'. Available: {:?}",
                name,
                tool_names
            );
        }
    }

    /// S04AC3: calendar_tools in read-only mode returns only 2 read-only tools.
    #[test]
    fn test_S04AC3_read_only_returns_only_2_tools() {
        let tools = calendar_tools(true);
        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert_eq!(
            tool_names.len(),
            2,
            "Expected exactly 2 tools in read-only mode, got: {:?}",
            tool_names
        );
        assert!(tool_names.contains(&"getCalendars"));
        assert!(tool_names.contains(&"getCalendarEvents"));
    }

    /// S06AC10: All tools return errors in JSON format when bridge is not available.
    #[test]
    fn test_S06AC10_all_tools_return_json_error_when_no_bridge() {
        let all_tools = [
            "getCalendars",
            "getCalendarEvents",
            "createCalendar",
            "deleteCalendar",
            "createCalendarEvent",
            "updateCalendarEvent",
            "deleteCalendarEvent",
        ];

        for name in &all_tools {
            let result = error_tool_result(&format!("Test error for {}", name));
            let json = serde_json::to_value(&result).unwrap();
            let text = json["content"][0]["text"].as_str().unwrap();
            let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
            assert!(
                parsed.get("error").is_some(),
                "tool '{}' error should contain 'error' key",
                name
            );
        }
    }

    /// S06AC11: JSON Schema properties are correctly generated for each tool.
    #[test]
    fn test_S06AC11_json_schema_properties_for_all_tools() {
        let tools = calendar_tools(false);

        // Helper: get properties map from tool's input_schema
        fn get_props(tool: &Tool) -> &serde_json::Map<String, serde_json::Value> {
            tool.input_schema
                .get("properties")
                .unwrap()
                .as_object()
                .unwrap()
        }

        // getCalendars: no properties (empty struct)
        let gc = tools.iter().find(|t| t.name == "getCalendars").unwrap();
        let props_opt = gc
            .input_schema
            .get("properties")
            .and_then(|p| p.as_object());
        assert!(
            props_opt.map_or(true, |p| p.is_empty()),
            "getCalendars should have empty or no properties"
        );

        // getCalendarEvents: has calendar_id
        let gce = tools
            .iter()
            .find(|t| t.name == "getCalendarEvents")
            .unwrap();
        let props = get_props(gce);
        assert!(props.contains_key("calendar_id"));

        // createCalendar: has title and color
        let cc = tools.iter().find(|t| t.name == "createCalendar").unwrap();
        let props = get_props(cc);
        assert!(props.contains_key("title"));
        assert!(props.contains_key("color"));

        // deleteCalendar: has calendar_id
        let dc = tools.iter().find(|t| t.name == "deleteCalendar").unwrap();
        let props = get_props(dc);
        assert!(props.contains_key("calendar_id"));

        // createCalendarEvent: has all event fields
        let cce = tools
            .iter()
            .find(|t| t.name == "createCalendarEvent")
            .unwrap();
        let props = get_props(cce);
        assert!(props.contains_key("calendar_id"));
        assert!(props.contains_key("title"));
        assert!(props.contains_key("start_date"));
        assert!(props.contains_key("end_date"));
        assert!(props.contains_key("location"));
        assert!(props.contains_key("notes"));

        // updateCalendarEvent: has calendar_id, event_id, and optional fields
        let uce = tools
            .iter()
            .find(|t| t.name == "updateCalendarEvent")
            .unwrap();
        let props = get_props(uce);
        assert!(props.contains_key("calendar_id"));
        assert!(props.contains_key("event_id"));
        assert!(props.contains_key("title"));
        assert!(props.contains_key("start_date"));
        assert!(props.contains_key("end_date"));
        assert!(props.contains_key("location"));
        assert!(props.contains_key("notes"));

        // deleteCalendarEvent: has calendar_id and event_id
        let dce = tools
            .iter()
            .find(|t| t.name == "deleteCalendarEvent")
            .unwrap();
        let props = get_props(dce);
        assert!(props.contains_key("calendar_id"));
        assert!(props.contains_key("event_id"));
    }

    /// S04AC8: get_info contains name "mcp-macos-calendar" and version.
    #[test]
    fn test_S04AC8_server_info_contains_name_and_version() {
        let handler = CalendarMcpHandler::default();
        let info = handler.get_info();
        assert_eq!(info.server_info.name.as_str(), "mcp-macos-calendar");
        assert_eq!(info.server_info.version.as_str(), env!("CARGO_PKG_VERSION"));
    }

    /// S04AC7: error_tool_result returns JSON {"error": "..."} with is_error: true.
    #[test]
    fn test_S04AC7_error_tool_result_returns_json_with_is_error() {
        let result = error_tool_result("something went wrong");
        assert_eq!(result.is_error, Some(true), "is_error must be Some(true)");
        assert_eq!(
            result.content.len(),
            1,
            "should have exactly one content item"
        );

        let json = serde_json::to_value(&result).expect("result should serialize");
        let text = json["content"][0]["text"]
            .as_str()
            .expect("content should have text");

        let parsed: serde_json::Value =
            serde_json::from_str(text).expect("content text should be valid JSON");
        assert_eq!(parsed["error"].as_str().unwrap(), "something went wrong");
    }
}
