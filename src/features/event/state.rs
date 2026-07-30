use super::recurrence::RecurrenceRule;
use crate::fl;
use crate::features::calendar_management::model::{CalendarKey, Source};
use crate::features::event::model::{Event, EventKind, EventTime, Reminder, TaskDetails};
use crate::shared::ids::EventId;
use cosmic::widget;
use time::OffsetDateTime;

/// The year range offered by the event editor's start-date picker.
pub const EVENT_EDITOR_YEARS: [i32; 100] = {
    let mut years = [0; 100];
    let mut i = 0;
    while i < 100 {
        years[i] = 2000 + i as i32;
        i += 1;
    }
    years
};

/// The minute options offered by the event editor's time pickers.
pub const EVENT_EDITOR_MINUTES: [u8; 4] = [0, 15, 30, 45];

pub fn event_editor_months() -> [time::Month; 12] {
    [
        time::Month::January,
        time::Month::February,
        time::Month::March,
        time::Month::April,
        time::Month::May,
        time::Month::June,
        time::Month::July,
        time::Month::August,
        time::Month::September,
        time::Month::October,
        time::Month::November,
        time::Month::December,
    ]
}

/// Moves `state.start` to `new_start`, keeping the event's duration
/// constant by shifting `state.end` by the same delta.
pub fn shift_event_editor_start(state: &mut EventEditorState, new_start: OffsetDateTime) {
    let duration = state.end - state.start;
    state.start = new_start;
    state.end = new_start + duration;
}

/// State backing the event creation/editing form shown in the context drawer.
#[derive(Debug, Clone)]
pub struct EventEditorState {
    /// `None` while creating a new event; `Some` while editing an existing one.
    pub event_id: Option<EventId>,
    pub calendar: CalendarKey,
    pub summary: String,
    pub description: String,
    pub location: String,
    pub all_day: bool,
    pub is_task: bool,
    pub start: OffsetDateTime,
    pub end: OffsetDateTime,
    pub recurrence: RecurrenceRule,
    /// Minutes-before-start lead times for each reminder attached to this
    /// event.
    pub reminders: Vec<i64>,
    /// Current text in the "custom reminder" minutes input, offered
    /// alongside the fixed presets since those don't cover every lead time
    /// someone might want.
    pub custom_reminder_input: String,
    /// Whether the custom reminder field (a `widget::editable_input`) is
    /// currently showing its text box rather than its read-only label.
    pub custom_reminder_editing: bool,
    /// The specific occurrence that was clicked to open this editor (equal
    /// to `start` for non-recurring events and new events), used to record
    /// which single occurrence to exclude via EXDATE on "delete this
    /// occurrence".
    pub occurrence_start: OffsetDateTime,
}

/// The lead-time presets offered when adding a reminder in the event editor.
pub const REMINDER_PRESETS_MINUTES: [i64; 6] = [0, 5, 10, 15, 30, 60];

/// The label shown on a reminder preset's segmented-button entry.
pub fn reminder_label(minutes: i64) -> String {
    if minutes == 0 {
        fl!("reminder-at-start")
    } else if minutes % 60 == 0 {
        format!("{}h", minutes / 60)
    } else {
        format!("{minutes}m")
    }
}

/// Builds the reminders multi-select model: one entry per preset, plus one
/// for each of `reminders`'s lead times that isn't already covered by a
/// preset (added via the custom-minutes input). All are activated, since a
/// non-preset entry only exists in the model because it's in `reminders`;
/// deactivating one removes it from `reminders` entirely (there's no default
/// state for it to fall back to) and it disappears next rebuild.
pub fn build_reminders_model(
    reminders: &[i64],
) -> widget::segmented_button::Model<widget::segmented_button::MultiSelect> {
    let mut minutes_list: Vec<i64> = REMINDER_PRESETS_MINUTES.to_vec();
    for minutes in reminders {
        if !minutes_list.contains(minutes) {
            minutes_list.push(*minutes);
        }
    }
    minutes_list.sort_unstable();

    let mut builder =
        widget::segmented_button::Model::<widget::segmented_button::MultiSelect>::builder();
    for minutes in minutes_list {
        let is_on = reminders.contains(&minutes);
        builder = builder.insert(|b| {
            let b = b.text(reminder_label(minutes)).data(minutes);
            if is_on {
                b.activate()
            } else {
                b
            }
        });
    }
    builder.build()
}

