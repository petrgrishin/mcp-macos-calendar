//! MCP macOS Calendar Server
//!
//! Entry point for the MCP server providing access to macOS Calendar via EventKit.
//! Supports stdio and SSE/HTTP transports using the `rmcp` SDK.

mod bridge;
mod config;
mod error;
mod models;
mod server;
mod services;
mod sse_transport;
mod tools;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use bridge::eventkit::EventKitBridge;
use clap::Parser;
use config::{CliArgs, ServerConfig, TransportType};
use rmcp::ServiceExt;
use server::CalendarMcpHandler;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = CliArgs::parse();
    let config = ServerConfig::from(args);

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level)),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    if config.read_only {
        tracing::info!("Running in read-only mode");
    }
    if config.default_calendar_only {
        log_default_calendar_only_mode();
    }

    tracing::info!(
        "Starting MCP macOS Calendar Server (transport: {})",
        config.transport
    );

    match config.transport {
        TransportType::Stdio => run_stdio(config.read_only, config.default_calendar_only).await,
        TransportType::Sse => run_sse(&config).await,
    }
}

fn log_default_calendar_only_mode() {
    tracing::info!("Default-calendar-only mode enabled");
}

/// Log instruction when calendar access is not granted.
fn log_access_denied_instruction() {
    tracing::error!("Calendar access is not granted.");
    tracing::error!("To grant access: System Settings > Privacy & Security > Calendars");
    tracing::error!("Enable access for mcp-macos-calendar");
}

/// Create EventKitBridge and request calendar access.
/// Returns `Some(bridge)` on success, `None` if access is denied or unavailable.
fn try_create_bridge() -> Option<EventKitBridge> {
    let bridge = match EventKitBridge::new() {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("Failed to create EventKit bridge: {}", e);
            return None;
        }
    };
    tracing::info!("Requesting calendar access...");
    let granted = match bridge.request_access() {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!("Failed to request calendar access: {}", e);
            return None;
        }
    };
    if !granted {
        log_access_denied_instruction();
        return None;
    }
    tracing::info!("Calendar access granted");
    Some(bridge)
}

/// Run the MCP server in stdio mode.
async fn run_stdio(read_only: bool, default_calendar_only: bool) -> anyhow::Result<()> {
    tracing::info!("Using stdio transport");
    let bridge = try_create_bridge();
    if bridge.is_none() {
        tracing::warn!(
            "Calendar bridge not available; tools will return errors until access is granted"
        );
    }
    let handler =
        CalendarMcpHandler::with_bridge_and_options(bridge, read_only, default_calendar_only);
    let service = handler
        .serve(rmcp::transport::stdio())
        .await
        .inspect_err(|e| {
            tracing::error!("serving error: {:?}", e);
        })?;
    service.waiting().await?;
    Ok(())
}

/// Run the MCP server in SSE/HTTP mode with both legacy SSE and Streamable HTTP transports.
///
/// - `/sse` (GET) + `/message` (POST) — legacy SSE transport for backwards compatibility
/// - `/mcp` — Streamable HTTP transport (modern MCP protocol)
async fn run_sse(config: &ServerConfig) -> anyhow::Result<()> {
    use rmcp::transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpService,
    };

    tracing::info!(
        "Using SSE transport at {} and Streamable HTTP at {}",
        config.sse_endpoint(),
        config.mcp_endpoint(),
    );

    let bridge = try_create_bridge();
    if bridge.is_none() {
        tracing::warn!(
            "Calendar bridge not available; tools will return errors until access is granted"
        );
    }
    let bridge_arc: Arc<Mutex<Option<EventKitBridge>>> = Arc::new(Mutex::new(bridge));
    let read_only = config.read_only;
    let default_calendar_only = config.default_calendar_only;

    // --- Legacy SSE transport at /sse + /message ---
    let (sse_router, session_rx) = sse_transport::create_sse_router("/sse", "/message");

    let bridge_for_sse = bridge_arc.clone();
    tokio::spawn(sse_transport::serve_sse_sessions(session_rx, move || {
        let bridge = bridge_for_sse.clone();
        CalendarMcpHandler::with_shared_bridge(bridge, read_only, default_calendar_only)
    }));

    // --- Streamable HTTP transport at /mcp ---
    let bridge_for_http = bridge_arc;
    let streamable_service = StreamableHttpService::new(
        move || {
            let bridge = bridge_for_http.clone();
            Ok(CalendarMcpHandler::with_shared_bridge(
                bridge,
                read_only,
                default_calendar_only,
            ))
        },
        LocalSessionManager::default().into(),
        Default::default(),
    );

    let router = sse_router
        .nest_service("/mcp", streamable_service)
        .layer(tower_http::cors::CorsLayer::permissive());
    let listener =
        tokio::net::TcpListener::bind(format!("{}:{}", config.host, config.port)).await?;

    tracing::info!("HTTP server listening on {}:{}", config.host, config.port);

    // Oneshot channel to detect when Ctrl+C fires so the timeout
    // only starts AFTER the signal, not at server startup.
    let (shutdown_trigger, shutdown_signal) = tokio::sync::oneshot::channel::<()>();

    let server = axum::serve(listener, router).with_graceful_shutdown(async {
        tokio::signal::ctrl_c().await.unwrap();
        tracing::info!("Shutting down...");
        let _ = shutdown_trigger.send(());
    });

    tokio::select! {
        result = server => {
            if let Err(e) = result {
                tracing::error!("Server error: {}", e);
            }
        }
        _ = async {
            // Wait for Ctrl+C to fire first, then start the timeout
            shutdown_signal.await.ok();
            tokio::time::sleep(Duration::from_secs(3)).await;
            tracing::warn!("Graceful shutdown timed out, forcing exit");
        } => {}
    }

    std::process::exit(0);
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

    /// S07AC7: Сервер корректно обрабатывает Ctrl+C и завершает работу.
    #[test]
    fn test_S07AC7_shutdown_logs_message() {
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

    #[test]
    fn test_S10AC1_default_calendar_only_mode_logs_message() {
        let writer = TestWriter {
            buf: Arc::new(Mutex::new(Vec::new())),
        };
        let buf = writer.buf.clone();

        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
            .with_writer(writer)
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            super::log_default_calendar_only_mode();
        });

        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            output.contains("Default-calendar-only mode enabled"),
            "Expected default-calendar-only log message, got: {}",
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

    /// The server must run as a UI agent so it does not appear in the Dock.
    #[test]
    fn test_InfoPlist_runs_as_ui_element() {
        let plist_str = include_str!("../Info.plist");
        let eventkit_source = include_str!("bridge/eventkit.rs");
        assert!(
            plist_str.contains("<key>LSUIElement</key>")
                && plist_str.contains("<key>LSUIElement</key>\n    <true/>"),
            "Info.plist must enable LSUIElement"
        );
        assert!(
            eventkit_source
                .contains("setActivationPolicy(NSApplicationActivationPolicy::Accessory)"),
            "NSApplication must use Accessory activation policy"
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
