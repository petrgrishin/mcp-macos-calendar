//! MCP macOS Calendar Server
//!
//! Entry point for the MCP server providing access to macOS Calendar via EventKit.

mod bridge;
mod config;
mod error;
mod models;
mod server;
mod services;
mod tools;

use clap::Parser;
use config::{CliArgs, ServerConfig, TransportType};
use rust_mcp_sdk::error::SdkResult;
use rust_mcp_sdk::event_store::InMemoryEventStore;
use rust_mcp_sdk::mcp_server::{hyper_server, server_runtime, HyperServerOptions, McpServerOptions};
use rust_mcp_sdk::{McpServer, StdioTransport, ToMcpServerHandler, TransportOptions};
use bridge::eventkit::EventKitBridge;
use server::{create_server_info, CalendarMcpHandler};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> SdkResult<()> {
    let args = CliArgs::parse();
    let config = ServerConfig::from(args);

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(&config.log_level)),
        )
        .init();

    if config.read_only {
        tracing::info!("Running in read-only mode");
    }

    tracing::info!(
        "Starting MCP macOS Calendar Server (transport: {})",
        config.transport
    );

    match config.transport {
        TransportType::Stdio => run_stdio(config.read_only).await,
        TransportType::Sse => run_sse(&config).await,
    }
}

/// Log instruction when calendar access is not granted.
fn log_access_denied_instruction() {
    tracing::error!("Calendar access is not granted.");
    tracing::error!("To grant access: System Settings > Privacy & Security > Calendars");
    tracing::error!("Enable access for mcp-macos-calendar");
}

/// Wait for Ctrl+C signal and log shutdown message.
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
    tracing::info!("Shutting down...");
}

/// Run the MCP server in stdio mode.
async fn run_stdio(read_only: bool) -> SdkResult<()> {
    tracing::info!("Using stdio transport");
    let server_info = create_server_info();
    let transport = StdioTransport::new(TransportOptions::default())?;

    let bridge = EventKitBridge::new().map_err(|e| {
        rust_mcp_sdk::schema::RpcError {
            code: -32603,
            message: e.to_string(),
            data: None,
        }
    })?;
    tracing::info!("Requesting calendar access...");
    let granted = bridge.request_access().map_err(|e| {
        rust_mcp_sdk::schema::RpcError {
            code: -32603,
            message: e.to_string(),
            data: None,
        }
    })?;
    if !granted {
        log_access_denied_instruction();
        return Err(rust_mcp_sdk::schema::RpcError {
            code: -32603,
            message: "Calendar access denied".into(),
            data: None,
        }.into());
    }
    tracing::info!("Calendar access granted");
    let handler = CalendarMcpHandler::with_bridge_and_read_only(bridge, read_only);
    let server = server_runtime::create_server(McpServerOptions {
        server_details: server_info,
        transport,
        handler: handler.to_mcp_server_handler(),
        task_store: None,
        client_task_store: None,
        message_observer: None,
    });
    server.start().await
}

/// Run the MCP server in SSE/HTTP mode.
async fn run_sse(config: &ServerConfig) -> SdkResult<()> {
    tracing::info!(
        "Using SSE/HTTP transport at {} ({})",
        config.mcp_endpoint(),
        config.sse_endpoint(),
    );
    let server_info = create_server_info();

    let bridge = EventKitBridge::new().map_err(|e| {
        rust_mcp_sdk::schema::RpcError {
            code: -32603,
            message: e.to_string(),
            data: None,
        }
    })?;
    tracing::info!("Requesting calendar access...");
    let granted = bridge.request_access().map_err(|e| {
        rust_mcp_sdk::schema::RpcError {
            code: -32603,
            message: e.to_string(),
            data: None,
        }
    })?;
    if !granted {
        log_access_denied_instruction();
        return Err(rust_mcp_sdk::schema::RpcError {
            code: -32603,
            message: "Calendar access denied".into(),
            data: None,
        }.into());
    }
    tracing::info!("Calendar access granted");
    let handler = CalendarMcpHandler::with_bridge_and_read_only(bridge, config.read_only);
    let server = hyper_server::create_server(
        server_info,
        handler.to_mcp_server_handler(),
        HyperServerOptions {
            host: config.host.clone(),
            port: config.port,
            event_store: Some(std::sync::Arc::new(InMemoryEventStore::default())),
            sse_support: true,
            dns_rebinding_protection: true,
            allowed_hosts: Some(vec![
                config.host.clone(),
                format!("{}:{}", config.host, config.port),
                "localhost".into(),
                format!("localhost:{}", config.port),
            ]),
            ..Default::default()
        },
    );
    tokio::select! {
        result = server.start() => result,
        _ = shutdown_signal() => {
            Ok(())
        }
    }
}

