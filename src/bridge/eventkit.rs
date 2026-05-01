//! EventKit FFI bridge through objc2/objc2-event-kit.
//!
//! This module provides safe Rust wrappers around macOS EventKit APIs.

use std::sync::mpsc;

use block2::StackBlock;
use chrono::TimeZone;
use objc2::runtime::Bool as ObjcBool;
use objc2_core_graphics::CGColor;
use objc2_event_kit::{
    EKAuthorizationStatus, EKCalendar, EKEntityType, EKEvent, EKEventStore, EKSpan, EKSource,
    EKSourceType,
};
use objc2_foundation::{NSDate, NSError, NSString, NSURL};
use thiserror::Error;

use crate::models::{Calendar, Event, EventCreateRequest, EventUpdateRequest};

// ---------------------------------------------------------------------------
// BridgeError (R6)
// ---------------------------------------------------------------------------

/// Errors produced by [`EventKitBridge`].
#[derive(Error, Debug)]
pub enum BridgeError {
    #[error("access to calendar was denied – grant permission in System Settings > Privacy & Security > Calendars")]
    AccessDenied,

    #[error("calendar not found: {0}")]
    CalendarNotFound(String),

    #[error("event not found: {0}")]
    EventNotFound(String),

    #[error("calendar does not allow modifications")]
    ModificationNotAllowed,

    #[error("no valid source found for creating a calendar")]
    NoValidSource,

    #[error("invalid date format: {0}")]
    InvalidDateFormat(String),

    #[error("EventKit error: {0}")]
    EventKitError(String),

    #[error("Objective-C error: {0}")]
    ObjcError(String),
}

// ---------------------------------------------------------------------------
// EventKitBridge
// ---------------------------------------------------------------------------

/// Safe Rust wrapper around macOS EventKit.
///
/// Holds a long-lived `EKEventStore` instance (the recommended pattern per
/// Apple documentation).
pub struct EventKitBridge {
    event_store: objc2::rc::Retained<EKEventStore>,
}

impl EventKitBridge {
    // -----------------------------------------------------------------------
    // R1: Initialisation
    // -----------------------------------------------------------------------

    /// Create a new `EventKitBridge` backed by a fresh `EKEventStore`.
    ///
    /// # Safety
    ///
    /// Must be called on the main thread or a thread with a running
    /// `CFRunLoop` (required by EventKit internals).
    pub fn new() -> Result<Self, BridgeError> {
        let event_store = unsafe { EKEventStore::new() };
        Ok(Self { event_store })
    }

    // -----------------------------------------------------------------------
    // R2: Access request
    // -----------------------------------------------------------------------

    /// Request calendar access. Returns `Ok(true)` when access is granted.
    ///
    /// If the status is `NotDetermined` the system permission dialog is shown.
    /// If the status is `Denied` or `Restricted` an error is returned.
    pub fn request_access(&self) -> Result<bool, BridgeError> {
        let status = unsafe {
            EKEventStore::authorizationStatusForEntityType(EKEntityType::Event)
        };

        match status {
            s if s == EKAuthorizationStatus::FullAccess
                || s == EKAuthorizationStatus::WriteOnly =>
            {
                Ok(true)
            }
            _ => {
                // NotDetermined, Denied, Restricted — request access synchronously via channel
                let (sender, receiver) = mpsc::channel();
                let block =
                    StackBlock::new(move |granted: ObjcBool, _error: *mut NSError| {
                        let _ = sender.send(granted.as_bool());
                    });
                let block = block.copy();
                unsafe {
                    #[allow(deprecated)]
                    self.event_store
                        .requestAccessToEntityType_completion(
                            EKEntityType::Event,
                            &*block as *const _ as *mut _,
                        );
                }
                receiver
                    .recv()
                    .map_err(|_| BridgeError::EventKitError("channel closed".into()))
            }
        }
    }

    // -----------------------------------------------------------------------
    // R3: Calendar operations
    // -----------------------------------------------------------------------

