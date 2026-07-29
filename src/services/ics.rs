//! iCalendar (`.ics`) import/export.
//!
//! Enabling `icalendar`'s `time` feature lets us hand `time::Date` /
//! `time::OffsetDateTime` straight to its builder API, so the only manual
//! `chrono` <-> `time` conversion needed here is on the *read* path
//! (`icalendar` only exposes parsed date-times as `chrono` types).

use chrono::{DateTime, NaiveDate, Utc};
use icalendar::{
    Calendar as IcsCalendar, CalendarComponent, CalendarDateTime, Component, DatePerhapsTime,
    Event as IcsEvent, EventLike, Todo as IcsTodo,
};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::models::{CalendarKey, Event, EventId, EventKind, EventTime, Source, TaskDetails};
use crate::Result;

fn from_utc(dt: DateTime<Utc>) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(dt.timestamp())
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
        + time::Duration::nanoseconds(i64::from(dt.timestamp_subsec_nanos()))
}

fn to_utc(dt: OffsetDateTime) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(dt.unix_timestamp(), dt.nanosecond())
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).unwrap())
}

fn from_naive_date(date: NaiveDate) -> time::Date {
    use chrono::Datelike;
    time::Month::try_from(date.month() as u8)
        .ok()
        .and_then(|month| time::Date::from_calendar_date(date.year(), month, date.day() as u8).ok())
        .unwrap_or(time::Date::MIN)
}

/// Treats floating and zone-qualified date-times as UTC: this app doesn't
/// model `VTIMEZONE` components, so a full round trip through a real
/// timezone isn't attempted here.
fn calendar_date_time_to_offset(dt: &CalendarDateTime) -> OffsetDateTime {
    let utc = match dt {
        CalendarDateTime::Utc(dt) => *dt,
        CalendarDateTime::Floating(dt) => DateTime::from_naive_utc_and_offset(*dt, Utc),
        CalendarDateTime::WithTimezone { date_time, .. } => {
            DateTime::from_naive_utc_and_offset(*date_time, Utc)
        }
    };
    from_utc(utc)
}

fn date_perhaps_time_to_event_time(dt: DatePerhapsTime) -> OffsetDateTime {
    match dt {
        DatePerhapsTime::DateTime(dt) => calendar_date_time_to_offset(&dt),
        DatePerhapsTime::Date(date) => from_naive_date(date)
            .midnight()
            .assume_offset(time::UtcOffset::UTC),
    }
}

fn is_all_day(dt: &DatePerhapsTime) -> bool {
    matches!(dt, DatePerhapsTime::Date(_))
}

/// Builds an in-memory `.ics` document from `events`.
pub fn export_events(events: &[Event]) -> IcsCalendar {
    let mut calendar = IcsCalendar::new();
    for event in events {
        match &event.kind {
            EventKind::Event => {
                let mut ics_event = IcsEvent::new();
                apply_common(&mut ics_event, event);
                match &event.time {
                    EventTime::Timed { start, end } => {
                        ics_event.starts(*start);
                        ics_event.ends(*end);
                    }
                    EventTime::AllDay { start, end } => {
                        ics_event.starts(*start);
                        ics_event.ends(*end);
                    }
                }
                if let Some(rrule) = &event.rrule {
                    ics_event.add_property("RRULE", rrule);
                }
                for exdate in &event.exdates {
                    ics_event.append_multi_property(
                        DatePerhapsTime::from(*exdate).to_property("EXDATE"),
                    );
                }
                calendar.push(ics_event.done());
            }
            EventKind::Task(details) => {
                let mut ics_todo = IcsTodo::new();
                apply_common(&mut ics_todo, event);
                if let Some(due) = details.due {
                    ics_todo.due(due);
                }
                if let Some(completed) = details.completed_at {
                    ics_todo.completed(to_utc(completed));
                }
                calendar.push(ics_todo.done());
            }
        }
    }
    calendar.done()
}

fn apply_common(component: &mut impl EventLike, event: &Event) {
    component.uid(&event.id.0.to_string());
    component.summary(&event.summary);
    if let Some(description) = &event.description {
        component.description(description);
    }
    if let Some(location) = &event.location {
        component.location(location);
    }
    component.sequence(event.sequence);
}

