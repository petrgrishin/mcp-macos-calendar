# MCP macOS Calendar

MCP macOS Calendar is a Rust MCP server that exposes Apple Calendar data through EventKit. It supports stdio for desktop MCP clients and HTTP transport for local browser or Electron clients.

The server can list calendars, read calendar events, and optionally create, update, or delete calendars and events. Use `--read-only` when a client should only be allowed to inspect calendar data.

## Features

- Native macOS Calendar access through EventKit.
- MCP tools for calendars and events.
- Stdio transport for Claude Desktop and similar clients.
- Legacy SSE endpoints at `/sse` and `/message`.
- Streamable HTTP endpoint at `/mcp`.
- Optional read-only mode that hides mutation tools.
- Embedded `Info.plist` calendar usage description for macOS permission prompts.

## Requirements

- macOS with Calendar access available.
- Rust 1.80 or newer.
- Xcode Command Line Tools for coverage reporting with `xcrun llvm-profdata` and `xcrun llvm-cov`.

## Build

```bash
cargo build --release
```

The release binary is written to:

```text
target/release/mcp-macos-calendar
```

## Run

Run over stdio:

```bash
cargo run -- --transport stdio
```

Run over HTTP on the default host and port:

```bash
cargo run -- --transport sse
```

Run over HTTP on a custom host and port:

```bash
cargo run -- --transport sse --host 127.0.0.1 --port 8080
```

Run in read-only mode:

```bash
cargo run -- --transport stdio --read-only
```

When macOS asks for permission, grant Calendar access in System Settings, Privacy & Security, Calendars.

## MCP Endpoints

When started with `--transport sse --host 127.0.0.1 --port 8080`:

- Legacy SSE: `http://127.0.0.1:8080/sse`
- Legacy message POST: `http://127.0.0.1:8080/message`
- Streamable HTTP: `http://127.0.0.1:8080/mcp`

## Claude Desktop

Stdio configuration:

```json
{
  "mcpServers": {
    "macos-calendar": {
      "command": "/absolute/path/to/mcp-macos-calendar",
      "args": ["--transport", "stdio"]
    }
  }
}
```

Read-only stdio configuration:

```json
{
  "mcpServers": {
    "macos-calendar": {
      "command": "/absolute/path/to/mcp-macos-calendar",
      "args": ["--transport", "stdio", "--read-only"]
    }
  }
}
```

HTTP/SSE configuration:

```json
{
  "mcpServers": {
    "macos-calendar": {
      "url": "http://127.0.0.1:8080/sse"
    }
  }
}
```

Use the built binary path from `target/release/mcp-macos-calendar`, or install/copy the binary to a stable location and point `command` there.

## Tools

| Tool | Mode | Description |
| --- | --- | --- |
| `getCalendars` | read | List available macOS calendars. |
| `getCalendarEvents` | read | List events from a calendar with optional date filters and pagination. |
| `createCalendar` | write | Create a new calendar. |
| `deleteCalendar` | write | Delete a calendar. |
| `createCalendarEvent` | write | Create an event in a calendar. |
| `updateCalendarEvent` | write | Update selected event fields. |
| `deleteCalendarEvent` | write | Delete an event. |

In `--read-only` mode, only `getCalendars` and `getCalendarEvents` are exposed.

## Tool Parameters

`getCalendars` takes no parameters.

`getCalendarEvents`:

```json
{
  "calendar_id": "calendar-id",
  "start_date": "2026-06-01T00:00:00",
  "end_date": "2026-06-30T23:59:59",
  "limit": 100,
  "offset": 0
}
```

`start_date`, `end_date`, `limit`, and `offset` are optional. Without dates, the server uses a default window from 30 days before now to 30 days after now. `limit` defaults to 100 and cannot exceed 1000.

`createCalendar`:

```json
{
  "title": "Work",
  "color": "#FF0000"
}
```

`deleteCalendar`:

```json
{
  "calendar_id": "calendar-id"
}
```

`createCalendarEvent`:

```json
{
  "calendar_id": "calendar-id",
  "title": "Planning",
  "start_date": "2026-06-15T10:00:00",
  "end_date": "2026-06-15T11:00:00",
  "location": "Office",
  "notes": "Weekly planning"
}
```

`updateCalendarEvent`:

```json
{
  "calendar_id": "calendar-id",
  "event_id": "event-id",
  "title": "Updated planning",
  "start_date": "2026-06-15T10:30:00",
  "end_date": "2026-06-15T11:30:00",
  "location": "Conference room",
  "notes": "Bring notes"
}
```

`deleteCalendarEvent`:

```json
{
  "calendar_id": "calendar-id",
  "event_id": "event-id"
}
```

## Test

Run all tests:

```bash
cargo test
```

Run integration tests only:

```bash
cargo test --test integration_main
```

Some service tests touch EventKit. If calendar access is unavailable, those tests print a skip message and return without failing.

## Coverage

See [COVERAGE.md](COVERAGE.md) for the current coverage report and the exact commands used to regenerate it.

```bash
just coverage
just coverage --update-coverage-md
```

## Development

Useful `just` recipes:

```bash
just build
just test
just run-stdio
just run-sse
just lint
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