    /// Return all calendars that support events.
    pub fn list_calendars(&self) -> Result<Vec<Calendar>, BridgeError> {
        let ek_calendars = unsafe {
            self.event_store
                .calendarsForEntityType(EKEntityType::Event)
        };

        let default_cal = unsafe { self.event_store.defaultCalendarForNewEvents() };
        let default_id = default_cal.as_ref().map(|c| {
            unsafe { c.calendarIdentifier() }.to_string()
        });

        let mut result = Vec::new();
        for ek_cal in ek_calendars.iter() {
            result.push(ekcalendar_to_model(&ek_cal, default_id.as_deref()));
        }
        Ok(result)
    }

    /// Find a calendar by its identifier.
    pub fn find_calendar_by_id(&self, id: &str) -> Result<Option<Calendar>, BridgeError> {
        let ns_id = rust_to_nsstring(id);
        let ek_cal =
            unsafe { self.event_store.calendarWithIdentifier(&ns_id) };

        match ek_cal {
            Some(cal) => {
                let default_cal =
                    unsafe { self.event_store.defaultCalendarForNewEvents() };
                let default_id = default_cal.as_ref().map(|c| {
                    unsafe { c.calendarIdentifier() }.to_string()
                });
                Ok(Some(ekcalendar_to_model(&cal, default_id.as_deref())))
            }
            None => Ok(None),
        }
    }

    /// Create a new calendar with the given title and optional hex colour.
    pub fn create_calendar(
        &self,
        title: &str,
        color: Option<&str>,
    ) -> Result<Calendar, BridgeError> {
        let source = self.find_local_source()?;

        let ek_cal = unsafe {
            let cal =
                EKCalendar::calendarForEntityType_eventStore(EKEntityType::Event, &self.event_store);
            cal.setTitle(&rust_to_nsstring(title));
            cal.setSource(Some(&source));

            if let Some(hex) = color {
                let cg = hex_to_cgcolor(hex);
                if let Some(cg_color) = cg {
                    cal.setCGColor(Some(&cg_color));
                }
            }
            cal
        };

        unsafe {
            self.event_store
                .saveCalendar_commit_error(&ek_cal, true)
                .map_err(|e| BridgeError::EventKitError(nserror_to_string(&e)))?;
        }

        let default_cal = unsafe { self.event_store.defaultCalendarForNewEvents() };
        let default_id = default_cal.as_ref().map(|c| {
            unsafe { c.calendarIdentifier() }.to_string()
        });

        Ok(ekcalendar_to_model(&ek_cal, default_id.as_deref()))
    }

