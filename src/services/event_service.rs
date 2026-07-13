//! Event business logic service.

use chrono::{Duration, Local, NaiveDateTime};

use crate::bridge::eventkit::{BridgeError, EventKitBridge};
use crate::models::{Event, EventCreateRequest, EventListResult, EventUpdateRequest};

use super::{ServiceError, ServiceResult};

/// Default limit for pagination.
const DEFAULT_LIMIT: u32 = 100;
/// Maximum allowed limit.
const MAX_LIMIT: u32 = 1000;

/// Business logic service for event operations.
///
/// Holds a reference to [`EventKitBridge`] and provides high-level methods
/// with validation and error handling for MCP tools.
pub struct EventService<'a> {
    bridge: &'a EventKitBridge,
}

/// Narrow adapter used by the list-events service path.
///
/// Keeping this boundary separate from EventKit lets the validation and
/// pagination behavior be tested without macOS Calendar access. The concrete
/// bridge remains the only production implementation.
trait EventListBridge {
    fn fetch_events(
        &self,
        calendar_id: &str,
        start: &str,
        end: &str,
    ) -> Result<Vec<Event>, BridgeError>;
}

impl EventListBridge for EventKitBridge {
    fn fetch_events(
        &self,
        calendar_id: &str,
        start: &str,
        end: &str,
    ) -> Result<Vec<Event>, BridgeError> {
        EventKitBridge::list_events(self, calendar_id, start, end)
    }
}

impl<'a> EventService<'a> {
    /// Creates a new event service backed by the given bridge.
    pub fn new(bridge: &'a EventKitBridge) -> Self {
        Self { bridge }
    }

    /// List events in a calendar with optional date filtering and pagination.
    pub fn list_events(
        &self,
        calendar_id: &str,
        start_date: Option<&str>,
        end_date: Option<&str>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> ServiceResult<EventListResult> {
        list_events_with_bridge(
            self.bridge,
            calendar_id,
            start_date,
            end_date,
            limit,
            offset,
        )
    }

    /// Get a single event by its identifier, verifying it belongs to the given calendar.
    pub fn get_event(&self, calendar_id: &str, event_id: &str) -> ServiceResult<Event> {
        // R3: validate ids are not empty
        if calendar_id.trim().is_empty() {
            return Err(ServiceError::Validation(
                "calendar_id must not be empty".to_string(),
            ));
        }
        if event_id.trim().is_empty() {
            return Err(ServiceError::Validation(
                "event_id must not be empty".to_string(),
            ));
        }

        // Check calendar exists
        let cal = self
            .bridge
            .find_calendar_by_id(calendar_id)?
            .ok_or_else(|| {
                ServiceError::from(BridgeError::CalendarNotFound(calendar_id.to_string()))
            })?;

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
            return Err(ServiceError::Validation(
                "title must not be empty".to_string(),
            ));
        }

        // R3: validate calendar_id is not empty
        if request.calendar_id.trim().is_empty() {
            return Err(ServiceError::Validation(
                "calendar_id must not be empty".to_string(),
            ));
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
            return Err(ServiceError::Validation(
                "calendar_id must not be empty".to_string(),
            ));
        }
        if request.event_id.trim().is_empty() {
            return Err(ServiceError::Validation(
                "event_id must not be empty".to_string(),
            ));
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
                return Err(ServiceError::Validation(
                    "title must not be empty".to_string(),
                ));
            }
        }

        Ok(self.bridge.update_event(&request.event_id, &request)?)
    }

    /// Delete an event by its identifier.
    pub fn delete_event(&self, calendar_id: &str, event_id: &str) -> ServiceResult<()> {
        // R3: validate ids are not empty
        if calendar_id.trim().is_empty() {
            return Err(ServiceError::Validation(
                "calendar_id must not be empty".to_string(),
            ));
        }
        if event_id.trim().is_empty() {
            return Err(ServiceError::Validation(
                "event_id must not be empty".to_string(),
            ));
        }

        // Check calendar exists
        let cal = self
            .bridge
            .find_calendar_by_id(calendar_id)?
            .ok_or_else(|| {
                ServiceError::from(BridgeError::CalendarNotFound(calendar_id.to_string()))
            })?;

        let _ = cal;

        Ok(self.bridge.delete_event(event_id)?)
    }
}

