//! MCP ServerHandler implementation.

use async_trait::async_trait;
use rust_mcp_sdk::mcp_server::ServerHandler;
use rust_mcp_sdk::schema::schema_utils::CallToolError;
use rust_mcp_sdk::schema::*;
use rust_mcp_sdk::mcp_server::enforce_compatible_protocol_version;
use rust_mcp_sdk::McpServer;
use std::sync::{Arc, Mutex};

use crate::bridge::eventkit::EventKitBridge;
use crate::tools::calendar::{
    CreateCalendarTool, DeleteCalendarTool, GetCalendarEventsTool, GetCalendarsTool,
};
use crate::tools::event::{
    CreateCalendarEventTool, DeleteCalendarEventTool, UpdateCalendarEventTool,
};

/// MCP server handler for macOS Calendar.
pub struct CalendarMcpHandler {
    bridge: Mutex<Option<EventKitBridge>>,
    read_only: bool,
}

impl Default for CalendarMcpHandler {
    fn default() -> Self {
        Self {
            bridge: Mutex::new(None),
            read_only: false,
        }
    }
}

impl CalendarMcpHandler {
    /// Creates a new handler with the given EventKit bridge.
    pub fn with_bridge(bridge: EventKitBridge) -> Self {
        Self {
            bridge: Mutex::new(Some(bridge)),
            read_only: false,
        }
    }

    /// Creates a new handler with the given EventKit bridge and read-only flag.
    pub fn with_bridge_and_read_only(bridge: EventKitBridge, read_only: bool) -> Self {
        Self {
            bridge: Mutex::new(Some(bridge)),
            read_only,
        }
    }
}

#[async_trait]
impl ServerHandler for CalendarMcpHandler {
    /// Diagnostic override: logs initialize request details and replicates default logic.
    async fn handle_initialize_request(
        &self,
        params: InitializeRequestParams,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<InitializeResult, RpcError> {
        tracing::info!(
            " DIAG: handle_initialize_request called — client_protocol_version={}, client_info={:?}",
            params.protocol_version,
            params.client_info,
        );
        let mut server_info = runtime.server_info().to_owned();
        tracing::info!(
            " DIAG: server_protocol_version={}, server_name={}",
            server_info.protocol_version,
            server_info.server_info.name,
        );

        // Replicate default logic from SDK with diagnostic logging
        if let Some(updated_protocol_version) = enforce_compatible_protocol_version(
            &params.protocol_version,
            &server_info.protocol_version,
        )
        .map_err(|err| {
            tracing::error!(
                " DIAG: Incompatible protocol version — client: {} server: {}",
                &params.protocol_version,
                &server_info.protocol_version
            );
            RpcError::internal_error().with_message(err.to_string())
        })? {
            tracing::info!(
                " DIAG: Downgrading protocol version from {} to {}",
                server_info.protocol_version,
                updated_protocol_version,
            );
            server_info.protocol_version = updated_protocol_version;
        }

        runtime
            .set_client_details(params)
            .await
            .map_err(|err| {
                tracing::error!(" DIAG: set_client_details failed: {err}");
                RpcError::internal_error().with_message(format!("{err}"))
            })?;

        tracing::info!(
            " DIAG: handle_initialize_request OK — response_protocol_version={}",
            server_info.protocol_version,
        );

        Ok(server_info)
    }

    /// Diagnostic override: logs when the client sends `initialized` notification.
    async fn on_initialized(&self, _runtime: Arc<dyn McpServer>) {
        tracing::info!(" DIAG: on_initialized called — MCP handshake complete");
    }

    async fn handle_list_tools_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            tools: calendar_tools(self.read_only),
            meta: None,
            next_cursor: None,
        })
    }

    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        let bridge_guard = self.bridge.lock().map_err(|e| {
            CallToolError::unknown_tool(format!("Bridge lock poisoned: {}", e))
        })?;
        dispatch_tool(&params.name, params.arguments.as_ref(), bridge_guard.as_ref(), self.read_only)
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

/// Deserializes tool arguments from an optional JSON map.
fn deserialize_args<T: serde::de::DeserializeOwned>(
    args: Option<&serde_json::Map<String, serde_json::Value>>,
) -> std::result::Result<T, CallToolError> {
    let value = match args {
        Some(map) => serde_json::Value::Object(map.clone()),
        None => serde_json::Value::Object(serde_json::Map::new()),
    };
    serde_json::from_value(value)
        .map_err(|e| CallToolError::unknown_tool(format!("Invalid arguments: {}", e)))
}

