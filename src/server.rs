//! MCP ServerHandler implementation.

use async_trait::async_trait;
use rust_mcp_sdk::mcp_server::ServerHandler;
use rust_mcp_sdk::schema::schema_utils::CallToolError;
use rust_mcp_sdk::schema::*;
use rust_mcp_sdk::McpServer;
use std::sync::Arc;

use crate::tools::calendar::{
    CreateCalendarTool, DeleteCalendarTool, GetCalendarEventsTool, GetCalendarsTool,
};
use crate::tools::event::{
    CreateCalendarEventTool, DeleteCalendarEventTool, UpdateCalendarEventTool,
};

/// MCP server handler for macOS Calendar.
#[derive(Default)]
pub struct CalendarMcpHandler;

#[async_trait]
impl ServerHandler for CalendarMcpHandler {
    async fn handle_list_tools_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            tools: calendar_tools(),
            meta: None,
            next_cursor: None,
        })
    }

    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        dispatch_tool(&params.name, params.arguments.as_ref())
    }
}

/// Creates the server info for the MCP server.
pub fn create_server_info() -> InitializeResult {
    InitializeResult {
        server_info: Implementation {
            name: "mcp-macos-calendar".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            title: Some("MCP macOS Calendar".into()),
            description: Some("MCP server for macOS Calendar access via EventKit".into()),
            icons: vec![],
            website_url: None,
        },
        capabilities: ServerCapabilities {
            tools: Some(ServerCapabilitiesTools { list_changed: None }),
            ..Default::default()
        },
        protocol_version: ProtocolVersion::V2025_11_25.into(),
        instructions: None,
        meta: None,
    }
}

/// Creates a `CallToolResult` representing an error with `is_error: true`.
/// The content is a JSON string: `{"error": "<message>"}`.
pub fn error_tool_result(message: &str) -> CallToolResult {
    let error_json = serde_json::json!({"error": message}).to_string();
    CallToolResult {
        content: vec![TextContent::from(error_json).into()],
        is_error: Some(true),
        structured_content: None,
        meta: None,
    }
}

/// Dispatches a tool call by name to the appropriate tool implementation.
pub fn dispatch_tool(
    name: &str,
    _arguments: Option<&serde_json::Map<String, serde_json::Value>>,
) -> std::result::Result<CallToolResult, CallToolError> {
    match name {
        "getCalendars" => GetCalendarsTool {}.execute(),
        "getCalendarEvents" => GetCalendarEventsTool {
            calendar_id: String::new(),
        }
        .execute(),
        "createCalendar" => CreateCalendarTool {
            title: String::new(),
            color: None,
        }
        .execute(),
        "deleteCalendar" => DeleteCalendarTool {
            calendar_id: String::new(),
        }
        .execute(),
        "createCalendarEvent" => CreateCalendarEventTool {
            calendar_id: String::new(),
            title: String::new(),
            start_date: String::new(),
            end_date: String::new(),
            location: None,
            notes: None,
        }
        .execute(),
        "updateCalendarEvent" => UpdateCalendarEventTool {
            calendar_id: String::new(),
            event_id: String::new(),
            title: None,
            start_date: None,
            end_date: None,
            location: None,
            notes: None,
        }
        .execute(),
        "deleteCalendarEvent" => DeleteCalendarEventTool {
            calendar_id: String::new(),
            event_id: String::new(),
        }
        .execute(),
        _ => Err(CallToolError::unknown_tool(name.to_string())),
    }
}

/// Returns the list of all MCP calendar tools.
pub fn calendar_tools() -> Vec<Tool> {
    vec![
        GetCalendarsTool::tool(),
        GetCalendarEventsTool::tool(),
        CreateCalendarTool::tool(),
        DeleteCalendarTool::tool(),
        CreateCalendarEventTool::tool(),
        UpdateCalendarEventTool::tool(),
        DeleteCalendarEventTool::tool(),
    ]
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use rust_mcp_sdk::mcp_server::ServerHandler;

    /// S04AC1: CalendarMcpHandler implements ServerHandler trait.
    #[test]
    fn test_S04AC1_calendar_mcp_handler_implements_server_handler() {
        fn assert_impl<T: ServerHandler>() {}
        assert_impl::<CalendarMcpHandler>();
    }

    /// S04AC2: calendar_tools returns all 7 tools with correct names.
    #[test]
    fn test_S04AC2_handle_list_tools_returns_7_tools() {
        let tools = calendar_tools();
        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(tool_names.len(), 7, "Expected exactly 7 tools, got: {:?}", tool_names);

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

    /// S04AC3: dispatch_tool routes known tool names and rejects unknown ones.
    #[test]
    fn test_S04AC3_dispatch_tool_routes_known_tools_and_rejects_unknown() {
        let known_tools = [
            "getCalendars",
            "getCalendarEvents",
            "createCalendar",
            "deleteCalendar",
            "createCalendarEvent",
            "updateCalendarEvent",
            "deleteCalendarEvent",
        ];

        for name in &known_tools {
            let result = dispatch_tool(name, None);
            assert!(
                result.is_ok(),
                "dispatch_tool('{}') should succeed, got error: {:?}",
                name,
                result
            );
        }

        // Unknown tool should return error
        let result = dispatch_tool("unknownTool", None);
        assert!(result.is_err(), "dispatch_tool('unknownTool') should fail");
    }

    /// S04AC8: create_server_info contains name "mcp-macos-calendar" and version "0.1.0".
    #[test]
    fn test_S04AC8_server_info_contains_name_and_version() {
        let info = create_server_info();
        assert_eq!(info.server_info.name.as_str(), "mcp-macos-calendar");
        assert_eq!(info.server_info.version.as_str(), "0.1.0");
        assert!(info.capabilities.tools.is_some(), "tools capability should be present");
    }

    /// S04AC7: error_tool_result returns JSON {"error": "..."} with is_error: true.
    #[test]
    fn test_S04AC7_error_tool_result_returns_json_with_is_error() {
        let result = error_tool_result("something went wrong");
        assert_eq!(result.is_error, Some(true), "is_error must be Some(true)");
        assert_eq!(result.content.len(), 1, "should have exactly one content item");

        // Serialize the whole result to JSON and check the content
        let json = serde_json::to_value(&result).expect("result should serialize");
        let text = json["content"][0]["text"].as_str().expect("content should have text");

        let parsed: serde_json::Value = serde_json::from_str(text).expect("content text should be valid JSON");
        assert_eq!(parsed["error"].as_str().unwrap(), "something went wrong");
    }
}
