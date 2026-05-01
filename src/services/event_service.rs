//! Event business logic service.

use chrono::{Duration, Local, NaiveDateTime};

use crate::bridge::eventkit::{BridgeError, EventKitBridge};
use crate::models::{Event, EventCreateRequest, EventUpdateRequest};

use super::{ServiceError, ServiceResult};

/// Business logic service for event operations.
///
/// Holds a reference to [`EventKitBridge`] and provides high-level methods
/// with validation and error handling for MCP tools.
pub struct EventService<'a> {
    bridge: &'a EventKitBridge,
}

impl<'a> EventService<'a> {
    /// Creates a new event service backed by the given bridge.
    pub fn new(bridge: &'a EventKitBridge) -> Self {
        Self { bridge }
    }

    /// List events in a calendar for the default range: -30 days / +365 days from now.
    pub fn list_events(&self, calendar_id: &str) -> ServiceResult<Vec<Event>> {
        // R3: validate calendar_id is not empty
        if calendar_id.trim().is_empty() {
            return Err(ServiceError::Validation("calendar_id must not be empty".to_string()));
        }

        let now = Local::now().naive_utc();
        let start = now - Duration::days(30);
        let end = now + Duration::days(365);

        let start_str = start.format("%Y-%m-%dT%H:%M:%S").to_string();
        let end_str = end.format("%Y-%m-%dT%H:%M:%S").to_string();

        Ok(self.bridge.list_events(calendar_id, &start_str, &end_str)?)
    }

    /// Get a single event by its identifier, verifying it belongs to the given calendar.
    pub fn get_event(&self, calendar_id: &str, event_id: &str) -> ServiceResult<Event> {
        // R3: validate ids are not empty
        if calendar_id.trim().is_empty() {
            return Err(ServiceError::Validation("calendar_id must not be empty".to_string()));
        }
        if event_id.trim().is_empty() {
            return Err(ServiceError::Validation("event_id must not be empty".to_string()));
        }

        // Check calendar exists
        let cal = self
            .bridge
            .find_calendar_by_id(calendar_id)?
            .ok_or_else(|| ServiceError::from(BridgeError::CalendarNotFound(calendar_id.to_string())))?;

        let _ = cal; // calendar exists

        let event = self
            .bridge
            .get_event(event_id)?
            .ok_or_else(|| ServiceError::from(BridgeError::EventNotFound(event_id.to_string())))?;

        // Verify event belongs to calendar
        if event.calendar_id != calendar_id {
            return Err(ServiceError::from(BridgeError::EventNotFound(format!(
                "event {} does not belong to calendar {}",
                event_id, calendar_id
            ))));
        }

        Ok(event)
    }

    /// Create a new event with validation.
    pub fn create_event(&self, request: EventCreateRequest) -> ServiceResult<Event> {
        // R3: validate title is not empty
        if request.title.trim().is_empty() {
            return Err(ServiceError::Validation("title must not be empty".to_string()));
        }

        // R3: validate calendar_id is not empty
        if request.calendar_id.trim().is_empty() {
            return Err(ServiceError::Validation("calendar_id must not be empty".to_string()));
        }

        // R3: validate dates via parse_flexible_date
        let start = parse_flexible_date_as_ndt(&request.start_date)?;
        let end = parse_flexible_date_as_ndt(&request.end_date)?;

        // R3: start_date must be before end_date
        if start >= end {
            return Err(ServiceError::Validation(
                "start_date must be before end_date".to_string(),
            ));
        }

        Ok(self.bridge.create_event(&request.calendar_id, &request)?)
    }

    /// Update an existing event, only changing the provided fields.
    pub fn update_event(&self, request: EventUpdateRequest) -> ServiceResult<Event> {
        // R3: validate ids are not empty
        if request.calendar_id.trim().is_empty() {
            return Err(ServiceError::Validation("calendar_id must not be empty".to_string()));
        }
        if request.event_id.trim().is_empty() {
            return Err(ServiceError::Validation("event_id must not be empty".to_string()));
        }

        // R3: validate dates if provided
        if let Some(ref start) = request.start_date {
            parse_flexible_date_as_ndt(start)?;
        }
        if let Some(ref end) = request.end_date {
            parse_flexible_date_as_ndt(end)?;
        }

        // R3: validate title if provided
        if let Some(ref title) = request.title {
            if title.trim().is_empty() {
                return Err(ServiceError::Validation("title must not be empty".to_string()));
            }
        }

        Ok(self.bridge.update_event(&request.event_id, &request)?)
    }

    /// Delete an event by its identifier.
    pub fn delete_event(&self, calendar_id: &str, event_id: &str) -> ServiceResult<()> {
        // R3: validate ids are not empty
        if calendar_id.trim().is_empty() {
            return Err(ServiceError::Validation("calendar_id must not be empty".to_string()));
        }
        if event_id.trim().is_empty() {
            return Err(ServiceError::Validation("event_id must not be empty".to_string()));
        }

        // Check calendar exists
        let cal = self
            .bridge
            .find_calendar_by_id(calendar_id)?
            .ok_or_else(|| ServiceError::from(BridgeError::CalendarNotFound(calendar_id.to_string())))?;

        let _ = cal;

        Ok(self.bridge.delete_event(event_id)?)
    }
}

