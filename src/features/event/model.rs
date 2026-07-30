use accounts::models::Provider;
use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime};
use uuid::Uuid;

use crate::features::calendar_management::model::{CalendarKey, Source};
use crate::shared::ids::EventId;

/// The current on-disk shape of an [`Event`]. Bump when the struct's fields
/// change shape and add a matching arm in `src/storage/migration.rs`.
pub const CURRENT_EVENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventTime {
    /// `end` is exclusive, matching RFC 5545 `DTEND` semantics for all-day events.
    AllDay { start: Date, end: Date },
    Timed {
        start: OffsetDateTime,
        end: OffsetDateTime,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum AttendeeStatus {
    #[default]
    NeedsAction,
    Accepted,
    Declined,
    Tentative,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attendee {
    pub name: Option<String>,
    pub email: String,
    pub response: AttendeeStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reminder {
    /// Minutes before the event start that this reminder should fire.
    ///
    /// Stored as a plain integer rather than `time::Duration` to avoid a
    /// custom serde implementation.
    pub minutes_before: i64,
}

/// Identity of an [`Event`] on a remote provider, kept for a future sync
/// layer. Reconciliation should match by this first, then by UID, then
/// fall back to creating a new local event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalRef {
    pub account_id: Uuid,
    pub provider: Provider,
    /// The provider's calendar id (matches `Calendar::id`).
    pub calendar_id: String,
    pub provider_event_id: String,
    /// Used for future conflict detection against the provider's copy.
    pub etag: Option<String>,
    pub last_synced: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDetails {
    pub due: Option<OffsetDateTime>,
    pub completed_at: Option<OffsetDateTime>,
    /// RFC 5545 VTODO PRIORITY: 1 (highest) - 9 (lowest), 0/None = undefined.
    pub priority: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventKind {
    Event,
    Task(TaskDetails),
}

/// A single event or task, either created locally or (in the future) synced
/// from a provider account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: EventId,
    pub calendar: CalendarKey,
    pub kind: EventKind,
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub time: EventTime,
    /// IANA timezone name; `None` means floating/local time.
    pub timezone: Option<String>,
    /// Per-event override of the calendar's color.
    pub color: Option<String>,
    /// RFC 5545 RRULE string, expanded on demand via `crate::features::event::occurrences`.
    pub rrule: Option<String>,
    /// RFC 5545 EXDATE: occurrences of `rrule` to exclude.
    pub exdates: Vec<OffsetDateTime>,
    /// Set when this `Event` is a modified single occurrence of a recurring
    /// parent (RFC 5545 RECURRENCE-ID).
    pub recurrence_id: Option<OffsetDateTime>,
    pub attendees: Vec<Attendee>,
    pub reminders: Vec<Reminder>,
    pub source: Source,
    pub external: Option<ExternalRef>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    /// RFC 5545 SEQUENCE, bumped on every edit. Needed for ICS export and
    /// future sync conflict detection.
    pub sequence: u32,
}
