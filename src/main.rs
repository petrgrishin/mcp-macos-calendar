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

    tracing::info!(
        "Starting MCP macOS Calendar Server (transport: {})",
        config.transport
    );

    match config.transport {
        TransportType::Stdio => run_stdio().await,
        TransportType::Sse => run_sse(&config).await,
    }
}

/// Run the MCP server in stdio mode.
async fn run_stdio() -> SdkResult<()> {
    tracing::info!("Using stdio transport");
    let server_info = create_server_info();
    let transport = StdioTransport::new(TransportOptions::default())?;
    let handler = CalendarMcpHandler::default();
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
    let handler = CalendarMcpHandler::default();
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
    server.start().await
}
