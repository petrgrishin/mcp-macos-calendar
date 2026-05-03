# Spec 08: Read-Only Mode Flag

**Metadata:**
- Priority: 8
- Status: Done
- Effort: S (<10 min)

## Overview
### Problem Statement
MCP сервер предоставляет 7 инструментов: 2 для чтения (`getCalendars`, `getCalendarEvents`) и 5 для мутации (`createCalendar`, `deleteCalendar`, `createCalendarEvent`, `updateCalendarEvent`, `deleteCalendarEvent`). В некоторых сценариях использования (демо, аудит, безопасный доступ) требуется ограничить сервер только операциями чтения, чтобы MCP-клиент не мог изменять или удалять календари и события.

### Solution Summary
Добавить CLI-флаг `--read-only` в [`CliArgs`](src/config.rs:40). При включении этого флага:
- [`calendar_tools()`](src/server.rs:176) возвращает только read-only инструменты
- [`dispatch_tool()`](src/server.rs:112) отклоняет вызовы mutation-инструментов с ошибкой
- При старте сервер логирует режим read-only

## Requirements
### R1: CLI-флаг `--read-only`
- Добавить поле `read_only: bool` в [`CliArgs`](src/config.rs:40) с `#[arg(long, default_value_t = false)]`
- Добавить поле `read_only: bool` в [`ServerConfig`](src/config.rs:59)
- Передавать значение из `CliArgs` в `ServerConfig` в impl [`From<CliArgs>`](src/config.rs:67)

### R2: Фильтрация инструментов в `handle_list_tools_request`
- Передать флаг `read_only` в [`CalendarMcpHandler`](src/server.rs:19) (новое поле `read_only: bool`)
- В [`handle_list_tools_request`](src/server.rs:42): если `read_only == true`, вернуть только `getCalendars` и `getCalendarEvents`
- Если `read_only == false` (по умолчанию), вернуть все 7 инструментов — текущее поведение

### R3: Защита в `dispatch_tool`
- В [`dispatch_tool`](src/server.rs:112): если `read_only == true` и вызывается mutation-инструмент (`createCalendar`, `deleteCalendar`, `createCalendarEvent`, `updateCalendarEvent`, `deleteCalendarEvent`), вернуть ошибку `CallToolError`
- Это защита на случай, если клиент вызовет tool по имени напрямую, минуя `list_tools`

### R4: Логирование режима
- При старте сервера в [`main.rs`](src/main.rs:24): если `read_only == true`, логировать `"Running in read-only mode"` через `tracing::info!`

### R5: Передача флага в handler
- В [`run_stdio`](src/main.rs:63) и [`run_sse`](src/main.rs:105): передавать `config.read_only` в конструктор `CalendarMcpHandler`
- Добавить метод `CalendarMcpHandler::with_bridge_and_read_only(bridge, read_only)` или расширить существующий [`with_bridge`](src/server.rs:33)

## Acceptance Criteria
- [x] S08AC1: CLI-флаг `--read-only` парсится корректно, по умолчанию `false`
- [x] S08AC2: При `--read-only` вызов `handle_list_tools_request` возвращает только 2 инструмента: `getCalendars` и `getCalendarEvents`
- [x] S08AC3: Без флага `--read-only` `handle_list_tools_request` возвращает все 7 инструментов (обратная совместимость)
- [x] S08AC4: При `--read-only` вызов mutation-инструмента через `dispatch_tool` возвращает ошибку `CallToolError`
- [x] S08AC5: При старте с `--read-only` в логе присутствует сообщение `"Running in read-only mode"`

## Implementation Notes
- Функция `calendar_tools()` изменена на `calendar_tools(read_only: bool)` — параметр управляет фильтрацией.
- Функция `dispatch_tool()` получила 4-й параметр `read_only: bool` — при `true` mutation-инструменты отклоняются с `CallToolError`.
- Добавлен конструктор `CalendarMcpHandler::with_bridge_and_read_only(bridge, read_only)`.
- Существующий конструктор `with_bridge()` сохранён для обратной совместимости (использует `read_only: false`).
- 5 существующих тестов spec-05 падают из-за требования main thread для EventKitBridge — это предсуществующая проблема, не связанная с данным спеком.
