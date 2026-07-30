//! The intentional `time` <-> `chrono` conversion boundary.
//!
//! The rest of the app uses the `time` crate. `rrule` (recurrence) and
//! `icalendar` (import/export, phase 4) are both `chrono`-based, so any
//! conversion between the two date/time libraries should happen only here.

use chrono::{DateTime, Utc};
use rrule::{RRule, RRuleSet, Tz, Unvalidated};
use time::OffsetDateTime;

use super::model::Event;

/// Maximum occurrences generated per query, guarding against unbounded
/// RRULEs (e.g. no COUNT/UNTIL) combined with a huge query range.
const MAX_OCCURRENCES: u16 = 512;

fn to_chrono(dt: OffsetDateTime) -> DateTime<Tz> {
    let utc = DateTime::<Utc>::from_timestamp(dt.unix_timestamp(), dt.nanosecond())
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).unwrap());
    utc.with_timezone(&Tz::UTC)
}

fn from_chrono(dt: DateTime<Tz>) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(dt.timestamp())
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
        + time::Duration::nanoseconds(i64::from(dt.timestamp_subsec_nanos()))
}

impl Event {
    /// Returns the occurrence start times of this event that fall within
    /// `range`. Non-recurring events yield at most their own start time;
    /// recurring events (`rrule.is_some()`) are expanded via the `rrule`
    /// crate, honoring `exdates`.
    pub fn occurrences(&self, range: std::ops::Range<OffsetDateTime>) -> Vec<OffsetDateTime> {
        let start = self.start();

        let Some(rrule_str) = self.rrule.as_deref() else {
            return if range.contains(&start) {
                vec![start]
            } else {
                Vec::new()
            };
        };

        let dt_start = to_chrono(start);
        let rule: RRule<Unvalidated> = match rrule_str.parse() {
            Ok(rule) => rule,
            Err(err) => {
                tracing::warn!("invalid RRULE {rrule_str:?} on event {:?}: {err}", self.id);
                return Vec::new();
            }
        };
        let rule = match rule.validate(dt_start) {
            Ok(rule) => rule,
            Err(err) => {
                tracing::warn!("invalid RRULE {rrule_str:?} on event {:?}: {err}", self.id);
                return Vec::new();
            }
        };

        let mut set = RRuleSet::new(dt_start).rrule(rule);
        for exdate in &self.exdates {
            set = set.exdate(to_chrono(*exdate));
        }
        set = set.after(to_chrono(range.start)).before(to_chrono(range.end));

        set.all(MAX_OCCURRENCES)
            .dates
            .into_iter()
            .map(from_chrono)
            .collect()
    }

    /// The start instant of this event, normalizing all-day events to
    /// midnight UTC.
    pub fn start(&self) -> OffsetDateTime {
        match &self.time {
            super::model::EventTime::Timed { start, .. } => *start,
            super::model::EventTime::AllDay { start, .. } => {
                start.midnight().assume_offset(time::UtcOffset::UTC)
            }
        }
    }
}