    /// Delete a calendar by its identifier.
    pub fn delete_calendar(&self, id: &str) -> Result<(), BridgeError> {
        let ns_id = rust_to_nsstring(id);
        let ek_cal =
            unsafe { self.event_store.calendarWithIdentifier(&ns_id) }
                .ok_or_else(|| BridgeError::CalendarNotFound(id.to_string()))?;

        if !unsafe { ek_cal.allowsContentModifications() } {
            return Err(BridgeError::ModificationNotAllowed);
        }

        unsafe {
            self.event_store
                .removeCalendar_commit_error(&ek_cal, true)
                .map_err(|e| BridgeError::EventKitError(nserror_to_string(&e)))?;
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // R4: Event operations
    // -----------------------------------------------------------------------

    /// List events in a calendar within the given date range (ISO-8601).
    pub fn list_events(
        &self,
        calendar_id: &str,
        start: &str,
        end: &str,
    ) -> Result<Vec<Event>, BridgeError> {
        let ns_cal_id = rust_to_nsstring(calendar_id);
        let ek_cal = unsafe {
            self.event_store.calendarWithIdentifier(&ns_cal_id)
        }
        .ok_or_else(|| BridgeError::CalendarNotFound(calendar_id.to_string()))?;

        let ns_start = iso8601_to_nsdate(start)?;
        let ns_end = iso8601_to_nsdate(end)?;

        let calendars = objc2_foundation::NSArray::from_retained_slice(&[ek_cal]);
        let predicate = unsafe {
            self.event_store
                .predicateForEventsWithStartDate_endDate_calendars(
                    &ns_start,
                    &ns_end,
                    Some(&calendars),
                )
        };

        let ek_events = unsafe { self.event_store.eventsMatchingPredicate(&predicate) };

        let mut result = Vec::new();
        for ek_event in ek_events.iter() {
            result.push(ekevent_to_model(&ek_event)?);
        }
        Ok(result)
    }

    /// Get a single event by its identifier.
    pub fn get_event(&self, event_id: &str) -> Result<Option<Event>, BridgeError> {
        let ns_id = rust_to_nsstring(event_id);
        let ek_event =
            unsafe { self.event_store.eventWithIdentifier(&ns_id) };

        match ek_event {
            Some(ev) => Ok(Some(ekevent_to_model(&ev)?)),
            None => Ok(None),
        }
    }

    /// Create a new event.
    pub fn create_event(
        &self,
        calendar_id: &str,
        request: &EventCreateRequest,
    ) -> Result<Event, BridgeError> {
        let ns_cal_id = rust_to_nsstring(calendar_id);
        let ek_cal = unsafe {
            self.event_store.calendarWithIdentifier(&ns_cal_id)
        }
        .ok_or_else(|| BridgeError::CalendarNotFound(calendar_id.to_string()))?;

        let start_date = iso8601_to_nsdate(&request.start_date)?;
        let end_date = iso8601_to_nsdate(&request.end_date)?;

        let ek_event = unsafe {
            let ev = EKEvent::eventWithEventStore(&self.event_store);
            ev.setTitle(Some(&rust_to_nsstring(&request.title)));
            ev.setCalendar(Some(&ek_cal));
            ev.setStartDate(Some(&start_date));
            ev.setEndDate(Some(&end_date));

            if let Some(ref loc) = request.location {
                ev.setLocation(Some(&rust_to_nsstring(loc)));
            }
            if let Some(ref notes) = request.notes {
                ev.setNotes(Some(&rust_to_nsstring(notes)));
            }
            if let Some(is_all_day) = request.is_all_day {
                ev.setAllDay(is_all_day);
            }
            if let Some(ref url) = request.url {
                if let Some(ns_url) = NSURL::URLWithString(&rust_to_nsstring(url)) {
                    ev.setURL(Some(&ns_url));
                }
            }
            ev
        };

        unsafe {
            self.event_store
                .saveEvent_span_error(&ek_event, EKSpan::ThisEvent)
                .map_err(|e| BridgeError::EventKitError(nserror_to_string(&e)))?;
        }

        ekevent_to_model(&ek_event)
    }

    /// Update an existing event. Only the fields present in `request` are changed.
    pub fn update_event(
        &self,
        event_id: &str,
        request: &EventUpdateRequest,
    ) -> Result<Event, BridgeError> {
        let ns_id = rust_to_nsstring(event_id);
        let ek_event =
            unsafe { self.event_store.eventWithIdentifier(&ns_id) }
                .ok_or_else(|| BridgeError::EventNotFound(event_id.to_string()))?;

        unsafe {
            if let Some(ref title) = request.title {
                ek_event.setTitle(Some(&rust_to_nsstring(title)));
            }
            if let Some(ref start) = request.start_date {
                let d = iso8601_to_nsdate(start)?;
                ek_event.setStartDate(Some(&d));
            }
            if let Some(ref end) = request.end_date {
                let d = iso8601_to_nsdate(end)?;
                ek_event.setEndDate(Some(&d));
            }
            if let Some(ref loc) = request.location {
                ek_event.setLocation(Some(&rust_to_nsstring(loc)));
            }
            if let Some(ref notes) = request.notes {
                ek_event.setNotes(Some(&rust_to_nsstring(notes)));
            }
            if let Some(is_all_day) = request.is_all_day {
                ek_event.setAllDay(is_all_day);
            }
            if let Some(ref url) = request.url {
                if let Some(ns_url) = NSURL::URLWithString(&rust_to_nsstring(url)) {
                    ek_event.setURL(Some(&ns_url));
                }
            }

            self.event_store
                .saveEvent_span_error(&ek_event, EKSpan::ThisEvent)
                .map_err(|e| BridgeError::EventKitError(nserror_to_string(&e)))?;
        }

        ekevent_to_model(&ek_event)
    }

    /// Delete an event by its identifier.
    pub fn delete_event(&self, event_id: &str) -> Result<(), BridgeError> {
        let ns_id = rust_to_nsstring(event_id);
        let ek_event =
            unsafe { self.event_store.eventWithIdentifier(&ns_id) }
                .ok_or_else(|| BridgeError::EventNotFound(event_id.to_string()))?;

        unsafe {
            self.event_store
                .removeEvent_span_error(&ek_event, EKSpan::ThisEvent)
                .map_err(|e| BridgeError::EventKitError(nserror_to_string(&e)))?;
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Find a suitable `EKSource` for creating a new calendar.
    ///
    /// Prefers `Local` source; falls back to the default calendar's source.
    fn find_local_source(
        &self,
    ) -> Result<objc2::rc::Retained<EKSource>, BridgeError> {
        let sources = unsafe { self.event_store.sources() };
        for src in sources.iter() {
            if unsafe { src.sourceType() } == EKSourceType::Local {
                return Ok(src);
            }
        }

        // Fall back to default calendar's source
        if let Some(default_cal) = unsafe { self.event_store.defaultCalendarForNewEvents() } {
            if let Some(source) = unsafe { default_cal.source() } {
                return Ok(source);
            }
        }

        Err(BridgeError::NoValidSource)
    }
}

// ---------------------------------------------------------------------------
// R5: Type conversion helpers
// ---------------------------------------------------------------------------

/// Create an `NSString` from a Rust `&str`.
fn rust_to_nsstring(s: &str) -> objc2::rc::Retained<NSString> {
    NSString::from_str(s)
}

/// Convert an `NSError` to a human-readable String.
fn nserror_to_string(err: &NSError) -> String {
    format!("{:?}", err)
}

/// Convert an `EKCalendar` to our Rust [`Calendar`] model.
fn ekcalendar_to_model(
    ek_cal: &EKCalendar,
    default_calendar_id: Option<&str>,
) -> Calendar {
    let id = unsafe { ek_cal.calendarIdentifier() }.to_string();
    let title = unsafe { ek_cal.title() }.to_string();
    let allows_modifications = unsafe { ek_cal.allowsContentModifications() };
    let is_default = default_calendar_id.map_or(false, |did| did == id);

    let color = unsafe { ek_cal.CGColor() }
        .map(|cg| cgcolor_to_hex(&cg))
        .unwrap_or_else(|| "#000000".to_string());

    Calendar {
        id,
        title,
        color,
        is_default,
        allows_modifications,
    }
}

/// Convert an `EKEvent` to our Rust [`Event`] model.
fn ekevent_to_model(ek_event: &EKEvent) -> Result<Event, BridgeError> {
    let id = unsafe { ek_event.eventIdentifier() }
        .map(|s| s.to_string())
        .unwrap_or_default();

    let title = unsafe { ek_event.title() }.to_string();

    let start_date = unsafe { ek_event.startDate() };
    let start_date = nsdate_to_iso8601(&start_date);

    let end_date = unsafe { ek_event.endDate() };
    let end_date = nsdate_to_iso8601(&end_date);

    let is_all_day = unsafe { ek_event.isAllDay() };

    let location = unsafe { ek_event.location() }.map(|s| s.to_string());
    let notes = unsafe { ek_event.notes() }.map(|s| s.to_string());
    let url = unsafe { ek_event.URL() }
        .and_then(|u| unsafe { u.absoluteString() })
        .map(|s| s.to_string());

    let calendar_id = unsafe { ek_event.calendar() }
        .map(|c| unsafe { c.calendarIdentifier() }.to_string())
        .unwrap_or_default();

    Ok(Event {
        id,
        title,
        calendar_id,
        start_date,
        end_date,
        location,
        notes,
        url,
        is_all_day,
    })
}

// ---------------------------------------------------------------------------
// R5: CGColor <-> hex
// ---------------------------------------------------------------------------

/// Convert a `CGColor` to a `#RRGGBB` hex string.
pub fn cgcolor_to_hex(color: &CGColor) -> String {
    let num = CGColor::number_of_components(Some(color));
    let components = CGColor::components(Some(color));

    if components.is_null() || num < 3 {
        return "#000000".to_string();
    }

    let r = unsafe { *components.add(0) };
    let g = unsafe { *components.add(1) };
    let b = unsafe { *components.add(2) };

    let ri = (r.clamp(0.0, 1.0) * 255.0).round() as u8;
    let gi = (g.clamp(0.0, 1.0) * 255.0).round() as u8;
    let bi = (b.clamp(0.0, 1.0) * 255.0).round() as u8;

    format!("#{:02X}{:02X}{:02X}", ri, gi, bi)
}

/// Parse a `#RRGGBB` hex string and create a `CGColor`.
///
/// Returns `None` if the string cannot be parsed.
pub fn hex_to_cgcolor(hex: &str) -> Option<objc2::rc::Retained<CGColor>> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f64 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f64 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f64 / 255.0;

    Some(CGColor::new_generic_rgb(r, g, b, 1.0).into())
}

// ---------------------------------------------------------------------------
// R5: NSDate <-> ISO-8601
// ---------------------------------------------------------------------------

/// Convert an `NSDate` to an ISO-8601 string (`2025-03-09T10:00:00.000Z`).
pub fn nsdate_to_iso8601(date: &NSDate) -> String {
    let ts = date.timeIntervalSince1970();
    let millis = (ts * 1000.0).round() as i64;
    let dt: chrono::DateTime<chrono::Utc> =
        chrono::Utc.timestamp_millis_opt(millis).single().unwrap_or_default();
    dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// Parse an ISO-8601 date string into an `NSDate`.
///
/// Supported formats:
/// - `2025-03-09T10:00:00.000Z`
/// - `2025-03-09T10:00:00`
/// - `2025-03-09 10:00:00`
pub fn iso8601_to_nsdate(
    s: &str,
) -> Result<objc2::rc::Retained<NSDate>, BridgeError> {
    let ts = parse_iso8601_timestamp(s)?;
    Ok(NSDate::dateWithTimeIntervalSince1970(ts))
}

/// Parse an ISO-8601 string and return seconds since 1970-01-01.
fn parse_iso8601_timestamp(s: &str) -> Result<f64, BridgeError> {
    // Try standard ISO-8601 formats
    let formats = [
        "%Y-%m-%dT%H:%M:%S%.3fZ", // 2025-03-09T10:00:00.000Z
        "%Y-%m-%dT%H:%M:%S%.fZ",  // 2025-03-09T10:00:00Z
        "%Y-%m-%dT%H:%M:%S",      // 2025-03-09T10:00:00
        "%Y-%m-%d %H:%M:%S",      // 2025-03-09 10:00:00
    ];

    for fmt in &formats {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
            let utc = dt.and_utc();
            return Ok(utc.timestamp() as f64 + utc.timestamp_subsec_millis() as f64 / 1000.0);
        }
    }

    // Try date-only
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let dt = d.and_hms_opt(0, 0, 0).unwrap().and_utc();
        return Ok(dt.timestamp() as f64);
    }

    Err(BridgeError::InvalidDateFormat(s.to_string()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(non_snake_case)]

    use super::*;

    // ------------------------------------------------------------------
    // S02AC10: BridgeError variants and Display
    // ------------------------------------------------------------------

    #[test]
    fn test_S02AC10_bridge_error_access_denied() {
        let err = BridgeError::AccessDenied;
        let msg = format!("{}", err);
        assert!(
            msg.to_lowercase().contains("denied"),
            "AccessDenied message should mention 'denied': got '{}'",
            msg
        );
    }

    #[test]
    fn test_S02AC10_bridge_error_calendar_not_found() {
        let err = BridgeError::CalendarNotFound("cal-123".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("cal-123"));
    }

    #[test]
    fn test_S02AC10_bridge_error_event_not_found() {
        let err = BridgeError::EventNotFound("evt-456".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("evt-456"));
    }

    #[test]
    fn test_S02AC10_bridge_error_modification_not_allowed() {
        let err = BridgeError::ModificationNotAllowed;
        let msg = format!("{}", err);
        assert!(
            msg.to_lowercase().contains("modif"),
            "ModificationNotAllowed message should mention 'modif': got '{}'",
            msg
        );
    }

    #[test]
    fn test_S02AC10_bridge_error_no_valid_source() {
        let err = BridgeError::NoValidSource;
        let msg = format!("{}", err);
        assert!(
            msg.to_lowercase().contains("source"),
            "NoValidSource message should mention 'source': got '{}'",
            msg
        );
    }

    #[test]
    fn test_S02AC10_bridge_error_invalid_date_format() {
        let err = BridgeError::InvalidDateFormat("bad-date".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("bad-date"));
    }

    #[test]
    fn test_S02AC10_bridge_error_eventkit_error() {
        let err = BridgeError::EventKitError("something broke".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("something broke"));
    }

    #[test]
    fn test_S02AC10_bridge_error_objc_error() {
        let err = BridgeError::ObjcError("objc fail".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("objc fail"));
    }

    #[test]
    fn test_S02AC10_bridge_error_implements_std_error() {
        fn assert_error<E: std::error::Error>(_: &E) {}
        assert_error(&BridgeError::AccessDenied);
        assert_error(&BridgeError::CalendarNotFound("x".into()));
        assert_error(&BridgeError::EventNotFound("x".into()));
        assert_error(&BridgeError::ModificationNotAllowed);
        assert_error(&BridgeError::NoValidSource);
        assert_error(&BridgeError::InvalidDateFormat("x".into()));
        assert_error(&BridgeError::EventKitError("x".into()));
        assert_error(&BridgeError::ObjcError("x".into()));
    }

    // ------------------------------------------------------------------
    // S02AC11: CGColor <-> hex conversion
    // ------------------------------------------------------------------

    #[test]
    fn test_S02AC11_hex_to_cgcolor_valid() {
        let result = hex_to_cgcolor("#FF0000");
        assert!(result.is_some(), "valid hex should produce a CGColor");

        let result = hex_to_cgcolor("#00FF00");
        assert!(result.is_some());

        let result = hex_to_cgcolor("#0000FF");
        assert!(result.is_some());
    }

    #[test]
    fn test_S02AC11_hex_to_cgcolor_without_hash() {
        let result = hex_to_cgcolor("FF0000");
        assert!(result.is_some());
    }

    #[test]
    fn test_S02AC11_hex_to_cgcolor_invalid() {
        assert!(hex_to_cgcolor("#FFF").is_none(), "too short");
        assert!(hex_to_cgcolor("#GGGGGG").is_none(), "invalid chars");
        assert!(hex_to_cgcolor("").is_none(), "empty string");
        assert!(hex_to_cgcolor("#12345").is_none(), "5 chars");
    }

    #[test]
    fn test_S02AC11_cgcolor_roundtrip() {
        let cg = hex_to_cgcolor("#AABBCC").unwrap();
        let hex = cgcolor_to_hex(&cg);
        assert_eq!(hex, "#AABBCC", "round-trip should preserve color");
    }

    // ------------------------------------------------------------------
    // R5: Date parsing
    // ------------------------------------------------------------------

    #[test]
    fn test_date_parse_iso8601_with_millis() {
        let ts = parse_iso8601_timestamp("2025-03-09T10:00:00.000Z").unwrap();
        let expected = 1741514400.0;
        assert!(
            (ts - expected).abs() < 1.0,
            "expected ~{}, got {}",
            expected,
            ts
        );
    }

    #[test]
    fn test_date_parse_iso8601_without_millis() {
        let ts = parse_iso8601_timestamp("2025-03-09T10:00:00").unwrap();
        let expected = 1741514400.0;
        assert!(
            (ts - expected).abs() < 1.0,
            "expected ~{}, got {}",
            expected,
            ts
        );
    }

    #[test]
    fn test_date_parse_space_separated() {
        let ts = parse_iso8601_timestamp("2025-03-09 10:00:00").unwrap();
        let expected = 1741514400.0;
        assert!(
            (ts - expected).abs() < 1.0,
            "expected ~{}, got {}",
            expected,
            ts
        );
    }

    #[test]
    fn test_date_parse_invalid() {
        let result = parse_iso8601_timestamp("not-a-date");
        assert!(result.is_err());
    }

    #[test]
    fn test_nsdate_roundtrip() {
        let original_ts = 1741514400.0; // 2025-03-09T10:00:00Z
        let date = NSDate::dateWithTimeIntervalSince1970(original_ts);
        let iso = nsdate_to_iso8601(&date);
        assert!(
            iso.starts_with("2025-03-09"),
            "ISO string should start with 2025-03-09: got {}",
            iso
        );

        let parsed_ts = parse_iso8601_timestamp(&iso).unwrap();
        assert!(
            (parsed_ts - original_ts).abs() < 1.0,
            "round-trip mismatch: original={}, parsed={}",
            original_ts,
            parsed_ts
        );
    }
}
