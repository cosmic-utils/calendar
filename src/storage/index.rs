use std::collections::{BTreeMap, HashMap};
use std::ops::Range;

use time::OffsetDateTime;

use crate::models::{CalendarKey, Event, EventId};

/// In-memory index over all loaded [`Event`]s, rebuilt (or loaded from a
/// disposable cache) at startup so date-range queries don't need to scan
/// every JSON file on disk each time.
#[derive(Default)]
pub struct EventIndex {
    by_id: HashMap<EventId, Event>,
    by_calendar: BTreeMap<CalendarKey, Vec<EventId>>,
    by_start: BTreeMap<OffsetDateTime, Vec<EventId>>,
}

impl EventIndex {
    pub fn upsert(&mut self, event: Event) {
        self.remove(event.id);

        let calendar = event.calendar.clone();
        let start = event_sort_key(&event);
        let id = event.id;

        self.by_id.insert(id, event);
        self.by_calendar.entry(calendar).or_default().push(id);
        self.by_start.entry(start).or_default().push(id);
    }

    pub fn remove(&mut self, id: EventId) {
        let Some(existing) = self.by_id.remove(&id) else {
            return;
        };

        if let Some(ids) = self.by_calendar.get_mut(&existing.calendar) {
            ids.retain(|other| *other != id);
        }
        let start = event_sort_key(&existing);
        if let Some(ids) = self.by_start.get_mut(&start) {
            ids.retain(|other| *other != id);
        }
    }

    pub fn get(&self, id: EventId) -> Option<&Event> {
        self.by_id.get(&id)
    }

    pub fn events_for_calendar(&self, calendar: &CalendarKey) -> Vec<&Event> {
        self.by_calendar
            .get(calendar)
            .into_iter()
            .flatten()
            .filter_map(|id| self.by_id.get(id))
            .collect()
    }

    /// Returns every event that has at least one occurrence within `range`.
    ///
    /// Non-recurring events are checked directly; recurring events
    /// (`rrule.is_some()`) are expanded on demand via [`Event::occurrences`].
    pub fn events_in_range(&self, range: Range<OffsetDateTime>) -> Vec<&Event> {
        self.by_id
            .values()
            .filter(|event| !event.occurrences(range.clone()).is_empty())
            .collect()
    }

    pub fn all(&self) -> impl Iterator<Item = &Event> {
        self.by_id.values()
    }

    /// Case-insensitive substring search over `summary`/`description`/
    /// `location`, sorted by start time. No search index is built given the
    /// expected data volumes (a personal calendar, hundreds-to-low-thousands
    /// of events).
    pub fn search(&self, query: &str) -> Vec<&Event> {
        let query = query.to_lowercase();
        if query.is_empty() {
            return Vec::new();
        }

        let mut results: Vec<&Event> = self
            .by_id
            .values()
            .filter(|event| {
                event.summary.to_lowercase().contains(&query)
                    || event
                        .description
                        .as_ref()
                        .is_some_and(|text| text.to_lowercase().contains(&query))
                    || event
                        .location
                        .as_ref()
                        .is_some_and(|text| text.to_lowercase().contains(&query))
            })
            .collect();
        results.sort_by_key(|event| event_sort_key(event));
        results
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

fn event_sort_key(event: &Event) -> OffsetDateTime {
    match &event.time {
        crate::models::EventTime::Timed { start, .. } => *start,
        crate::models::EventTime::AllDay { start, .. } => {
            start.midnight().assume_offset(time::UtcOffset::UTC)
        }
    }
}