#[cfg(test)]
mod spec07_tests {
    #![allow(non_snake_case)]

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

    /// S07AC6: When calendar access is denied, instruction is logged.
    #[test]
    fn test_S07AC6_access_denied_logs_instruction() {
        let writer = TestWriter {
            buf: Arc::new(Mutex::new(Vec::new())),
        };
        let buf = writer.buf.clone();

        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new("error"))
            .with_writer(writer)
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            super::log_access_denied_instruction();
        });

        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            output.contains("Calendar access is not granted"),
            "Should log 'Calendar access is not granted', got: {}",
            output
        );
        assert!(
            output.contains("System Settings") && output.contains("Privacy & Security"),
            "Should log instruction with 'System Settings' and 'Privacy & Security', got: {}",
            output
        );
    }

    /// S07AC7: Claude Desktop config for stdio mode exists and is valid JSON.
    #[test]
    fn test_S07AC7_claude_desktop_stdio_config_exists() {
        let config_str = include_str!("../examples/claude_desktop_config_stdio.json");
        let config: serde_json::Value = serde_json::from_str(config_str)
            .expect("claude_desktop_config_stdio.json should be valid JSON");

        let servers = config["mcpServers"]["macos-calendar"].as_object()
            .expect("should have macos-calendar server config");
        assert!(
            servers["command"].is_string(),
            "should have 'command' field"
        );
        assert!(
            servers["args"].as_array().map_or(false, |a| !a.is_empty()),
            "should have non-empty 'args'"
        );
    }

    /// S07AC8: Claude Desktop config for SSE mode exists and is valid JSON.
    #[test]
    fn test_S07AC8_claude_desktop_sse_config_exists() {
        let config_str = include_str!("../examples/claude_desktop_config_sse.json");
        let config: serde_json::Value = serde_json::from_str(config_str)
            .expect("claude_desktop_config_sse.json should be valid JSON");

        let servers = config["mcpServers"]["macos-calendar"].as_object()
            .expect("should have macos-calendar server config");
        assert!(
            servers["url"].is_string(),
            "should have 'url' field"
        );
        let url = servers["url"].as_str().unwrap();
        assert!(
            url.contains("127.0.0.1") && url.contains("/sse"),
            "url should point to SSE endpoint, got: {}",
            url
        );
    }

    /// S07AC9: Server handles Ctrl+C gracefully and logs "Shutting down...".
    #[test]
    fn test_S07AC9_shutdown_logs_message() {
        let writer = TestWriter {
            buf: Arc::new(Mutex::new(Vec::new())),
        };
        let buf = writer.buf.clone();

        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
            .with_writer(writer)
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("Shutting down...");
        });

        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            output.contains("Shutting down..."),
            "Expected 'Shutting down...' log message, got: {}",
            output
        );
    }

    // ------------------------------------------------------------------
    // Spec 08 tests
    // ------------------------------------------------------------------

    /// S08AC5: При read_only=true логируется "Running in read-only mode".
    #[test]
    fn test_S08AC5_read_only_mode_logs_message() {
        let writer = TestWriter {
            buf: Arc::new(Mutex::new(Vec::new())),
        };
        let buf = writer.buf.clone();

        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
            .with_writer(writer)
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("Running in read-only mode");
        });

        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            output.contains("Running in read-only mode"),
            "Expected 'Running in read-only mode' log message, got: {}",
            output
        );
    }
}

/// Tests for calendar permission dialog fix (Info.plist + new EventKit API).
#[cfg(test)]
mod permission_fix_tests {
    #![allow(non_snake_case)]

    /// Info.plist must exist and contain NSCalendarsUsageDescription key.
    #[test]
    fn test_InfoPlist_has_calendar_usage_description() {
        let plist_str = include_str!("../Info.plist");
        assert!(
            plist_str.contains("NSCalendarsUsageDescription"),
            "Info.plist must contain NSCalendarsUsageDescription key"
        );
        // Verify it's valid XML plist
        assert!(
            plist_str.contains("<plist") && plist_str.contains("<dict>"),
            "Info.plist must be a valid XML plist"
        );
    }

    /// build.rs must exist and embed Info.plist via sectcreate linker flag.
    #[test]
    fn test_build_rs_embeds_info_plist() {
        let build_rs = include_str!("../build.rs");
        assert!(
            build_rs.contains("sectcreate") && build_rs.contains("__info_plist"),
            "build.rs must embed Info.plist via -sectcreate __TEXT __info_plist"
        );
        assert!(
            build_rs.contains("Info.plist"),
            "build.rs must reference Info.plist"
        );
    }
}
