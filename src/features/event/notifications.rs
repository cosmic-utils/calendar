//! Periodic reminder-due checking and desktop notifications.
//!
//! Driven by a `cosmic::iced::time::every` tick in `AppModel::subscription`
//! (see `Message::CheckReminders` in `src/app.rs`), which calls
//! [`check_reminders`] on a blocking thread and folds the returned
//! already-fired keys into `AppModel.fired_reminders` so the same reminder
//! doesn't notify twice.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use time::OffsetDateTime;

use crate::features::event::model::Event;
use crate::shared::ids::EventId;
use crate::shared::storage::LocalStorage;

/// How far ahead to look for occurrences when checking for due reminders.
/// Generous enough to cover any reasonable lead time set on a reminder.
const LOOKAHEAD_HOURS: i64 = 48;

/// A reminder is considered "due" once its trigger time falls within this
/// many minutes of now, so a reminder isn't missed between ticks but also
/// doesn't fire hours late after the app was closed and reopened.
const GRACE_MINUTES: i64 = 5;

/// Scans `storage` for reminders that just became due (and aren't already in
/// `already_fired`), fires a desktop notification for each, and returns
/// their `(EventId, occurrence_start)` keys to be recorded as fired.
pub fn check_reminders(
    storage: &Arc<Mutex<LocalStorage>>,
    already_fired: &HashSet<(EventId, OffsetDateTime)>,
) -> Vec<(EventId, OffsetDateTime)> {
    let now = OffsetDateTime::now_utc();
    let range = now..(now + time::Duration::hours(LOOKAHEAD_HOURS));

    let storage = storage.lock().unwrap();
    let mut newly_fired = Vec::new();

    for event in storage.events_in_range(range.clone()) {
        if event.reminders.is_empty() {
            continue;
        }

        for occurrence_start in event.occurrences(range.clone()) {
            let key = (event.id, occurrence_start);
            if already_fired.contains(&key) {
                continue;
            }

            let is_due = event.reminders.iter().any(|reminder| {
                let trigger = occurrence_start - time::Duration::minutes(reminder.minutes_before);
                trigger <= now && trigger > now - time::Duration::minutes(GRACE_MINUTES)
            });

            if is_due {
                notify(event, occurrence_start);
                newly_fired.push(key);
            }
        }
    }

    newly_fired
}

fn notify(event: &Event, occurrence_start: OffsetDateTime) {
    let body = format!(
        "{:02}:{:02} on {}",
        occurrence_start.hour(),
        occurrence_start.minute(),
        occurrence_start.date()
    );

    if let Err(err) = notify_rust::Notification::new()
        .appname("Calendar")
        .summary(&event.summary)
        .body(&body)
        .icon("office-calendar-symbolic")
        .show()
    {
        tracing::warn!("failed to show reminder notification: {err}");
    }
}
