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
    let init_request = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"0.1.0"}}}"#;
    
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

/// S01AC3: `cargo run -- --transport sse --port 3000` запускает MCP сервер с SSE на порту 3000.
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
        match client.get("http://127.0.0.1:3000/mcp").send() {
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