impl EventEditorState {
    pub fn new(calendar: CalendarKey, start: OffsetDateTime) -> Self {
        Self {
            event_id: None,
            calendar,
            summary: String::new(),
            description: String::new(),
            location: String::new(),
            all_day: false,
            is_task: false,
            start,
            end: start + time::Duration::hours(1),
            recurrence: RecurrenceRule::NONE,
            reminders: Vec::new(),
            custom_reminder_input: String::new(),
            custom_reminder_editing: false,
            occurrence_start: start,
        }
    }

    pub fn from_event(event: &Event, occurrence_start: OffsetDateTime) -> Self {
        let (start, end, all_day) = match &event.time {
            EventTime::Timed { start, end } => (*start, *end, false),
            EventTime::AllDay { start, end } => (
                start.midnight().assume_offset(time::UtcOffset::UTC),
                end.midnight().assume_offset(time::UtcOffset::UTC),
                true,
            ),
        };

        Self {
            event_id: Some(event.id),
            calendar: event.calendar.clone(),
            summary: event.summary.clone(),
            description: event.description.clone().unwrap_or_default(),
            location: event.location.clone().unwrap_or_default(),
            all_day,
            is_task: matches!(event.kind, EventKind::Task(_)),
            start,
            end,
            recurrence: RecurrenceRule::from_rrule_string(event.rrule.as_deref()),
            reminders: event
                .reminders
                .iter()
                .map(|reminder| reminder.minutes_before)
                .collect(),
            custom_reminder_input: String::new(),
            custom_reminder_editing: false,
            occurrence_start,
        }
    }

    /// Builds the [`Event`] to persist from the current form state, reusing
    /// identity/timestamps/exceptions from `existing` when editing.
    pub fn build_event(&self, existing: Option<&Event>) -> Event {
        let now = OffsetDateTime::now_utc();

        let time = if self.all_day {
            let end_date = if self.end.date() > self.start.date() {
                self.end.date()
            } else {
                self.start
                    .date()
                    .next_day()
                    .unwrap_or_else(|| self.start.date())
            };
            EventTime::AllDay {
                start: self.start.date(),
                end: end_date,
            }
        } else {
            let end = if self.end > self.start {
                self.end
            } else {
                self.start + time::Duration::hours(1)
            };
            EventTime::Timed {
                start: self.start,
                end,
            }
        };

        let kind = if self.is_task {
            EventKind::Task(TaskDetails {
                due: None,
                completed_at: None,
                priority: None,
            })
        } else {
            EventKind::Event
        };

        Event {
            id: self.event_id.unwrap_or_default(),
            calendar: self.calendar.clone(),
            kind,
            summary: self.summary.clone(),
            description: (!self.description.is_empty()).then(|| self.description.clone()),
            location: (!self.location.is_empty()).then(|| self.location.clone()),
            time,
            timezone: None,
            color: None,
            rrule: self.recurrence.to_rrule_string(),
            exdates: existing.map_or(Vec::new(), |event| event.exdates.clone()),
            recurrence_id: None,
            attendees: existing.map_or(Vec::new(), |event| event.attendees.clone()),
            reminders: self
                .reminders
                .iter()
                .map(|minutes_before| Reminder {
                    minutes_before: *minutes_before,
                })
                .collect(),
            source: Source::Local,
            external: None,
            created_at: existing.map_or(now, |event| event.created_at),
            updated_at: now,
            sequence: existing.map_or(0, |event| event.sequence + 1),
        }
    }
}