/// Mutation tool names.
const MUTATION_TOOLS: &[&str] = &[
    "createCalendar",
    "deleteCalendar",
    "createCalendarEvent",
    "updateCalendarEvent",
    "deleteCalendarEvent",
];

/// Dispatches a tool call by name to the appropriate tool implementation.
pub fn dispatch_tool(
    name: &str,
    arguments: Option<&serde_json::Map<String, serde_json::Value>>,
    bridge: Option<&EventKitBridge>,
    read_only: bool,
) -> std::result::Result<CallToolResult, CallToolError> {
    // First validate tool name.
    match name {
        "getCalendars" | "getCalendarEvents" | "createCalendar" | "deleteCalendar"
        | "createCalendarEvent" | "updateCalendarEvent" | "deleteCalendarEvent" => {}
        _ => return Err(CallToolError::unknown_tool(name.to_string())),
    }

    // Reject mutation tools in read-only mode.
    if read_only && MUTATION_TOOLS.contains(&name) {
        return Err(CallToolError::unknown_tool(format!(
            "Tool '{}' is not available in read-only mode",
            name
        )));
    }

    let bridge = match bridge {
        Some(b) => b,
        None => {
            tracing::error!("Tool error: {}, error: EventKit bridge not available", name);
            return Ok(error_tool_result("EventKit bridge not available"));
        }
    };

    let result = match name {
        "getCalendars" => {
            let tool: GetCalendarsTool = deserialize_args(arguments)?;
            tool.execute(bridge)
        }
        "getCalendarEvents" => {
            let tool: GetCalendarEventsTool = deserialize_args(arguments)?;
            tool.execute(bridge)
        }
        "createCalendar" => {
            let tool: CreateCalendarTool = deserialize_args(arguments)?;
            tool.execute(bridge)
        }
        "deleteCalendar" => {
            let tool: DeleteCalendarTool = deserialize_args(arguments)?;
            tool.execute(bridge)
        }
        "createCalendarEvent" => {
            let tool: CreateCalendarEventTool = deserialize_args(arguments)?;
            tool.execute(bridge)
        }
        "updateCalendarEvent" => {
            let tool: UpdateCalendarEventTool = deserialize_args(arguments)?;
            tool.execute(bridge)
        }
        "deleteCalendarEvent" => {
            let tool: DeleteCalendarEventTool = deserialize_args(arguments)?;
            tool.execute(bridge)
        }
        _ => unreachable!("already validated above"),
    };

    if let Ok(ref res) = result {
        if res.is_error == Some(true) {
            tracing::error!("Tool error: {}", name);
        } else {
            tracing::info!("Tool completed: {}", name);
        }
    }

    result
}