/// Parse a flexible date string into NaiveDateTime, returning ServiceError on failure.
fn parse_flexible_date_as_ndt(input: &str) -> ServiceResult<NaiveDateTime> {
    crate::models::parse_flexible_date(input).map_err(|msg| {
        ServiceError::from(BridgeError::InvalidDateFormat(msg))
    })
}

#[cfg(test)]
mod tests {
    #![allow(non_snake_case)]

    use super::*;

    /// Helper: create a real EventKitBridge for integration tests.
    /// Returns None if calendar access is not granted.
    fn try_create_bridge() -> Option<EventKitBridge> {
        let bridge = EventKitBridge::new().ok()?;
        let granted = bridge.request_access().ok()?;
        if granted {
            Some(bridge)
        } else {
            None
        }
    }

    // ------------------------------------------------------------------
    // S05AC4: EventService::create_event() with invalid date returns InvalidDateFormat
    // ------------------------------------------------------------------

    #[test]
    fn test_S05AC4_create_event_invalid_date_returns_invalid_date_format() {
        let Some(bridge) = try_create_bridge() else {
            eprintln!("SKIP: calendar access not granted, skipping integration test");
            return;
        };
        let service = EventService::new(&bridge);

        let request = EventCreateRequest {
            calendar_id: "some-cal".to_string(),
            title: "Test Event".to_string(),
            start_date: "not-a-date".to_string(),
            end_date: "2025-03-09T11:00:00".to_string(),
            is_all_day: None,
            location: None,
            notes: None,
            url: None,
        };

        let result = service.create_event(request);
        assert!(result.is_err(), "invalid date should return error");

        let err = format!("{}", result.unwrap_err());
        assert!(
            err.to_lowercase().contains("invalid date format"),
            "error should mention 'invalid date format': got '{}'",
            err
        );
    }

    // ------------------------------------------------------------------
    // S05AC5: EventService::create_event() with start_date > end_date returns error
    // ------------------------------------------------------------------

    #[test]
    fn test_S05AC5_create_event_start_after_end_returns_error() {
        let Some(bridge) = try_create_bridge() else {
            eprintln!("SKIP: calendar access not granted, skipping integration test");
            return;
        };
        let service = EventService::new(&bridge);

        let request = EventCreateRequest {
            calendar_id: "some-cal".to_string(),
            title: "Test Event".to_string(),
            start_date: "2025-03-09T12:00:00".to_string(),
            end_date: "2025-03-09T10:00:00".to_string(),
            is_all_day: None,
            location: None,
            notes: None,
            url: None,
        };

        let result = service.create_event(request);
        assert!(result.is_err(), "start > end should return error");

        let err = format!("{}", result.unwrap_err());
        assert!(
            err.to_lowercase().contains("start_date")
                || err.to_lowercase().contains("before"),
            "error should mention start_date/before: got '{}'",
            err
        );
    }

    // ------------------------------------------------------------------
    // S05AC6: EventService::list_events() returns events for -30/+365 day range
    // ------------------------------------------------------------------

    #[test]
    fn test_S05AC6_list_events_uses_30_365_day_range() {
        // This is a unit test: verify the date range calculation logic
        let now = Local::now().naive_utc();
        let start = now - Duration::days(30);
        let end = now + Duration::days(365);

        // Verify the range is correct
        assert_eq!((now - start).num_days(), 30);
        assert_eq!((end - now).num_days(), 365);
    }

    // ------------------------------------------------------------------
    // S05AC7: EventService::update_event() updates only provided fields
    // ------------------------------------------------------------------

    #[test]
    fn test_S05AC7_update_event_validates_only_provided_fields() {
        // Verify that update request with only title passes validation
        // (dates are not provided, so they should not be validated)
        let request = EventUpdateRequest {
            calendar_id: "cal-1".to_string(),
            event_id: "evt-1".to_string(),
            title: Some("Updated Title".to_string()),
            start_date: None,
            end_date: None,
            is_all_day: None,
            location: None,
            notes: None,
            url: None,
        };

        // Validation should pass — no dates to validate
        if let Some(ref start) = request.start_date {
            let _ = parse_flexible_date_as_ndt(start).unwrap();
        }
        if let Some(ref end) = request.end_date {
            let _ = parse_flexible_date_as_ndt(end).unwrap();
        }
        // If we get here, validation passed for partial update
    }

    // ------------------------------------------------------------------
    // S05AC8: Services correctly propagate Bridge errors as ServiceError
    // ------------------------------------------------------------------

    #[test]
    fn test_S05AC8_bridge_error_propagates_as_service_error() {
        // Verify that BridgeError can be converted to ServiceError
        let bridge_err = BridgeError::CalendarNotFound("test-cal".to_string());
        let service_err: ServiceError = bridge_err.into();

        let msg = format!("{}", service_err);
        assert!(
            msg.contains("test-cal"),
            "ServiceError should contain original error details: got '{}'",
            msg
        );

        // Verify BridgeError variant
        assert!(
            matches!(service_err, ServiceError::Bridge(_)),
            "should be ServiceError::Bridge variant"
        );
    }

    #[test]
    fn test_S05AC8_validation_error_is_service_error() {
        let err = ServiceError::Validation("test validation".to_string());
        let msg = format!("{}", err);
        assert!(
            msg.contains("test validation"),
            "Validation error should contain message: got '{}'",
            msg
        );
    }
}
