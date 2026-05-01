//! MCP ServerHandler implementation.

use async_trait::async_trait;
use rust_mcp_sdk::mcp_server::ServerHandler;
use rust_mcp_sdk::schema::schema_utils::CallToolError;
use rust_mcp_sdk::schema::*;
use rust_mcp_sdk::McpServer;
use std::sync::Arc;

/// MCP server handler for macOS Calendar.
#[derive(Default)]
pub struct CalendarServerHandler;

#[async_trait]
impl ServerHandler for CalendarServerHandler {
    async fn handle_list_tools_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            tools: vec![],
            meta: None,
            next_cursor: None,
        })
    }

    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        Err(CallToolError::unknown_tool(params.name))
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
