//! Calendar business logic service.

use crate::bridge::eventkit::EventKitBridge;
use crate::models::Calendar;

use super::{ServiceError, ServiceResult};

/// Business logic service for calendar operations.
///
/// Holds a reference to [`EventKitBridge`] and provides high-level methods
/// with validation and error handling for MCP tools.
pub struct CalendarService<'a> {
    bridge: &'a EventKitBridge,
}

impl<'a> CalendarService<'a> {
    /// Creates a new calendar service backed by the given bridge.
    pub fn new(bridge: &'a EventKitBridge) -> Self {
        Self { bridge }
    }

    /// Returns all calendars from macOS EventKit.
    pub fn list_calendars(&self) -> ServiceResult<Vec<Calendar>> {
        Ok(self.bridge.list_calendars()?)
    }

    /// Returns a calendar by its identifier.
    pub fn get_calendar(&self, id: &str) -> ServiceResult<Calendar> {
        // R3: validate id is not empty
        if id.trim().is_empty() {
            return Err(ServiceError::Validation(
                "calendar id must not be empty".to_string(),
            ));
        }

        self.bridge.find_calendar_by_id(id)?.ok_or_else(|| {
            ServiceError::from(crate::bridge::eventkit::BridgeError::CalendarNotFound(
                id.to_string(),
            ))
        })
    }

    /// Creates a new calendar with the given title and optional color.
    pub fn create_calendar(&self, title: &str, color: Option<&str>) -> ServiceResult<Calendar> {
        // R3: validate title is not empty
        let title = title.trim();
        if title.is_empty() {
            return Err(ServiceError::Validation(
                "title must not be empty".to_string(),
            ));
        }

        // R3: validate hex color format if provided
        if let Some(hex) = color {
            if !is_valid_hex_color(hex) {
                return Err(ServiceError::Validation(format!(
                    "invalid color format: '{}'. Expected #RRGGBB",
                    hex
                )));
            }
        }

        Ok(self.bridge.create_calendar(title, color)?)
    }

    /// Deletes a calendar by its identifier.
    pub fn delete_calendar(&self, id: &str) -> ServiceResult<()> {
        // R3: validate id is not empty
        if id.trim().is_empty() {
            return Err(ServiceError::Validation(
                "calendar id must not be empty".to_string(),
            ));
        }

        Ok(self.bridge.delete_calendar(id)?)
    }
}

/// Validate hex color format `#RRGGBB`.
fn is_valid_hex_color(s: &str) -> bool {
    let s = s.trim_start_matches('#');
    s.len() == 6 && s.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    #![allow(non_snake_case)]

    use super::*;

    /// Helper: create a real EventKitBridge for integration tests.
    /// Returns None if calendar access is not granted or not on main thread.
    fn try_create_bridge() -> Option<EventKitBridge> {
        let bridge = std::panic::catch_unwind(|| EventKitBridge::new())
            .ok()?
            .ok()?;
        let granted = bridge.request_access().ok()?;
        if granted {
            Some(bridge)
        } else {
            None
        }
    }

    // ------------------------------------------------------------------
    // S05AC1: CalendarService::list_calendars() returns all calendars
    // ------------------------------------------------------------------

    #[test]
    #[ignore = "requires an EventKit test harness running on the macOS main thread"]
    fn test_S05AC1_list_calendars_returns_calendars() {
        let Some(bridge) = try_create_bridge() else {
            eprintln!("SKIP: calendar access not granted, skipping integration test");
            return;
        };
        let service = CalendarService::new(&bridge);

        let result = service.list_calendars();
        assert!(
            result.is_ok(),
            "list_calendars should succeed: {:?}",
            result.err()
        );

        let calendars = result.unwrap();
        // Verify each calendar has required fields
        for cal in &calendars {
            assert!(!cal.id.is_empty(), "calendar id should not be empty");
        }
    }

    // ------------------------------------------------------------------
    // S05AC2: CalendarService::create_calendar() with empty title returns validation error
    // ------------------------------------------------------------------

    #[test]
    #[ignore = "requires an EventKit test harness running on the macOS main thread"]
    fn test_S05AC2_create_calendar_empty_title_returns_validation_error() {
        let Some(bridge) = try_create_bridge() else {
            eprintln!("SKIP: calendar access not granted, skipping integration test");
            return;
        };
        let service = CalendarService::new(&bridge);

        let result = service.create_calendar("", None);
        assert!(result.is_err(), "empty title should return error");

        let err = result.unwrap_err();
        let msg = format!("{}", err);
        assert!(
            msg.to_lowercase().contains("validation")
                || msg.to_lowercase().contains("empty")
                || msg.to_lowercase().contains("title"),
            "error should mention validation/title: got '{}'",
            msg
        );
    }

    // ------------------------------------------------------------------
    // S05AC3: CalendarService::delete_calendar() with non-existent ID returns CalendarNotFound
    // ------------------------------------------------------------------

    #[test]
    #[ignore = "requires an EventKit test harness running on the macOS main thread"]
    fn test_S05AC3_delete_calendar_nonexistent_id_returns_not_found() {
        let Some(bridge) = try_create_bridge() else {
            eprintln!("SKIP: calendar access not granted, skipping integration test");
            return;
        };
        let service = CalendarService::new(&bridge);

        let result = service.delete_calendar("nonexistent-calendar-id-12345");
        assert!(result.is_err(), "non-existent calendar should return error");

        let err = format!("{}", result.unwrap_err());
        assert!(
            err.to_lowercase().contains("not found"),
            "error should mention 'not found': got '{}'",
            err
        );
    }
}
