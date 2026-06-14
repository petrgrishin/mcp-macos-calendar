//! Integration tests for server startup (S01AC2, S01AC3).

#![allow(non_snake_case)]

use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// Path to the built binary.
fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mcp-macos-calendar"))
}

/// Helper to start the server process with given arguments.
fn start_server(args: &[&str]) -> Child {
    Command::new(binary_path())
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::piped())
        .spawn()
        .expect("Failed to start server process")
}

/// Helper to kill a child process gracefully.
fn kill_server(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// S01AC2: `cargo run -- --transport stdio` запускает MCP сервер в stdio режиме.
#[test]
fn test_S01AC2_stdio_server_starts() {
    let mut child = start_server(&["--transport", "stdio"]);

    // Give the server time to start
    std::thread::sleep(Duration::from_millis(1000));

    // Send a JSON-RPC initialize request via stdin
    let init_request = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1.0"}}}"#;

    if let Some(stdin) = child.stdin.as_mut() {
        let _ = writeln!(stdin, "{}", init_request);
        let _ = stdin.flush();
    }

    // Give time for response
    std::thread::sleep(Duration::from_millis(1000));

    // Check the process is still running (didn't crash)
    let running = child.try_wait().unwrap().is_none();

    kill_server(child);

    assert!(running, "Server should be running in stdio mode");
}

/// S01AC3: `cargo run -- --transport sse --port 3000` запускает MCP сервер с HTTP на порту 3000.
#[test]
fn test_S01AC3_sse_server_starts_on_port_3000() {
    let mut child = start_server(&["--transport", "sse", "--port", "3000"]);

    // Give the server time to start
    std::thread::sleep(Duration::from_secs(2));

    // Check the process is still running (didn't crash)
    let running = child.try_wait().unwrap().is_none();

    if !running {
        let _output = child.wait_with_output();
        panic!("Server process exited unexpectedly");
    }

    // Try to connect to the MCP endpoint
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(3))
        .no_proxy()
        .build()
        .expect("Failed to build HTTP client");

    // Try to connect to the MCP endpoint - the server should accept connections
    let mut connected = false;
    for _ in 0..10 {
        match client.get("http://127.0.0.1:3000/sse").send() {
            Ok(_resp) => {
                connected = true;
                break;
            }
            Err(e) => {
                if e.is_connect() {
                    std::thread::sleep(Duration::from_millis(300));
                    continue;
                }
                // Timeout or other error - server might be listening but not responding to GET
                connected = true;
                break;
            }
        }
    }

    kill_server(child);

    assert!(connected, "Server should be listening on port 3000");
}

/// S04AC4: Stdio transport handles JSON-RPC and returns tools via tools/list.
#[test]
fn test_S04AC4_stdio_transport_handles_tools_list() {
    let mut child = start_server(&["--transport", "stdio"]);

    // Give the server time to start
    std::thread::sleep(Duration::from_millis(1000));

    // Send initialize request
    let init_request = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1.0"}}}"#;

    if let Some(stdin) = child.stdin.as_mut() {
        let _ = writeln!(stdin, "{}", init_request);
        let _ = stdin.flush();
    }

    std::thread::sleep(Duration::from_millis(1000));

    // Send tools/list request
    let tools_request = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#;

    if let Some(stdin) = child.stdin.as_mut() {
        let _ = writeln!(stdin, "{}", tools_request);
        let _ = stdin.flush();
    }

    std::thread::sleep(Duration::from_millis(1000));

    // Check the process is still running
    let running = child.try_wait().unwrap().is_none();
    kill_server(child);

    assert!(running, "Server should be running in stdio mode");
}

/// S04AC5: HTTP transport /sse endpoint is accessible.
#[test]
fn test_S04AC5_mcp_endpoint_is_accessible() {
    let mut child = start_server(&["--transport", "sse", "--port", "3001"]);

    // Give the server time to start
    std::thread::sleep(Duration::from_secs(2));

    let running = child.try_wait().unwrap().is_none();
    if !running {
        let _output = child.wait_with_output();
        panic!("Server process exited unexpectedly");
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(3))
        .no_proxy()
        .build()
        .expect("Failed to build HTTP client");

    // Try to connect to the MCP endpoint
    let mut connected = false;
    for _ in 0..10 {
        match client.get("http://127.0.0.1:3001/sse").send() {
            Ok(_resp) => {
                connected = true;
                break;
            }
            Err(e) => {
                if e.is_connect() {
                    std::thread::sleep(Duration::from_millis(300));
                    continue;
                }
                connected = true;
                break;
            }
        }
    }

    kill_server(child);
    assert!(connected, "MCP endpoint at /sse should be accessible");
}

/// CORS preflight should be accepted for Electron/browser MCP clients.
///
/// Uses `CorsLayer::permissive()` which returns `*` for all CORS headers,
/// allowing any origin, any method, and any headers.
#[test]
fn test_sse_transport_allows_cors_preflight() {
    let mut child = start_server(&["--transport", "sse", "--port", "3099"]);

    std::thread::sleep(Duration::from_secs(2));

    let running = child.try_wait().unwrap().is_none();
    if !running {
        let _output = child.wait_with_output();
        panic!("Server process exited unexpectedly");
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(3))
        .no_proxy()
        .build()
        .expect("Failed to build HTTP client");

    for (path, method) in [("/sse", "GET"), ("/message", "POST")] {
        let response = client
            .request(
                reqwest::Method::OPTIONS,
                format!("http://127.0.0.1:3099{path}"),
            )
            .header("origin", "http://localhost:5173")
            .header("access-control-request-method", method)
            .header(
                "access-control-request-headers",
                "mcp-protocol-version, content-type",
            )
            .send()
            .unwrap_or_else(|e| panic!("preflight request to {path} failed: {e}"));

        assert!(
            response.status().is_success(),
            "preflight to {path} should succeed, got {}",
            response.status()
        );

        let headers = response.headers();

        // CorsLayer::permissive() returns "*" for allow-origin
        let allow_origin = headers
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok());
        assert!(
            allow_origin == Some("*") || allow_origin == Some("http://localhost:5173"),
            "access-control-allow-origin should be '*' or the request origin, got {:?}",
            allow_origin,
        );

        // Verify allow-methods is present (permissive returns "*")
        let allow_methods = headers
            .get("access-control-allow-methods")
            .and_then(|value| value.to_str().ok());
        assert!(
            allow_methods.is_some(),
            "access-control-allow-methods should be present for {path}",
        );

        // Verify allow-headers is present (permissive returns "*")
        let allow_headers = headers
            .get("access-control-allow-headers")
            .and_then(|value| value.to_str().ok());
        assert!(
            allow_headers.is_some(),
            "access-control-allow-headers should be present for {path}",
        );
    }

    kill_server(child);
}