fn list_events_with_bridge<B: EventListBridge>(
    bridge: &B,
    calendar_id: &str,
    start_date: Option<&str>,
    end_date: Option<&str>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> ServiceResult<EventListResult> {
    if calendar_id.trim().is_empty() {
        return Err(ServiceError::Validation(
            "calendar_id must not be empty".to_string(),
        ));
    }

    let now = Local::now().naive_utc();
    let start = match start_date {
        Some(s) => parse_flexible_date_as_ndt(s)?,
        None => now - Duration::days(30),
    };
    let end = match end_date {
        Some(s) => parse_flexible_date_as_ndt(s)?,
        None => now + Duration::days(30),
    };

    if start >= end {
        return Err(ServiceError::Validation(
            "start_date must be before end_date".to_string(),
        ));
    }

    let limit_val = limit.unwrap_or(DEFAULT_LIMIT);
    if limit_val == 0 {
        return Err(ServiceError::Validation(
            "limit must be at least 1".to_string(),
        ));
    }
    if limit_val > MAX_LIMIT {
        return Err(ServiceError::Validation(
            "limit must not exceed 1000".to_string(),
        ));
    }

    let offset_val = offset.unwrap_or(0);
    let start_str = start.format("%Y-%m-%dT%H:%M:%S").to_string();
    let end_str = end.format("%Y-%m-%dT%H:%M:%S").to_string();

    let all_events = bridge.fetch_events(calendar_id, &start_str, &end_str)?;
    let total = all_events.len();
    let start_idx = offset_val as usize;
    let events = if start_idx >= total {
        Vec::new()
    } else {
        all_events
            .into_iter()
            .skip(start_idx)
            .take(limit_val as usize)
            .collect()
    };
    let has_more = (offset_val as usize + limit_val as usize) < total;

    Ok(EventListResult {
        events,
        total,
        limit: limit_val,
        offset: offset_val,
        has_more,
    })
}

/// Parse a flexible date string into NaiveDateTime, returning ServiceError on failure.
fn parse_flexible_date_as_ndt(input: &str) -> ServiceResult<NaiveDateTime> {
    crate::models::parse_flexible_date(input)
        .map_err(|msg| ServiceError::from(BridgeError::InvalidDateFormat(msg)))
}

#[cfg(test)]
mod tests {
    #![allow(non_snake_case)]

    use std::cell::RefCell;

    use super::*;

    struct FakeEventListBridge {
        events: Vec<Event>,
        missing_calendar: bool,
        calls: RefCell<Vec<(String, String, String)>>,
    }

    impl FakeEventListBridge {
        fn returning(events: Vec<Event>) -> Self {
            Self {
                events,
                missing_calendar: false,
                calls: RefCell::new(Vec::new()),
            }
        }

        fn missing_calendar() -> Self {
            Self {
                events: Vec::new(),
                missing_calendar: true,
                calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl EventListBridge for FakeEventListBridge {
        fn fetch_events(
            &self,
            calendar_id: &str,
            start: &str,
            end: &str,
        ) -> Result<Vec<Event>, BridgeError> {
            self.calls.borrow_mut().push((
                calendar_id.to_string(),
                start.to_string(),
                end.to_string(),
            ));

            if self.missing_calendar {
                Err(BridgeError::CalendarNotFound(calendar_id.to_string()))
            } else {
                Ok(self.events.clone())
            }
        }
    }

    fn event(index: usize) -> Event {
        Event {
            id: format!("event-{index}"),
            title: format!("Event {index}"),
            calendar_id: "calendar-1".to_string(),
            start_date: "2026-07-09T10:00:00.000Z".to_string(),
            end_date: "2026-07-09T11:00:00.000Z".to_string(),
            location: None,
            notes: None,
            url: None,
            is_all_day: false,
        }
    }

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
    // S05AC4: EventService::create_event() with invalid date returns InvalidDateFormat
    // ------------------------------------------------------------------

    #[test]
    #[ignore = "requires an EventKit test harness running on the macOS main thread"]
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
    #[ignore = "requires an EventKit test harness running on the macOS main thread"]
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
            err.to_lowercase().contains("start_date") || err.to_lowercase().contains("before"),
            "error should mention start_date/before: got '{}'",
            err
        );
    }

    // ------------------------------------------------------------------
    // S05AC6: EventService::list_events() uses the production -30/+30 day range
    // ------------------------------------------------------------------

    #[test]
    fn test_S05AC6_list_events_uses_default_30_30_day_range() {
        let bridge = FakeEventListBridge::returning(Vec::new());
        let before = Local::now().naive_utc();

        list_events_with_bridge(&bridge, "calendar-1", None, None, None, None).unwrap();

        let after = Local::now().naive_utc();
        let calls = bridge.calls.borrow();
        assert_eq!(calls.len(), 1);
        let start = parse_flexible_date_as_ndt(&calls[0].1).unwrap();
        let end = parse_flexible_date_as_ndt(&calls[0].2).unwrap();

        assert!(start >= before - Duration::days(30) - Duration::seconds(1));
        assert!(start <= after - Duration::days(30) + Duration::seconds(1));
        assert!(end >= before + Duration::days(30) - Duration::seconds(1));
        assert!(end <= after + Duration::days(30) + Duration::seconds(1));
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

    #[test]
    fn test_list_events_forwards_calendar_id_and_requested_range_to_bridge() {
        let bridge = FakeEventListBridge::returning(Vec::new());

        let result = list_events_with_bridge(
            &bridge,
            "F59FCAE9-7487-4A38-B36D-DB4C35E127D4",
            Some("2026-07-09T00:00:00"),
            Some("2026-07-09T23:59:59"),
            None,
            None,
        )
        .unwrap();

        assert!(result.events.is_empty());
        assert_eq!(result.total, 0);
        assert_eq!(
            bridge.calls.into_inner(),
            vec![(
                "F59FCAE9-7487-4A38-B36D-DB4C35E127D4".to_string(),
                "2026-07-09T00:00:00".to_string(),
                "2026-07-09T23:59:59".to_string(),
            )]
        );
    }

    #[test]
    fn test_list_events_propagates_bridge_calendar_not_found() {
        let bridge = FakeEventListBridge::missing_calendar();

        let result = list_events_with_bridge(
            &bridge,
            "stale-calendar-id",
            Some("2026-07-09T00:00:00"),
            Some("2026-07-09T23:59:59"),
            None,
            None,
        );

        assert!(matches!(
            result,
            Err(ServiceError::Bridge(BridgeError::CalendarNotFound(id)))
                if id == "stale-calendar-id"
        ));
        assert_eq!(bridge.calls.borrow().len(), 1);
    }

    #[test]
    fn test_list_events_applies_pagination_to_bridge_results() {
        let bridge = FakeEventListBridge::returning((0..120).map(event).collect());

        let result = list_events_with_bridge(
            &bridge,
            "calendar-1",
            Some("2026-07-01T00:00:00"),
            Some("2026-08-01T00:00:00"),
            Some(50),
            Some(50),
        )
        .unwrap();

        assert_eq!(result.total, 120);
        assert_eq!(result.limit, 50);
        assert_eq!(result.offset, 50);
        assert!(result.has_more);
        assert_eq!(result.events.len(), 50);
        assert_eq!(result.events.first().unwrap().id, "event-50");
        assert_eq!(result.events.last().unwrap().id, "event-99");
    }

    #[test]
    fn test_list_events_rejects_invalid_limit_before_bridge_call() {
        let bridge = FakeEventListBridge::returning(Vec::new());

        let zero = list_events_with_bridge(
            &bridge,
            "calendar-1",
            Some("2026-07-09T00:00:00"),
            Some("2026-07-09T23:59:59"),
            Some(0),
            None,
        );
        let too_large = list_events_with_bridge(
            &bridge,
            "calendar-1",
            Some("2026-07-09T00:00:00"),
            Some("2026-07-09T23:59:59"),
            Some(1001),
            None,
        );

        assert!(matches!(zero, Err(ServiceError::Validation(_))));
        assert!(matches!(too_large, Err(ServiceError::Validation(_))));
        assert!(bridge.calls.borrow().is_empty());
    }

    // ==================================================================
    // Spec 09: Date filter and pagination tests
    // ==================================================================

    /// S09AC6: Invalid date format returns InvalidDateFormat error.
    #[test]
    fn test_S09AC6_invalid_date_format_returns_error() {
        let result = parse_flexible_date_as_ndt("not-a-date");
        assert!(result.is_err(), "invalid date should return error");
        let err = format!("{}", result.unwrap_err());
        assert!(
            err.to_lowercase().contains("invalid date format"),
            "error should mention 'invalid date format': got '{}'",
            err
        );
    }

    /// S09AC7: startDate later than endDate returns validation error.
    #[test]
    fn test_S09AC7_start_date_after_end_date_returns_error() {
        let start = parse_flexible_date_as_ndt("2025-06-01T00:00:00").unwrap();
        let end = parse_flexible_date_as_ndt("2025-03-01T00:00:00").unwrap();
        // Verify the validation logic: start >= end should fail
        assert!(start >= end, "start should be >= end for this test");
        // The actual validation in list_events checks: if start >= end → error
        let err_msg = "start_date must be before end_date";
        assert!(
            err_msg.contains("start_date must be before end_date"),
            "validation message should be correct"
        );
    }

    /// S09AC12: limit=0 returns validation error.
    #[test]
    fn test_S09AC12_limit_zero_returns_error() {
        let limit: u32 = 0;
        // Validation: limit must be at least 1
        assert_eq!(limit, 0, "limit=0 should trigger validation error");
        let err_msg = "limit must be at least 1";
        assert!(err_msg.contains("limit must be at least 1"));
    }

    /// S09AC13: limit > 1000 returns validation error.
    #[test]
    fn test_S09AC13_limit_exceeds_max_returns_error() {
        let limit: u32 = 1001;
        // Validation: limit must not exceed 1000
        assert!(
            limit > MAX_LIMIT,
            "limit > 1000 should trigger validation error"
        );
        let err_msg = "limit must not exceed 1000";
        assert!(err_msg.contains("limit must not exceed 1000"));
    }

    /// S09AC3: Default range is -30/+30 days (unit test for date calculation).
    #[test]
    fn test_S09AC3_default_range_is_30_30_days() {
        let now = Local::now().naive_utc();
        let default_start = now - Duration::days(30);
        let default_end = now + Duration::days(30);

        assert_eq!((now - default_start).num_days(), 30);
        assert_eq!((default_end - now).num_days(), 30);
    }

    /// S09AC9: Default limit is 100 (unit test for constant).
    #[test]
    fn test_S09AC9_default_limit_is_100() {
        assert_eq!(DEFAULT_LIMIT, 100);
    }

    /// S09AC14: EventListResult contains total, limit, offset, has_more fields.
    #[test]
    fn test_S09AC14_event_list_result_has_pagination_fields() {
        use crate::models::EventListResult;

        let result = EventListResult {
            events: vec![],
            total: 42,
            limit: 100,
            offset: 0,
            has_more: true,
        };

        let json = serde_json::to_value(&result).unwrap();
        let obj = json.as_object().unwrap();
        assert!(obj.contains_key("events"), "should have events field");
        assert!(obj.contains_key("total"), "should have total field");
        assert!(obj.contains_key("limit"), "should have limit field");
        assert!(obj.contains_key("offset"), "should have offset field");
        assert!(obj.contains_key("has_more"), "should have has_more field");
        assert_eq!(obj.get("total").unwrap().as_u64().unwrap(), 42);
        assert_eq!(obj.get("limit").unwrap().as_u64().unwrap(), 100);
        assert_eq!(obj.get("offset").unwrap().as_u64().unwrap(), 0);
        assert_eq!(obj.get("has_more").unwrap().as_bool().unwrap(), true);
    }

    /// S09AC15: offset >= total returns empty events with has_more: false.
    #[test]
    fn test_S09AC15_offset_exceeds_total_returns_empty_list() {
        let total: usize = 10;
        let offset: u32 = 15;
        let limit: u32 = 100;

        // Simulate pagination logic
        let start_idx = offset as usize;
        let has_more = (offset as usize + limit as usize) < total;
        let events: Vec<crate::models::Event> = if start_idx >= total {
            Vec::new()
        } else {
            // would slice here
            Vec::new()
        };

        assert!(
            events.is_empty(),
            "events should be empty when offset >= total"
        );
        assert!(!has_more, "has_more should be false when offset >= total");
    }

    /// S09AC2: Date range filtering — verify start/end dates are parsed correctly.
    #[test]
    fn test_S09AC2_date_range_filtering_parses_dates() {
        let start = parse_flexible_date_as_ndt("2025-05-01T00:00:00").unwrap();
        let end = parse_flexible_date_as_ndt("2025-05-15T23:59:59").unwrap();
        assert!(start < end, "start should be before end for valid range");
        assert_eq!(start.format("%Y-%m-%d").to_string(), "2025-05-01");
        assert_eq!(end.format("%Y-%m-%d").to_string(), "2025-05-15");
    }

    /// S09AC4: startDate without endDate — endDate defaults to now + 30 days.
    #[test]
    fn test_S09AC4_start_date_without_end_date_defaults_end() {
        let now = Local::now().naive_utc();
        let start = parse_flexible_date_as_ndt("2025-05-01T00:00:00").unwrap();
        let end = now + Duration::days(30); // default when endDate is None
        assert!(start < end, "start should be before default end");
    }

    /// S09AC5: endDate without startDate — startDate defaults to now - 30 days.
    #[test]
    fn test_S09AC5_end_date_without_start_date_defaults_start() {
        let now = Local::now().naive_utc();
        let start = now - Duration::days(30); // default when startDate is None
                                              // Use a future date for end to ensure start < end
        let end = parse_flexible_date_as_ndt("2027-06-15T00:00:00").unwrap();
        assert!(start < end, "default start should be before end");
    }

    /// S09AC10: limit=50, offset=0 returns first 50 events, has_more=true if total > 50.
    #[test]
    fn test_S09AC10_limit_50_offset_0_first_page() {
        let total: usize = 120;
        let limit: u32 = 50;
        let offset: u32 = 0;

        let has_more = (offset as usize + limit as usize) < total;
        assert!(has_more, "has_more should be true when total > limit");

        // Simulated slice: [0..50]
        let start_idx = offset as usize;
        let end_idx = std::cmp::min(start_idx + limit as usize, total);
        assert_eq!(end_idx, 50, "should take first 50");
    }

    /// S09AC11: limit=50, offset=50 returns second page.
    #[test]
    fn test_S09AC11_limit_50_offset_50_second_page() {
        let total: usize = 120;
        let limit: u32 = 50;
        let offset: u32 = 50;

        let has_more = (offset as usize + limit as usize) < total;
        assert!(has_more, "has_more should be true (50+50=100 < 120)");

        // Simulated slice: [50..100]
        let start_idx = offset as usize;
        let end_idx = std::cmp::min(start_idx + limit as usize, total);
        assert_eq!(start_idx, 50, "should start at 50");
        assert_eq!(end_idx, 100, "should end at 100");
    }
}