/// Parses `text` as an iCalendar document and converts its `VEVENT`/`VTODO`
/// components into [`Event`]s attached to `target`.
pub fn import_ics(text: &str, target: CalendarKey) -> Result<Vec<Event>> {
    let calendar: IcsCalendar = text
        .parse()
        .map_err(|err: String| crate::Error::Storage(format!("invalid ICS file: {err}")))?;

    let now = OffsetDateTime::now_utc();
    let mut events = Vec::new();

    for component in &calendar.components {
        match component {
            CalendarComponent::Event(ics_event) => {
                let Some(start) = ics_event.get_start() else {
                    continue;
                };
                let all_day = is_all_day(&start);
                let start = date_perhaps_time_to_event_time(start);
                let end = ics_event
                    .get_end()
                    .map(date_perhaps_time_to_event_time)
                    .unwrap_or(start + time::Duration::hours(1));

                let time = if all_day {
                    EventTime::AllDay {
                        start: start.date(),
                        end: end.date(),
                    }
                } else {
                    EventTime::Timed { start, end }
                };

                events.push(Event {
                    id: EventId(
                        ics_event
                            .get_uid()
                            .and_then(|uid| Uuid::parse_str(uid).ok())
                            .unwrap_or_else(Uuid::new_v4),
                    ),
                    calendar: target.clone(),
                    kind: EventKind::Event,
                    summary: ics_event.get_summary().unwrap_or("Untitled").to_string(),
                    description: ics_event.get_description().map(str::to_string),
                    location: ics_event.get_location().map(str::to_string),
                    time,
                    timezone: None,
                    color: None,
                    rrule: ics_event.property_value("RRULE").map(str::to_string),
                    exdates: ics_event
                        .multi_properties()
                        .get("EXDATE")
                        .into_iter()
                        .flatten()
                        .filter_map(|property| DatePerhapsTime::from_property(property))
                        .map(date_perhaps_time_to_event_time)
                        .collect(),
                    recurrence_id: ics_event
                        .get_recurrence_id()
                        .map(date_perhaps_time_to_event_time),
                    attendees: Vec::new(),
                    reminders: Vec::new(),
                    source: Source::Local,
                    external: None,
                    created_at: now,
                    updated_at: now,
                    sequence: 0,
                });
            }
            CalendarComponent::Todo(ics_todo) => {
                let start = ics_todo
                    .get_start()
                    .map(date_perhaps_time_to_event_time)
                    .unwrap_or(now);
                let due = ics_todo.get_due().map(date_perhaps_time_to_event_time);
                let end = due.unwrap_or(start + time::Duration::hours(1));

                events.push(Event {
                    id: EventId(
                        ics_todo
                            .get_uid()
                            .and_then(|uid| Uuid::parse_str(uid).ok())
                            .unwrap_or_else(Uuid::new_v4),
                    ),
                    calendar: target.clone(),
                    kind: EventKind::Task(TaskDetails {
                        due,
                        completed_at: ics_todo.get_completed().map(from_utc),
                        priority: ics_todo.get_priority().map(|p| p as u8),
                    }),
                    summary: ics_todo.get_summary().unwrap_or("Untitled").to_string(),
                    description: ics_todo.get_description().map(str::to_string),
                    location: ics_todo.get_location().map(str::to_string),
                    time: EventTime::Timed { start, end },
                    timezone: None,
                    color: None,
                    rrule: None,
                    exdates: Vec::new(),
                    recurrence_id: None,
                    attendees: Vec::new(),
                    reminders: Vec::new(),
                    source: Source::Local,
                    external: None,
                    created_at: now,
                    updated_at: now,
                    sequence: 0,
                });
            }
            _ => {}
        }
    }

    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CalendarId;

    #[test]
    fn round_trips_a_timed_event() {
        let calendar = CalendarKey::Local(CalendarId::new());
        let now = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap();
        let original = Event {
            id: EventId::new(),
            calendar: calendar.clone(),
            kind: EventKind::Event,
            summary: "Team sync".to_string(),
            description: Some("Weekly check-in".to_string()),
            location: Some("Room 4".to_string()),
            time: EventTime::Timed {
                start: now,
                end: now + time::Duration::hours(1),
            },
            timezone: None,
            color: None,
            rrule: Some("FREQ=WEEKLY".to_string()),
            exdates: vec![now + time::Duration::days(7)],
            recurrence_id: None,
            attendees: Vec::new(),
            reminders: Vec::new(),
            source: Source::Local,
            external: None,
            created_at: now,
            updated_at: now,
            sequence: 0,
        };

        let ics_text = export_events(std::slice::from_ref(&original)).to_string();
        let imported = import_ics(&ics_text, calendar).expect("import should succeed");

        assert_eq!(imported.len(), 1);
        let round_tripped = &imported[0];
        assert_eq!(round_tripped.id, original.id);
        assert_eq!(round_tripped.summary, original.summary);
        assert_eq!(round_tripped.description, original.description);
        assert_eq!(round_tripped.location, original.location);
        assert_eq!(round_tripped.rrule, original.rrule);
        assert_eq!(round_tripped.exdates.len(), 1);

        match round_tripped.time {
            EventTime::Timed { start, end } => {
                assert_eq!(start.unix_timestamp(), original.start().unix_timestamp());
                assert_eq!(
                    end.unix_timestamp(),
                    match original.time {
                        EventTime::Timed { end, .. } => end.unix_timestamp(),
                        _ => unreachable!(),
                    }
                );
            }
            _ => panic!("expected a timed event"),
        }
    }

    #[test]
    fn round_trips_an_all_day_event() {
        let calendar = CalendarKey::Local(CalendarId::new());
        let now = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap();
        let original = Event {
            id: EventId::new(),
            calendar: calendar.clone(),
            kind: EventKind::Event,
            summary: "Birthday".to_string(),
            description: None,
            location: None,
            time: EventTime::AllDay {
                start: now.date(),
                end: now.date().next_day().unwrap(),
            },
            timezone: None,
            color: None,
            rrule: None,
            exdates: Vec::new(),
            recurrence_id: None,
            attendees: Vec::new(),
            reminders: Vec::new(),
            source: Source::Local,
            external: None,
            created_at: now,
            updated_at: now,
            sequence: 0,
        };

        let ics_text = export_events(std::slice::from_ref(&original)).to_string();
        let imported = import_ics(&ics_text, calendar).expect("import should succeed");

        assert_eq!(imported.len(), 1);
        match imported[0].time {
            EventTime::AllDay { start, end } => {
                assert_eq!(start, original.start().date());
                assert_eq!(
                    end,
                    match original.time {
                        EventTime::AllDay { end, .. } => end,
                        _ => unreachable!(),
                    }
                );
            }
            _ => panic!("expected an all-day event"),
        }
    }
}