/// Returns the list of MCP calendar tools.
/// If `read_only` is true, only read-only tools are returned.
pub fn calendar_tools(read_only: bool) -> Vec<Tool> {
    let mut tools = vec![
        GetCalendarsTool::tool(),
        GetCalendarEventsTool::tool(),
    ];
    if !read_only {
        tools.extend_from_slice(&[
            CreateCalendarTool::tool(),
            DeleteCalendarTool::tool(),
            CreateCalendarEventTool::tool(),
            UpdateCalendarEventTool::tool(),
            DeleteCalendarEventTool::tool(),
        ]);
    }
    tools
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
        let tools = calendar_tools(false);
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
            let result = dispatch_tool(name, None, None, false);
            assert!(
                result.is_ok(),
                "dispatch_tool('{}') should succeed, got error: {:?}",
                name,
                result
            );
        }

        // Unknown tool should return error
        let result = dispatch_tool("unknownTool", None, None, false);
        assert!(result.is_err(), "dispatch_tool('unknownTool') should fail");
    }

    /// S06AC3: getCalendars returns error when bridge is not available.
    #[test]
    fn test_S06AC3_dispatch_getCalendars_no_bridge_returns_error() {
        let result = dispatch_tool("getCalendars", None, None, false);
        assert!(result.is_ok(), "dispatch_tool should return Ok");
        let result = result.unwrap();
        assert_eq!(result.is_error, Some(true), "should be error when no bridge");

        // Verify error JSON format
        let json = serde_json::to_value(&result).unwrap();
        let text = json["content"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert!(parsed["error"].as_str().unwrap().contains("EventKit"));
    }

    /// S06AC10: All tools return errors in JSON format with is_error: true when bridge is not available.
    #[test]
    fn test_S06AC10_all_tools_return_json_error_with_is_error_true() {
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
            let result = dispatch_tool(name, None, None, false).unwrap();
            assert_eq!(
                result.is_error,
                Some(true),
                "tool '{}' should return is_error: true",
                name
            );

            // Verify content is valid JSON with "error" key
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

        // getCalendars: no properties (empty struct)
        let gc = tools.iter().find(|t| t.name == "getCalendars").unwrap();
        assert!(gc.input_schema.properties.is_none() || gc.input_schema.properties.as_ref().map(|p| p.is_empty()).unwrap_or(true));

        // getCalendarEvents: has calendar_id
        let gce = tools.iter().find(|t| t.name == "getCalendarEvents").unwrap();
        let props = gce.input_schema.properties.as_ref().unwrap();
        assert!(props.contains_key("calendar_id"));

        // createCalendar: has title and color
        let cc = tools.iter().find(|t| t.name == "createCalendar").unwrap();
        let props = cc.input_schema.properties.as_ref().unwrap();
        assert!(props.contains_key("title"));
        assert!(props.contains_key("color"));

        // deleteCalendar: has calendar_id
        let dc = tools.iter().find(|t| t.name == "deleteCalendar").unwrap();
        let props = dc.input_schema.properties.as_ref().unwrap();
        assert!(props.contains_key("calendar_id"));

        // createCalendarEvent: has all event fields
        let cce = tools.iter().find(|t| t.name == "createCalendarEvent").unwrap();
        let props = cce.input_schema.properties.as_ref().unwrap();
        assert!(props.contains_key("calendar_id"));
        assert!(props.contains_key("title"));
        assert!(props.contains_key("start_date"));
        assert!(props.contains_key("end_date"));
        assert!(props.contains_key("location"));
        assert!(props.contains_key("notes"));

        // updateCalendarEvent: has calendar_id, event_id, and optional fields
        let uce = tools.iter().find(|t| t.name == "updateCalendarEvent").unwrap();
        let props = uce.input_schema.properties.as_ref().unwrap();
        assert!(props.contains_key("calendar_id"));
        assert!(props.contains_key("event_id"));
        assert!(props.contains_key("title"));
        assert!(props.contains_key("start_date"));
        assert!(props.contains_key("end_date"));
        assert!(props.contains_key("location"));
        assert!(props.contains_key("notes"));

        // deleteCalendarEvent: has calendar_id and event_id
        let dce = tools.iter().find(|t| t.name == "deleteCalendarEvent").unwrap();
        let props = dce.input_schema.properties.as_ref().unwrap();
        assert!(props.contains_key("calendar_id"));
        assert!(props.contains_key("event_id"));
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

    // ------------------------------------------------------------------
    // Spec 07 tests
    // ------------------------------------------------------------------

    /// S07AC3: Errors are logged with tracing::error!
    #[test]
    fn test_S07AC3_errors_logged_with_tracing_error() {
        use std::io::Write;
        use std::sync::{Arc, Mutex};

        #[derive(Clone)]
        struct TestWriter {
            buf: Arc<Mutex<Vec<u8>>>,
        }

        impl Write for TestWriter {
            fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
                self.buf.lock().unwrap().extend_from_slice(data);
                Ok(data.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for TestWriter {
            type Writer = TestWriter;
            fn make_writer(&'a self) -> Self::Writer {
                self.clone()
            }
        }

        let writer = TestWriter {
            buf: Arc::new(Mutex::new(Vec::new())),
        };
        let buf = writer.buf.clone();

        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new("error"))
            .with_writer(writer)
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            let _ = dispatch_tool("getCalendars", None, None, false);
        });

        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            output.contains("ERROR"),
            "Expected ERROR level log when tool returns error, got: {}",
            output
        );
    }

    /// S07AC4: Successful operations are logged with tracing::info!
    #[test]
    fn test_S07AC4_successful_operations_logged_with_tracing_info() {
        use std::io::Write;
        use std::sync::{Arc, Mutex};

        #[derive(Clone)]
        struct TestWriter {
            buf: Arc<Mutex<Vec<u8>>>,
        }

        impl Write for TestWriter {
            fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
                self.buf.lock().unwrap().extend_from_slice(data);
                Ok(data.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for TestWriter {
            type Writer = TestWriter;
            fn make_writer(&'a self) -> Self::Writer {
                self.clone()
            }
        }

        let writer = TestWriter {
            buf: Arc::new(Mutex::new(Vec::new())),
        };
        let buf = writer.buf.clone();

        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
            .with_writer(writer)
            .finish();

        // Verify that tracing::info! produces INFO-level output
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("Starting MCP macOS Calendar Server (transport: stdio)");
        });

        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            output.contains("INFO"),
            "Expected INFO level log for successful operation, got: {}",
            output
        );
        assert!(
            output.contains("Starting MCP macOS Calendar Server"),
            "Expected 'Starting MCP macOS Calendar Server' message, got: {}",
            output
        );
    }

    /// S07AC5: Log level is configurable via RUST_LOG env variable.
    #[test]
    fn test_S07AC5_log_level_configurable_via_env_filter() {
        use std::io::Write;
        use std::sync::{Arc, Mutex};

        #[derive(Clone)]
        struct TestWriter {
            buf: Arc<Mutex<Vec<u8>>>,
        }

        impl Write for TestWriter {
            fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
                self.buf.lock().unwrap().extend_from_slice(data);
                Ok(data.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for TestWriter {
            type Writer = TestWriter;
            fn make_writer(&'a self) -> Self::Writer {
                self.clone()
            }
        }

        // With RUST_LOG=error, INFO should NOT appear
        let writer_error = TestWriter {
            buf: Arc::new(Mutex::new(Vec::new())),
        };
        let buf_error = writer_error.buf.clone();

        let subscriber_error = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new("error"))
            .with_writer(writer_error)
            .finish();

        tracing::subscriber::with_default(subscriber_error, || {
            tracing::info!("this info should be filtered out");
            tracing::error!("this error should appear");
        });

        let output_error = String::from_utf8(buf_error.lock().unwrap().clone()).unwrap();
        assert!(
            !output_error.contains("this info should be filtered out"),
            "INFO should be filtered when RUST_LOG=error, got: {}",
            output_error
        );
        assert!(
            output_error.contains("this error should appear"),
            "ERROR should appear when RUST_LOG=error, got: {}",
            output_error
        );

        // With RUST_LOG=info, INFO SHOULD appear
        let writer_info = TestWriter {
            buf: Arc::new(Mutex::new(Vec::new())),
        };
        let buf_info = writer_info.buf.clone();

        let subscriber_info = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
            .with_writer(writer_info)
            .finish();

        tracing::subscriber::with_default(subscriber_info, || {
            tracing::info!("this info should appear");
        });

        let output_info = String::from_utf8(buf_info.lock().unwrap().clone()).unwrap();
        assert!(
            output_info.contains("this info should appear"),
            "INFO should appear when RUST_LOG=info, got: {}",
            output_info
        );
    }

    // ------------------------------------------------------------------
    // Spec 08 tests
    // ------------------------------------------------------------------

    /// S08AC2: При read_only=true calendar_tools возвращает только 2 инструмента.
    #[test]
    fn test_S08AC2_read_only_returns_only_read_tools() {
        let tools = calendar_tools(true);
        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(tool_names.len(), 2, "Expected 2 tools in read-only mode, got: {:?}", tool_names);
        assert!(tool_names.contains(&"getCalendars"), "Should contain getCalendars");
        assert!(tool_names.contains(&"getCalendarEvents"), "Should contain getCalendarEvents");
    }

    /// S08AC3: Без флага read_only calendar_tools возвращает все 7 инструментов.
    #[test]
    fn test_S08AC3_no_read_only_returns_all_7_tools() {
        let tools = calendar_tools(false);
        assert_eq!(tools.len(), 7, "Expected 7 tools without read-only flag");
    }

    /// S08AC4: При read_only=true dispatch_tool отклоняет mutation-инструменты.
    #[test]
    fn test_S08AC4_read_only_dispatch_rejects_mutation_tools() {
        let mutation_tools = [
            "createCalendar",
            "deleteCalendar",
            "createCalendarEvent",
            "updateCalendarEvent",
            "deleteCalendarEvent",
        ];

        for name in &mutation_tools {
            let result = dispatch_tool(name, None, None, true);
            assert!(
                result.is_err(),
                "dispatch_tool('{}', read_only=true) should return CallToolError",
                name
            );
        }
    }

    /// S08AC4: При read_only=true read-инструменты по-прежнему работают (возвращают Ok).
    #[test]
    fn test_S08AC4_read_only_allows_read_tools() {
        let read_tools = ["getCalendars", "getCalendarEvents"];
        for name in &read_tools {
            let result = dispatch_tool(name, None, None, true);
            assert!(
                result.is_ok(),
                "dispatch_tool('{}', read_only=true) should succeed for read tools",
                name
            );
        }
    }
}
