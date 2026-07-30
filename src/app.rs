// SPDX-License-Identifier: GPL-3.0-only

use crate::components::LocalCalendar;
use crate::shared::config::Config;
use crate::fl;
use crate::models::{
    Calendar, CalendarId, CalendarKey, Event, EventId, EventKind, EventTime, LocalCalendarMeta,
    Reminder, Source, TaskDetails,
};
use crate::services::{ics, CalendarServiceFactory};
use crate::shared::storage::LocalStorage;
use crate::Result;
use accounts::models::{Account, Service};
use accounts::AccountsClient;
use cosmic::app::context_drawer;
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::dialog::file_chooser;
use cosmic::iced::alignment::{Horizontal, Vertical};
use cosmic::iced::futures::channel::mpsc::Sender;
use cosmic::iced::{stream, Alignment, Length, Subscription};
use cosmic::prelude::*;
use cosmic::theme::spacing;
use cosmic::widget::segmented_button::SingleSelect;
use cosmic::widget::{self, menu, nav_bar};
use cosmic::{cosmic_theme, theme};
use futures_util::{SinkExt, StreamExt};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::ops::Index;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use time::OffsetDateTime;

mod flags;
pub use flags::*;
mod settings;
pub use settings::*;

const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const APP_ICON: &[u8] = include_bytes!("../resources/icons/hicolor/scalable/apps/icon.svg");

/// Wraps subscription data alongside a stable identifier used for hashing,
/// since the wrapped client itself does not implement [`std::hash::Hash`].
struct AccountSubscription<T>(&'static str, T);

impl<T> std::hash::Hash for AccountSubscription<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

/// The application model stores app-specific state used to describe its interface and
/// drive its logic.
pub struct AppModel {
    /// Application state which is managed by the COSMIC runtime.
    core: cosmic::Core,
    /// Display a context drawer with the designated page if defined.
    context_page: ContextPage,
    /// Contains items assigned to the nav bar panel.
    nav: nav_bar::Model,
    /// Contains items assigned to the tab bar.
    tabs: widget::segmented_button::Model<SingleSelect>,
    /// Key bindings for the application's menu bar.
    key_binds: HashMap<menu::KeyBind, MenuAction>,
    // Configuration data that persists between application runs.
    config: Config,
    // Calendar data that persists between application runs.
    calendar: LocalCalendar,
    client: Option<AccountsClient>,
    accounts: VecDeque<Account>,
    calendars: BTreeMap<Account, Vec<Calendar>>,
    // Local calendar/event storage, persisted to disk as JSON.
    storage: Arc<Mutex<LocalStorage>>,
    // State of the event editor drawer, when open.
    editing_event: Option<EventEditorState>,
    // Multi-select segmented button backing the reminder-preset picker in
    // the event editor. Rebuilt from `editing_event.reminders` whenever the
    // editor opens or a preset is toggled; `Vec<i64>` remains the source of
    // truth that's actually persisted, since `segmented_button::Model` isn't
    // `Clone` and can't live inside `EventEditorState`.
    reminders_model: widget::segmented_button::Model<widget::segmented_button::MultiSelect>,
    // State of the calendar manager drawer.
    calendar_manager: CalendarManagerState,
    // Nav bar entity ids for the "Calendars" header + per-calendar rows,
    // tracked so they can be torn down and rebuilt on every CRUD change.
    calendar_nav_ids: Vec<nav_bar::Id>,
    // Current text in the header search bar; non-empty replaces the main
    // view with search results.
    search_query: String,
    // Reminders already notified this session, so the periodic check
    // doesn't re-fire the same notification every tick.
    fired_reminders: std::collections::HashSet<(EventId, OffsetDateTime)>,
}

/// State of an in-progress edit to a calendar's name/color, shown inline in
/// the calendar manager.
#[derive(Debug, Clone)]
pub struct CalendarEditState {
    pub id: CalendarId,
    pub name: String,
    pub color: String,
}

/// State backing the calendar management drawer.
#[derive(Debug, Clone, Default)]
pub struct CalendarManagerState {
    pub new_name: String,
    pub editing: Option<CalendarEditState>,
}

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
fn shift_event_editor_start(state: &mut EventEditorState, new_start: OffsetDateTime) {
    let duration = state.end - state.start;
    state.start = new_start;
    state.end = new_start + duration;
}

/// The recurrence frequencies offered in the event editor, mapped to/from a
/// plain RFC 5545 `RRULE` string (no custom intervals/exceptions in the UI
/// yet — those can be added to the RRULE string format later without
/// changing the stored [`Event::rrule`] shape).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecurrenceFreq {
    None,
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

impl RecurrenceFreq {
    pub const ALL: [RecurrenceFreq; 5] = [
        RecurrenceFreq::None,
        RecurrenceFreq::Daily,
        RecurrenceFreq::Weekly,
        RecurrenceFreq::Monthly,
        RecurrenceFreq::Yearly,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            RecurrenceFreq::None => "Does not repeat",
            RecurrenceFreq::Daily => "Daily",
            RecurrenceFreq::Weekly => "Weekly",
            RecurrenceFreq::Monthly => "Monthly",
            RecurrenceFreq::Yearly => "Yearly",
        }
    }

    fn rrule_freq_code(self) -> Option<&'static str> {
        match self {
            RecurrenceFreq::None => None,
            RecurrenceFreq::Daily => Some("DAILY"),
            RecurrenceFreq::Weekly => Some("WEEKLY"),
            RecurrenceFreq::Monthly => Some("MONTHLY"),
            RecurrenceFreq::Yearly => Some("YEARLY"),
        }
    }
}

fn chrono_naive_date_to_time(date: chrono::NaiveDate) -> time::Date {
    use chrono::Datelike;
    time::Month::try_from(date.month() as u8)
        .ok()
        .and_then(|month| time::Date::from_calendar_date(date.year(), month, date.day() as u8).ok())
        .unwrap_or(time::Date::MIN)
}

/// The weekday codes used by RFC 5545 BYDAY, in Monday-first order matching
/// [`RecurrenceRule::by_weekday`] and `chrono::Weekday::num_days_from_monday`.
const RRULE_WEEKDAY_CODES: [&str; 7] = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"];

/// How a recurrence rule stops repeating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecurrenceEnd {
    Never,
    Until(time::Date),
    Count(u32),
}

/// The full set of recurrence options exposed by the event editor's advanced
/// recurrence sub-form. Converts to/from a plain RFC 5545 RRULE string, the
/// only representation actually stored on [`Event::rrule`] — the rest of the
/// app (`Event::occurrences`) already handles arbitrary valid RRULE strings,
/// so this is purely an editor-side convenience.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecurrenceRule {
    pub freq: RecurrenceFreq,
    pub interval: u16,
    /// Only meaningful (and only emitted into the RRULE) when `freq` is
    /// `Weekly`; Monday-first (`by_weekday[0]` = Monday).
    pub by_weekday: [bool; 7],
    pub end: RecurrenceEnd,
}

impl RecurrenceRule {
    pub const NONE: Self = Self {
        freq: RecurrenceFreq::None,
        interval: 1,
        by_weekday: [false; 7],
        end: RecurrenceEnd::Never,
    };

    pub fn to_rrule_string(&self) -> Option<String> {
        let freq_code = self.freq.rrule_freq_code()?;
        let mut parts = vec![format!("FREQ={freq_code}")];

        if self.interval > 1 {
            parts.push(format!("INTERVAL={}", self.interval));
        }

        if self.freq == RecurrenceFreq::Weekly {
            let days: Vec<&str> = RRULE_WEEKDAY_CODES
                .iter()
                .zip(self.by_weekday.iter())
                .filter(|(_, on)| **on)
                .map(|(code, _)| *code)
                .collect();
            if !days.is_empty() {
                parts.push(format!("BYDAY={}", days.join(",")));
            }
        }

        match self.end {
            RecurrenceEnd::Never => {}
            RecurrenceEnd::Count(count) => parts.push(format!("COUNT={count}")),
            RecurrenceEnd::Until(date) => {
                // UNTIL must match DTSTART's value type (date-time here);
                // pin it to the end of the chosen day so that day's
                // occurrence is still included regardless of start time.
                parts.push(format!(
                    "UNTIL={:04}{:02}{:02}T235959Z",
                    date.year(),
                    date.month() as u8,
                    date.day()
                ));
            }
        }

        Some(parts.join(";"))
    }

    /// Parses an existing RRULE string (as read from [`Event::rrule`]) back
    /// into editable form. Falls back to [`Self::NONE`] on any parse error
    /// rather than failing the whole editor open.
    pub fn from_rrule_string(rrule: Option<&str>) -> Self {
        let Some(rule_str) = rrule else {
            return Self::NONE;
        };
        let Ok(rule) = rule_str.parse::<rrule::RRule<rrule::Unvalidated>>() else {
            return Self::NONE;
        };

        let freq = match rule.get_freq() {
            rrule::Frequency::Daily => RecurrenceFreq::Daily,
            rrule::Frequency::Weekly => RecurrenceFreq::Weekly,
            rrule::Frequency::Monthly => RecurrenceFreq::Monthly,
            rrule::Frequency::Yearly => RecurrenceFreq::Yearly,
            _ => RecurrenceFreq::None,
        };

        let mut by_weekday = [false; 7];
        for nweekday in rule.get_by_weekday() {
            if let rrule::NWeekday::Every(weekday) = nweekday {
                by_weekday[weekday.num_days_from_monday() as usize] = true;
            }
        }

        let end = if let Some(count) = rule.get_count() {
            RecurrenceEnd::Count(count)
        } else if let Some(until) = rule.get_until() {
            RecurrenceEnd::Until(chrono_naive_date_to_time(until.naive_utc().date()))
        } else {
            RecurrenceEnd::Never
        };

        Self {
            freq,
            interval: rule.get_interval(),
            by_weekday,
            end,
        }
    }
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
    fn new(calendar: CalendarKey, start: OffsetDateTime) -> Self {
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

    fn from_event(event: &Event, occurrence_start: OffsetDateTime) -> Self {
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
    fn build_event(&self, existing: Option<&Event>) -> Event {
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

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    OpenRepositoryUrl,
    SubscriptionChannel,
    ToggleContextPage(ContextPage),
    UpdateConfig(Config),
    TabSelected(widget::segmented_button::Entity),
    LaunchUrl(String),
    NavigateNextDay,
    NavigatePreviousDay,
    NavigateNextMonth,
    NavigatePreviousMonth,
    NavigateNextYear,
    NavigatePreviousYear,
    NavigateToday,
    AddEvent(OffsetDateTime),
    SelectDate(OffsetDateTime),
    SelectMonth(usize),
    SelectYear(usize),
    SelectDay(usize),
    LoadClient,
    SetClient(Option<AccountsClient>),
    LoadAccounts,
    ReloadAccounts,
    SetAccounts(VecDeque<Account>),
    LoadCalendars,
    AddCalendars((Account, Vec<Calendar>)),
    OpenEventEditor(Option<EventId>, OffsetDateTime),
    CloseEventEditor,
    EventEditorSummaryChanged(String),
    EventEditorDescriptionChanged(String),
    EventEditorLocationChanged(String),
    EventEditorAllDayToggled(bool),
    EventEditorTaskToggled(bool),
    EventEditorStartYearChanged(usize),
    EventEditorStartMonthChanged(usize),
    EventEditorStartDayChanged(usize),
    EventEditorStartHourChanged(usize),
    EventEditorStartMinuteChanged(usize),
    EventEditorEndHourChanged(usize),
    EventEditorEndMinuteChanged(usize),
    EventEditorRecurrenceChanged(usize),
    EventEditorRecurrenceIntervalChanged(usize),
    EventEditorRecurrenceWeekdayToggled(usize, bool),
    EventEditorRecurrenceEndChanged(usize),
    EventEditorRecurrenceUntilYearChanged(usize),
    EventEditorRecurrenceUntilMonthChanged(usize),
    EventEditorRecurrenceUntilDayChanged(usize),
    EventEditorRecurrenceCountChanged(usize),
    EventEditorCalendarChanged(usize),
    SaveEvent,
    EventSaved(std::result::Result<Event, String>),
    DeleteEvent(EventId, CalendarKey),
    DeleteOccurrence(EventId, OffsetDateTime),
    EventDeleted(std::result::Result<EventId, String>),
    ImportIcs,
    IcsFileChosen(Option<PathBuf>),
    IcsImported(std::result::Result<usize, String>),
    ExportIcs,
    IcsExported(std::result::Result<PathBuf, String>),
    OpenNewCalendarForm,
    CalendarManagerNewNameChanged(String),
    CreateCalendar,
    CalendarCreated(std::result::Result<LocalCalendarMeta, String>),
    StartEditCalendar(CalendarId),
    CalendarEditNameChanged(String),
    CalendarEditColorChanged(String),
    SaveCalendarEdit,
    CalendarEditSaved(std::result::Result<(), String>),
    CancelCalendarEdit,
    ToggleCalendarVisibility(CalendarId, bool),
    CalendarVisibilityToggled(std::result::Result<(), String>),
    DeleteCalendar(CalendarId),
    CalendarDeleted(std::result::Result<CalendarId, String>),
    SearchChanged(String),
    SelectSearchResult(EventId, OffsetDateTime),
    EventEditorReminderToggled(widget::segmented_button::Entity),
    EventEditorCustomReminderInputChanged(String),
    EventEditorCustomReminderEditToggled(bool),
    EventEditorAddCustomReminder,
    CheckReminders,
    RemindersFired(Vec<(EventId, OffsetDateTime)>),
}

/// Create a COSMIC application from the app model
impl cosmic::Application for AppModel {
    /// The async executor that will be used to run your application's commands.
    type Executor = cosmic::executor::Default;

    /// Data that your application receives to its init method.
    type Flags = Flags;

    /// Messages which the application and its widgets will emit.
    type Message = Message;

    /// Unique identifier in RDNN (reverse domain name notation) format.
    const APP_ID: &'static str = "dev.edfloreshz.Calendar";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    /// Initializes the application with any given flags and startup commands.
    fn init(
        core: cosmic::Core,
        _flags: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        // Create a nav bar with three page items.
        let nav = nav_bar::Model::default();

        // Create a tab bar with three page items.
        let tabs = widget::segmented_button::Model::builder()
            .insert(|b| {
                b.text(fl!("month"))
                    .icon(widget::icon::from_name("office-calendar-symbolic"))
                    .data(Tab::Month)
                    .activate()
            })
            .insert(|b| {
                b.text(fl!("week"))
                    .icon(widget::icon::from_name("x-office-spreadsheet-symbolic"))
                    .data(Tab::Week)
            })
            .insert(|b| {
                b.text(fl!("day"))
                    .icon(widget::icon::from_name("calendar-go-today-symbolic"))
                    .data(Tab::Day)
            })
            .build();

        // Construct the app model with the runtime's core.
        let mut app = AppModel {
            core,
            context_page: ContextPage::default(),
            nav,
            tabs,
            key_binds: HashMap::new(),
            // Optional configuration file for an application.
            config: cosmic_config::Config::new(Self::APP_ID, Config::VERSION)
                .map(|context| match Config::get_entry(&context) {
                    Ok(config) => config,
                    Err((_errors, config)) => {
                        // for why in errors {
                        //     tracing::error!(%why, "error loading app config");
                        // }

                        config
                    }
                })
                .unwrap_or_default(),
            calendar: LocalCalendar::default(),
            client: None,
            accounts: VecDeque::new(),
            calendars: BTreeMap::new(),
            storage: Arc::new(Mutex::new(LocalStorage::load().unwrap_or_else(|err| {
                tracing::error!("failed to load local storage: {err}");
                LocalStorage::empty()
            }))),
            editing_event: None,
            reminders_model: build_reminders_model(&[]),
            calendar_manager: CalendarManagerState::default(),
            calendar_nav_ids: Vec::new(),
            search_query: String::new(),
            fired_reminders: std::collections::HashSet::new(),
        };

        // Seed a default calendar on first run so the app is usable offline
        // without requiring a linked account.
        {
            let mut storage = app.storage.lock().unwrap();
            if storage.list_calendars().is_empty() {
                let personal = LocalCalendarMeta {
                    id: crate::models::CalendarId::new(),
                    name: fl!("personal-calendar"),
                    description: None,
                    color: None,
                    visible: true,
                };
                if let Err(err) = storage.create_calendar(personal) {
                    tracing::error!("failed to create default calendar: {err}");
                }
            }
        }

        app.refresh_calendar_nav();

        app.core.nav_bar_set_toggled(false);

        // Create a startup command that sets the window title.
        let command = app.update_title();

        (
            app,
            Task::batch(vec![command, cosmic::task::message(Message::LoadClient)]),
        )
    }

    /// Elements to pack at the start of the header bar.
    fn header_start<'a>(&'a self) -> Vec<Element<'a, Self::Message>> {
        let menu_bar = menu::bar(vec![menu::Tree::with_children(
            menu::root(fl!("view")).apply(Element::from),
            menu::items(
                &self.key_binds,
                vec![
                    menu::Item::Button(fl!("import-ics"), None, MenuAction::ImportIcs),
                    menu::Item::Button(fl!("export-ics"), None, MenuAction::ExportIcs),
                    menu::Item::Button(fl!("about"), None, MenuAction::About),
                ],
            ),
        )]);

        vec![menu_bar.into()]
    }

    fn header_center<'a>(&'a self) -> Vec<Element<'a, Self::Message>> {
        vec![widget::text(format!(
            "{} {}",
            self.calendar.selected_date.month(),
            self.calendar.selected_date.year()
        ))
        .into()]
    }

    fn header_end<'a>(&'a self) -> Vec<Element<'a, Self::Message>> {
        vec![widget::search_input(fl!("search"), &self.search_query)
            .on_input(Message::SearchChanged)
            .on_clear(Message::SearchChanged(String::new()))
            .width(Length::Fixed(240.0))
            .into()]
    }

    fn footer<'a>(&'a self) -> Option<Element<'a, Self::Message>> {
        let today = widget::button::icon(widget::icon::from_name("calendar-go-today-symbolic"))
            .tooltip(fl!("today"))
            .class(cosmic::style::Button::NavToggle)
            .on_press_maybe((!self.calendar.today()).then_some(Message::NavigateToday));

        let days = widget::row(vec![
            widget::button::icon(widget::icon::from_name("go-previous-symbolic"))
                .on_press(Message::NavigatePreviousDay)
                .into(),
            widget::dropdown(
                self.calendar
                    .days()
                    .iter()
                    .map(|m| m.to_string())
                    .collect::<Vec<String>>(),
                self.calendar
                    .days()
                    .iter()
                    .position(|m| *m == self.calendar.selected_date.day()),
                Message::SelectDay,
            )
            .into(),
            widget::button::icon(widget::icon::from_name("go-next-symbolic"))
                .on_press(Message::NavigateNextDay)
                .into(),
        ]);

        let months = widget::row(vec![
            widget::button::icon(widget::icon::from_name("go-previous-symbolic"))
                .on_press(Message::NavigatePreviousMonth)
                .into(),
            widget::dropdown(
                self.calendar
                    .months()
                    .iter()
                    .map(|m| m.to_string())
                    .collect::<Vec<String>>(),
                self.calendar
                    .months()
                    .iter()
                    .position(|m| *m == self.calendar.selected_date.month()),
                Message::SelectMonth,
            )
            .into(),
            widget::button::icon(widget::icon::from_name("go-next-symbolic"))
                .on_press(Message::NavigateNextMonth)
                .into(),
        ]);

        let years = widget::row(vec![
            widget::button::icon(widget::icon::from_name("go-previous-symbolic"))
                .on_press(Message::NavigatePreviousYear)
                .into(),
            widget::dropdown(
                self.calendar
                    .years()
                    .iter()
                    .map(|m| m.to_string())
                    .collect::<Vec<String>>(),
                self.calendar
                    .years()
                    .iter()
                    .position(|y| *y == self.calendar.selected_date.year()),
                Message::SelectYear,
            )
            .into(),
            widget::button::icon(widget::icon::from_name("go-next-symbolic"))
                .on_press(Message::NavigateNextYear)
                .into(),
        ])
        .align_y(Vertical::Center)
        .spacing(spacing().space_xxs);

        let create_event = widget::row(vec![widget::button::icon(widget::icon::from_name(
            "list-add-symbolic",
        ))
        .tooltip(fl!("crate-event"))
        .on_press(Message::AddEvent(OffsetDateTime::now_local().unwrap()))
        .into()])
        .align_y(Vertical::Center)
        .spacing(spacing().space_xxs);

        Some(
            widget::container(widget::flex_row(vec![
                today.into(),
                widget::space::horizontal().into(),
                days.into(),
                months.into(),
                years.into(),
                widget::space::horizontal().into(),
                create_event.into(),
            ]))
            .align_x(Horizontal::Center)
            .class(cosmic::theme::Container::Card)
            .padding(spacing().space_xxxs)
            .width(Length::Fill)
            .into(),
        )
    }

    /// Enables the COSMIC application to create a nav bar with this model.
    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav)
    }

    /// The nav bar's per-item right-click menu: "New calendar" on the
    /// "Calendars" header row, "Rename"/"Delete" on each calendar row.
    /// `nav_bar`'s context menu implementation asserts that this yields
    /// exactly one entry (possibly empty) per nav item, in iteration order.
    fn nav_context_menu(&self) -> Option<Vec<menu::Tree<cosmic::Action<Self::Message>>>> {
        let can_delete = self.storage.lock().unwrap().list_calendars().len() > 1;
        let header_id = self.calendar_nav_ids.first().copied();

        Some(menu::nav_context(
            &HashMap::new(),
            self.nav
                .iter()
                .map(|id| {
                    if let Some(calendar_id) = self.nav.data::<CalendarId>(id).copied() {
                        let mut items = vec![menu::Item::Button(
                            fl!("rename"),
                            None,
                            NavMenuAction::RenameCalendar(calendar_id),
                        )];
                        if can_delete {
                            items.push(menu::Item::Button(
                                fl!("delete"),
                                None,
                                NavMenuAction::DeleteCalendar(calendar_id),
                            ));
                        }
                        items
                    } else if Some(id) == header_id {
                        vec![menu::Item::Button(
                            fl!("new-calendar"),
                            None,
                            NavMenuAction::NewCalendar,
                        )]
                    } else {
                        Vec::new()
                    }
                })
                .collect(),
        ))
    }

    /// Display a context drawer if the context page is requested.
    fn context_drawer<'a>(&'a self) -> Option<context_drawer::ContextDrawer<'a, Self::Message>> {
        if !self.core.window.show_context {
            return None;
        }

        Some(match self.context_page {
            ContextPage::About => context_drawer::context_drawer(
                self.about(),
                Message::ToggleContextPage(ContextPage::About),
            )
            .title(fl!("about")),
            ContextPage::CalendarManager => context_drawer::context_drawer(
                self.calendar_manager_view(),
                Message::ToggleContextPage(ContextPage::CalendarManager),
            )
            .title(if self.calendar_manager.editing.is_some() {
                fl!("edit-calendar")
            } else {
                fl!("new-calendar")
            }),
        })
    }

    /// Displays the event creation/editing form as a centered dialog rather
    /// than a side drawer: it's a focused, modal task (unlike the calendar
    /// manager or about page, which make sense alongside the calendar view).
    fn dialog<'a>(&'a self) -> Option<Element<'a, Self::Message>> {
        self.editing_event.as_ref().map(|_| self.event_editor_view())
    }

    /// Describes the interface based on the current state of the application model.
    ///
    /// Application events will be processed through the view. Any messages emitted by
    /// events received by widgets will be passed to the update method.
    fn view<'a>(&'a self) -> Element<'a, Self::Message> {
        let tabs = widget::segmented_button::horizontal(&self.tabs)
            .padding(2)
            .button_spacing(spacing().space_xxs)
            .button_alignment(cosmic::iced::Alignment::Center)
            .on_activate(Message::TabSelected)
            .into();

        let events: Vec<Event> = {
            let storage = self.storage.lock().unwrap();
            let center = self.calendar.selected_date;
            let range = (center - time::Duration::days(60))..(center + time::Duration::days(60));
            let hidden: std::collections::HashSet<crate::models::CalendarId> = storage
                .list_calendars()
                .into_iter()
                .filter(|meta| !meta.visible)
                .map(|meta| meta.id)
                .collect();
            storage
                .events_in_range(range)
                .into_iter()
                .filter(|event| match &event.calendar {
                    CalendarKey::Local(id) => !hidden.contains(id),
                    CalendarKey::Remote { .. } => true,
                })
                .cloned()
                .collect()
        };

        let active_tab = if !self.search_query.is_empty() {
            self.search_results_view()
        } else {
            match self.tabs.active_data::<Tab>() {
                Some(active_tab) => match active_tab {
                    Tab::Month => self.calendar.month_view(&events).into(),
                    Tab::Week => self.calendar.week_view(&events).into(),
                    Tab::Day => self.calendar.day_view(&events).into(),
                },
                None => widget::text::title1("Welcome")
                    .apply(widget::container)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(Horizontal::Center)
                    .align_y(Vertical::Center)
                    .into(),
            }
        };

        widget::column(vec![tabs, active_tab])
            .spacing(spacing().space_xxs)
            .into()
    }

    /// Register subscriptions for this application.
    ///
    /// Subscriptions are long-running async tasks running in the background which
    /// emit messages to the application through a channel. They are started at the
    /// beginning of the application, and persist through its lifetime.
    fn subscription(&self) -> Subscription<Self::Message> {
        let reminders_tick =
            cosmic::iced::time::every(std::time::Duration::from_secs(30)).map(|_| Message::CheckReminders);

        let Some(client) = self.client.clone() else {
            return Subscription::batch(vec![reminders_tick]);
        };
        let account_changed_client = client.clone();
        let account_removed_client = client.clone();

        Subscription::batch(vec![
            reminders_tick,
            // Create a subscription which emits updates through a channel.
            Subscription::run_with((), |()| {
                cosmic::iced::stream::channel(4, move |mut channel: Sender<Message>| async move {
                    _ = channel.send(Message::SubscriptionChannel).await;

                    futures_util::future::pending().await
                })
            }),
            // Watch for application configuration changes.
            self.core()
                .watch_config::<Config>(Self::APP_ID)
                .map(|update| {
                    // for why in update.errors {
                    //     tracing::error!(?why, "app config error");
                    // }

                    Message::UpdateConfig(update.config)
                }),
            Subscription::run_with(
                AccountSubscription("account_added", client),
                |AccountSubscription(_, client)| {
                    let client = client.clone();
                    stream::channel(1, move |mut output: Sender<Message>| async move {
                        if let Ok(mut account_added_stream) = client.receive_account_added().await {
                            while let Some(_) = account_added_stream.next().await {
                                if let Err(err) = output.send(Message::ReloadAccounts).await {
                                    tracing::warn!(
                                        "failed to send message from subscription: {}",
                                        err
                                    );
                                }
                            }
                        }
                    })
                },
            ),
            Subscription::run_with(
                AccountSubscription("account_changed", account_changed_client),
                |AccountSubscription(_, client)| {
                    let client = client.clone();
                    stream::channel(1, move |mut output: Sender<Message>| async move {
                        if let Ok(mut account_changed_stream) =
                            client.receive_account_changed().await
                        {
                            while let Some(_) = account_changed_stream.next().await {
                                if let Err(err) = output.send(Message::ReloadAccounts).await {
                                    tracing::warn!(
                                        "failed to send message from subscription: {}",
                                        err
                                    );
                                }
                            }
                        }
                    })
                },
            ),
            Subscription::run_with(
                AccountSubscription("account_removed", account_removed_client),
                |AccountSubscription(_, client)| {
                    let client = client.clone();
                    stream::channel(1, move |mut output: Sender<Message>| async move {
                        if let Ok(mut account_removed_stream) =
                            client.receive_account_removed().await
                        {
                            while let Some(_) = account_removed_stream.next().await {
                                if let Err(err) = output.send(Message::ReloadAccounts).await {
                                    tracing::warn!(
                                        "failed to send message from subscription: {}",
                                        err
                                    );
                                }
                            }
                        }
                    })
                },
            ),
        ])
    }

    /// Handles messages emitted by the application and its widgets.
    ///
    /// Tasks may be returned for asynchronous execution of code in the background
    /// on the application's async runtime.
    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        let mut tasks = vec![];
        match message {
            Message::TabSelected(tab) => {
                self.tabs.activate(tab);
            }
            Message::OpenRepositoryUrl => {
                _ = open::that_detached(REPOSITORY);
            }
            Message::SubscriptionChannel => {
                // For example purposes only.
            }
            Message::ToggleContextPage(context_page) => {
                if self.context_page == context_page {
                    // Close the context drawer if the toggled context page is the same.
                    self.core.window.show_context = !self.core.window.show_context;
                } else {
                    // Open the context drawer to display the requested context page.
                    self.context_page = context_page;
                    self.core.window.show_context = true;
                }
            }
            Message::UpdateConfig(config) => {
                self.config = config;
            }
            Message::LaunchUrl(url) => match open::that_detached(&url) {
                Ok(()) => {}
                Err(err) => {
                    eprintln!("failed to open {url:?}: {err}");
                }
            },
            Message::LoadClient => tasks.push(Task::perform(
                async { AccountsClient::new().await.ok() },
                |client| cosmic::action::app(Message::SetClient(client)),
            )),
            Message::SetClient(client) => {
                self.client = client;
                tasks.push(cosmic::task::message(Message::LoadAccounts));
            }
            Message::ReloadAccounts => {
                self.nav.clear();
                tasks.push(cosmic::task::message(Message::LoadAccounts));
            }
            Message::LoadAccounts => {
                if let Some(client) = self.client.as_ref() {
                    let client = client.clone();
                    tasks.push(Task::perform(
                        async move {
                            let accounts = client.list_enabled_accounts(Service::Calendar).await?;
                            Ok(VecDeque::from(accounts))
                        },
                        |accounts: Result<VecDeque<Account>>| match accounts {
                            Ok(accounts) => cosmic::action::app(Message::SetAccounts(accounts)),
                            Err(err) => {
                                tracing::error!("Failed to load accounts: {}", err);
                                cosmic::action::none()
                            }
                        },
                    ));
                }
            }
            Message::SetAccounts(accounts) => {
                self.accounts = accounts;
                tasks.push(cosmic::task::message(Message::LoadCalendars));
            }
            Message::LoadCalendars => {
                let accounts = self.accounts.clone();
                for account in accounts {
                    tasks.push(cosmic::Task::perform(
                        async move {
                            let mut service = CalendarServiceFactory::get_service(&account).await?;
                            let calendars = service.fetch_calendars().await?;
                            Ok((account.clone(), calendars))
                        },
                        |calendars: Result<(Account, Vec<Calendar>)>| match calendars {
                            Ok((account, calendars)) => {
                                cosmic::action::app(Message::AddCalendars((account, calendars)))
                            }
                            Err(err) => {
                                tracing::error!("Failed to load calendars: {}", err);
                                cosmic::action::none()
                            }
                        },
                    ));
                }
            }
            Message::AddCalendars((account, calendars)) => {
                self.core.nav_bar_set_toggled(true);
                self.calendars.insert(account.clone(), calendars.clone());
                self.nav.insert().text(account.username);
                for calendar in calendars {
                    self.nav
                        .insert()
                        .indent(1)
                        .text(calendar.name.clone())
                        .icon(widget::icon::from_name("office-calendar-symbolic"))
                        .data(calendar);
                }
            }
            Message::AddEvent(date) => {
                tasks.push(cosmic::task::message(Message::OpenEventEditor(None, date)));
            }
            Message::OpenEventEditor(event_id, date) => {
                let state = match event_id.and_then(|id| self.storage.lock().unwrap().get_event(id).cloned()) {
                    Some(event) => EventEditorState::from_event(&event, date),
                    None => {
                        let calendar = self
                            .storage
                            .lock()
                            .unwrap()
                            .list_calendars()
                            .first()
                            .map(|meta| CalendarKey::Local(meta.id))
                            .unwrap_or_else(|| CalendarKey::Local(crate::models::CalendarId::new()));
                        EventEditorState::new(calendar, date)
                    }
                };
                self.reminders_model = build_reminders_model(&state.reminders);
                self.editing_event = Some(state);
            }
            Message::CloseEventEditor => {
                self.editing_event = None;
            }
            Message::EventEditorSummaryChanged(value) => {
                if let Some(state) = self.editing_event.as_mut() {
                    state.summary = value;
                }
            }
            Message::EventEditorDescriptionChanged(value) => {
                if let Some(state) = self.editing_event.as_mut() {
                    state.description = value;
                }
            }
            Message::EventEditorLocationChanged(value) => {
                if let Some(state) = self.editing_event.as_mut() {
                    state.location = value;
                }
            }
            Message::EventEditorAllDayToggled(value) => {
                if let Some(state) = self.editing_event.as_mut() {
                    state.all_day = value;
                }
            }
            Message::EventEditorTaskToggled(value) => {
                if let Some(state) = self.editing_event.as_mut() {
                    state.is_task = value;
                }
            }
            Message::EventEditorStartYearChanged(idx) => {
                if let Some(state) = self.editing_event.as_mut() {
                    let year = EVENT_EDITOR_YEARS[0] + idx as i32;
                    if let Ok(new_start) = state.start.replace_year(year) {
                        shift_event_editor_start(state, new_start);
                    }
                }
            }
            Message::EventEditorStartMonthChanged(idx) => {
                if let Some(state) = self.editing_event.as_mut() {
                    if let Some(month) = event_editor_months().get(idx) {
                        let day = state
                            .start
                            .day()
                            .min(month.length(state.start.year()));
                        if let Ok(new_start) = state
                            .start
                            .replace_month(*month)
                            .and_then(|dt| dt.replace_day(day))
                        {
                            shift_event_editor_start(state, new_start);
                        }
                    }
                }
            }
            Message::EventEditorStartDayChanged(idx) => {
                if let Some(state) = self.editing_event.as_mut() {
                    let day = idx as u8 + 1;
                    if let Ok(new_start) = state.start.replace_day(day) {
                        shift_event_editor_start(state, new_start);
                    }
                }
            }
            Message::EventEditorStartHourChanged(idx) => {
                if let Some(state) = self.editing_event.as_mut() {
                    if let Ok(new_start) = state.start.replace_hour(idx as u8) {
                        shift_event_editor_start(state, new_start);
                    }
                }
            }
            Message::EventEditorStartMinuteChanged(idx) => {
                if let Some(state) = self.editing_event.as_mut() {
                    if let Some(minute) = EVENT_EDITOR_MINUTES.get(idx) {
                        if let Ok(new_start) = state.start.replace_minute(*minute) {
                            shift_event_editor_start(state, new_start);
                        }
                    }
                }
            }
            Message::EventEditorEndHourChanged(idx) => {
                if let Some(state) = self.editing_event.as_mut() {
                    if let Ok(new_end) = state.end.replace_hour(idx as u8) {
                        state.end = new_end;
                    }
                }
            }
            Message::EventEditorEndMinuteChanged(idx) => {
                if let Some(state) = self.editing_event.as_mut() {
                    if let Some(minute) = EVENT_EDITOR_MINUTES.get(idx) {
                        if let Ok(new_end) = state.end.replace_minute(*minute) {
                            state.end = new_end;
                        }
                    }
                }
            }
            Message::EventEditorRecurrenceChanged(idx) => {
                if let Some(state) = self.editing_event.as_mut() {
                    if let Some(freq) = RecurrenceFreq::ALL.get(idx) {
                        state.recurrence.freq = *freq;
                    }
                }
            }
            Message::EventEditorRecurrenceIntervalChanged(idx) => {
                if let Some(state) = self.editing_event.as_mut() {
                    state.recurrence.interval = idx as u16 + 1;
                }
            }
            Message::EventEditorRecurrenceWeekdayToggled(idx, on) => {
                if let Some(state) = self.editing_event.as_mut() {
                    if idx < 7 {
                        state.recurrence.by_weekday[idx] = on;
                    }
                }
            }
            Message::EventEditorRecurrenceEndChanged(idx) => {
                if let Some(state) = self.editing_event.as_mut() {
                    state.recurrence.end = match idx {
                        1 => RecurrenceEnd::Until(state.start.date() + time::Duration::days(30)),
                        2 => RecurrenceEnd::Count(10),
                        _ => RecurrenceEnd::Never,
                    };
                }
            }
            Message::EventEditorRecurrenceUntilYearChanged(idx) => {
                if let Some(state) = self.editing_event.as_mut() {
                    if let RecurrenceEnd::Until(until) = &mut state.recurrence.end {
                        let year = EVENT_EDITOR_YEARS[0] + idx as i32;
                        if let Ok(new_date) = until.replace_year(year) {
                            *until = new_date;
                        }
                    }
                }
            }
            Message::EventEditorRecurrenceUntilMonthChanged(idx) => {
                if let Some(state) = self.editing_event.as_mut() {
                    if let RecurrenceEnd::Until(until) = &mut state.recurrence.end {
                        if let Some(month) = event_editor_months().get(idx) {
                            let day = until.day().min(month.length(until.year()));
                            if let Ok(new_date) =
                                until.replace_month(*month).and_then(|d| d.replace_day(day))
                            {
                                *until = new_date;
                            }
                        }
                    }
                }
            }
            Message::EventEditorRecurrenceUntilDayChanged(idx) => {
                if let Some(state) = self.editing_event.as_mut() {
                    if let RecurrenceEnd::Until(until) = &mut state.recurrence.end {
                        if let Ok(new_date) = until.replace_day(idx as u8 + 1) {
                            *until = new_date;
                        }
                    }
                }
            }
            Message::EventEditorRecurrenceCountChanged(idx) => {
                if let Some(state) = self.editing_event.as_mut() {
                    if let RecurrenceEnd::Count(count) = &mut state.recurrence.end {
                        *count = idx as u32 + 1;
                    }
                }
            }
            Message::EventEditorCalendarChanged(idx) => {
                if let Some(state) = self.editing_event.as_mut() {
                    let storage = self.storage.lock().unwrap();
                    let mut calendars = storage.list_calendars();
                    calendars.sort_by(|a, b| a.name.cmp(&b.name));
                    if let Some(meta) = calendars.get(idx) {
                        state.calendar = CalendarKey::Local(meta.id);
                    }
                }
            }
            Message::SaveEvent => {
                if let Some(state) = self.editing_event.clone() {
                    let storage = self.storage.clone();
                    let existing = state.event_id.and_then(|id| storage.lock().unwrap().get_event(id).cloned());
                    let event = state.build_event(existing.as_ref());
                    tasks.push(Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                let mut storage = storage.lock().unwrap();
                                storage.create_event(event.clone()).map(|()| event)
                            })
                            .await
                            .map_err(|err| err.to_string())?
                            .map_err(|err| err.to_string())
                        },
                        |result| cosmic::action::app(Message::EventSaved(result)),
                    ));
                }
            }
            Message::EventSaved(result) => match result {
                Ok(_event) => {
                    self.editing_event = None;
                }
                Err(err) => tracing::error!("failed to save event: {err}"),
            },
            Message::DeleteEvent(id, calendar) => {
                let storage = self.storage.clone();
                tasks.push(Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let mut storage = storage.lock().unwrap();
                            storage.delete_event(&calendar, id).map(|()| id)
                        })
                        .await
                        .map_err(|err| err.to_string())?
                        .map_err(|err| err.to_string())
                    },
                    |result| cosmic::action::app(Message::EventDeleted(result)),
                ));
            }
            Message::DeleteOccurrence(id, occurrence_start) => {
                let storage = self.storage.clone();
                tasks.push(Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let mut storage = storage.lock().unwrap();
                            let Some(mut event) = storage.get_event(id).cloned() else {
                                return Err("event not found".to_string());
                            };
                            event.exdates.push(occurrence_start);
                            event.updated_at = OffsetDateTime::now_utc();
                            event.sequence += 1;
                            storage
                                .update_event(event)
                                .map(|()| id)
                                .map_err(|err| err.to_string())
                        })
                        .await
                        .map_err(|err| err.to_string())?
                    },
                    |result| cosmic::action::app(Message::EventDeleted(result)),
                ));
            }
            Message::EventDeleted(result) => match result {
                Ok(_id) => {
                    self.editing_event = None;
                }
                Err(err) => tracing::error!("failed to delete event: {err}"),
            },
            Message::ImportIcs => {
                tasks.push(Task::perform(
                    async {
                        let dialog = file_chooser::open::Dialog::new().title(fl!("import-ics"));
                        match dialog.open_file().await {
                            Ok(response) => response.url().to_file_path().ok(),
                            Err(_) => None,
                        }
                    },
                    |path| cosmic::action::app(Message::IcsFileChosen(path)),
                ));
            }
            Message::IcsFileChosen(path) => {
                if let Some(path) = path {
                    let storage = self.storage.clone();
                    tasks.push(Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                let mut storage = storage.lock().unwrap();
                                let calendar = storage
                                    .list_calendars()
                                    .first()
                                    .map(|meta| CalendarKey::Local(meta.id))
                                    .ok_or_else(|| "no local calendar to import into".to_string())?;
                                let text = std::fs::read_to_string(&path)
                                    .map_err(|err| err.to_string())?;
                                let events =
                                    ics::import_ics(&text, calendar).map_err(|err| err.to_string())?;
                                let count = events.len();
                                for event in events {
                                    storage.create_event(event).map_err(|err| err.to_string())?;
                                }
                                Ok(count)
                            })
                            .await
                            .map_err(|err| err.to_string())?
                        },
                        |result| cosmic::action::app(Message::IcsImported(result)),
                    ));
                }
            }
            Message::IcsImported(result) => match result {
                Ok(count) => tracing::info!("imported {count} event(s) from ICS"),
                Err(err) => tracing::error!("failed to import ICS file: {err}"),
            },
            Message::ExportIcs => {
                let storage = self.storage.clone();
                tasks.push(Task::perform(
                    async move {
                        let dialog = file_chooser::save::Dialog::new()
                            .title(fl!("export-ics"))
                            .file_name("calendar.ics".to_string());
                        let path = match dialog.save_file().await {
                            Ok(response) => response.url().and_then(|url| url.to_file_path().ok()),
                            Err(_) => None,
                        };
                        let Some(path) = path else {
                            return Err("export cancelled".to_string());
                        };

                        tokio::task::spawn_blocking(move || {
                            let storage = storage.lock().unwrap();
                            let events: Vec<Event> =
                                storage.all_events().into_iter().cloned().collect();
                            let calendar = ics::export_events(&events);
                            std::fs::write(&path, calendar.to_string())
                                .map(|()| path)
                                .map_err(|err| err.to_string())
                        })
                        .await
                        .map_err(|err| err.to_string())?
                    },
                    |result| cosmic::action::app(Message::IcsExported(result)),
                ));
            }
            Message::IcsExported(result) => match result {
                Ok(path) => tracing::info!("exported calendar to {}", path.display()),
                Err(err) => tracing::error!("failed to export ICS file: {err}"),
            },
            Message::OpenNewCalendarForm => {
                self.calendar_manager = CalendarManagerState::default();
                self.context_page = ContextPage::CalendarManager;
                self.core.window.show_context = true;
            }
            Message::CalendarManagerNewNameChanged(value) => {
                self.calendar_manager.new_name = value;
            }
            Message::CreateCalendar => {
                let name = self.calendar_manager.new_name.trim().to_string();
                if !name.is_empty() {
                    let storage = self.storage.clone();
                    tasks.push(Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                let meta = LocalCalendarMeta {
                                    id: CalendarId::new(),
                                    name,
                                    description: None,
                                    color: None,
                                    visible: true,
                                };
                                let mut storage = storage.lock().unwrap();
                                storage.create_calendar(meta.clone()).map(|()| meta)
                            })
                            .await
                            .map_err(|err| err.to_string())?
                            .map_err(|err| err.to_string())
                        },
                        |result| cosmic::action::app(Message::CalendarCreated(result)),
                    ));
                }
            }
            Message::CalendarCreated(result) => match result {
                Ok(_meta) => {
                    self.calendar_manager.new_name.clear();
                    self.core.window.show_context = false;
                    self.refresh_calendar_nav();
                }
                Err(err) => tracing::error!("failed to create calendar: {err}"),
            },
            Message::StartEditCalendar(id) => {
                if let Some(meta) = self.storage.lock().unwrap().get_calendar(id) {
                    self.calendar_manager.editing = Some(CalendarEditState {
                        id,
                        name: meta.name.clone(),
                        color: meta.color.clone().unwrap_or_default(),
                    });
                    self.context_page = ContextPage::CalendarManager;
                    self.core.window.show_context = true;
                }
            }
            Message::CalendarEditNameChanged(value) => {
                if let Some(editing) = self.calendar_manager.editing.as_mut() {
                    editing.name = value;
                }
            }
            Message::CalendarEditColorChanged(value) => {
                if let Some(editing) = self.calendar_manager.editing.as_mut() {
                    editing.color = value;
                }
            }
            Message::SaveCalendarEdit => {
                if let Some(editing) = self.calendar_manager.editing.clone() {
                    let storage = self.storage.clone();
                    tasks.push(Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                let mut storage = storage.lock().unwrap();
                                let Some(mut meta) = storage.get_calendar(editing.id).cloned()
                                else {
                                    return Err("calendar not found".to_string());
                                };
                                meta.name = editing.name;
                                meta.color = (!editing.color.is_empty()).then_some(editing.color);
                                storage.update_calendar(meta).map_err(|err| err.to_string())
                            })
                            .await
                            .map_err(|err| err.to_string())?
                        },
                        |result| cosmic::action::app(Message::CalendarEditSaved(result)),
                    ));
                }
            }
            Message::CalendarEditSaved(result) => match result {
                Ok(()) => {
                    self.calendar_manager.editing = None;
                    self.core.window.show_context = false;
                    self.refresh_calendar_nav();
                }
                Err(err) => tracing::error!("failed to save calendar: {err}"),
            },
            Message::CancelCalendarEdit => {
                self.calendar_manager.editing = None;
                self.core.window.show_context = false;
            }
            Message::ToggleCalendarVisibility(id, visible) => {
                let storage = self.storage.clone();
                tasks.push(Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let mut storage = storage.lock().unwrap();
                            let Some(mut meta) = storage.get_calendar(id).cloned() else {
                                return Err("calendar not found".to_string());
                            };
                            meta.visible = visible;
                            storage.update_calendar(meta).map_err(|err| err.to_string())
                        })
                        .await
                        .map_err(|err| err.to_string())?
                    },
                    |result| cosmic::action::app(Message::CalendarVisibilityToggled(result)),
                ));
            }
            Message::CalendarVisibilityToggled(result) => match result {
                Ok(()) => self.refresh_calendar_nav(),
                Err(err) => tracing::error!("failed to toggle calendar visibility: {err}"),
            },
            Message::DeleteCalendar(id) => {
                let storage = self.storage.clone();
                tasks.push(Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let mut storage = storage.lock().unwrap();
                            storage.delete_calendar(id).map(|()| id).map_err(|err| err.to_string())
                        })
                        .await
                        .map_err(|err| err.to_string())?
                    },
                    |result| cosmic::action::app(Message::CalendarDeleted(result)),
                ));
            }
            Message::CalendarDeleted(result) => match result {
                Ok(_id) => self.refresh_calendar_nav(),
                Err(err) => tracing::error!("failed to delete calendar: {err}"),
            },
            Message::SearchChanged(query) => {
                self.search_query = query;
            }
            Message::SelectSearchResult(id, start) => {
                self.search_query.clear();
                self.calendar.set_date(start);
                tasks.push(cosmic::task::message(Message::OpenEventEditor(
                    Some(id),
                    start,
                )));
            }
            Message::EventEditorReminderToggled(entity) => {
                let Some(minutes) = self.reminders_model.data::<i64>(entity).copied() else {
                    return Task::batch(tasks);
                };
                let now_on = !self.reminders_model.is_active(entity);
                if let Some(state) = self.editing_event.as_mut() {
                    if now_on {
                        if !state.reminders.contains(&minutes) {
                            state.reminders.push(minutes);
                        }
                    } else {
                        state.reminders.retain(|m| *m != minutes);
                    }
                    // Rebuilt rather than toggled in place: a custom (non-preset)
                    // entry has no "off" state to fall back to, so turning it off
                    // must drop it from the model entirely, not just deactivate it.
                    self.reminders_model = build_reminders_model(&state.reminders);
                }
            }
            Message::EventEditorCustomReminderInputChanged(value) => {
                if let Some(state) = self.editing_event.as_mut() {
                    state.custom_reminder_input = value;
                }
            }
            Message::EventEditorCustomReminderEditToggled(editing) => {
                if let Some(state) = self.editing_event.as_mut() {
                    state.custom_reminder_editing = editing;
                }
            }
            Message::EventEditorAddCustomReminder => {
                if let Some(state) = self.editing_event.as_mut() {
                    if let Ok(minutes) = state.custom_reminder_input.trim().parse::<i64>() {
                        if minutes >= 0 && !state.reminders.contains(&minutes) {
                            state.reminders.push(minutes);
                            state.custom_reminder_input.clear();
                            state.custom_reminder_editing = false;
                            self.reminders_model = build_reminders_model(&state.reminders);
                        }
                    }
                }
            }
            Message::CheckReminders => {
                let storage = self.storage.clone();
                let already_fired = self.fired_reminders.clone();
                tasks.push(Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            crate::notifications::check_reminders(&storage, &already_fired)
                        })
                        .await
                        .unwrap_or_default()
                    },
                    |fired| cosmic::action::app(Message::RemindersFired(fired)),
                ));
            }
            Message::RemindersFired(fired) => {
                self.fired_reminders.extend(fired);
            }
            Message::SelectDate(date) => {
                self.calendar.set_date(date);
            }
            Message::SelectDay(idx) => {
                let days = self.calendar.days();
                let day = days.index(idx);
                if let Ok(date) = self.calendar.selected_date.replace_day(*day) {
                    self.calendar.set_date(date);
                } else {
                    tracing::error!("failed to select date");
                }
            }
            Message::SelectMonth(idx) => {
                let months = self.calendar.months();
                let month = months.index(idx);
                if let Ok(date) = self.calendar.selected_date.replace_month(*month) {
                    self.calendar.set_date(date);
                } else {
                    tracing::error!("failed to select date");
                }
            }
            Message::SelectYear(idx) => {
                let years = self.calendar.years();
                let year = years.index(idx);
                if let Ok(date) = self.calendar.selected_date.replace_year(*year) {
                    self.calendar.set_date(date);
                } else {
                    tracing::error!("failed to select date");
                }
            }
            Message::NavigateToday => self.calendar.set_today(),
            Message::NavigateNextDay => {
                if let Err(err) = self.calendar.next_day() {
                    tracing::error!("failed to navigate to next day: {err}");
                }
            }
            Message::NavigatePreviousDay => {
                if let Err(err) = self.calendar.previous_day() {
                    tracing::error!("failed to navigate to previous day: {err}");
                }
            }
            Message::NavigateNextMonth => {
                if let Err(err) = self.calendar.next_month() {
                    tracing::error!("failed to navigate to next month: {err}");
                }
            }
            Message::NavigatePreviousMonth => {
                if let Err(err) = self.calendar.previous_month() {
                    tracing::error!("failed to navigate to previous month: {err}");
                }
            }
            Message::NavigateNextYear => {
                if let Err(err) = self.calendar.next_year() {
                    tracing::error!("failed to navigate to next year: {err}");
                }
            }
            Message::NavigatePreviousYear => {
                if let Err(err) = self.calendar.previous_year() {
                    tracing::error!("failed to navigate to previous year: {err}");
                }
            }
        }
        Task::batch(tasks)
    }

    /// Called when a nav item is selected.
    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<cosmic::Action<Self::Message>> {
        // Clicking a calendar entry toggles its visibility rather than
        // navigating to a page.
        if let Some(calendar_id) = self.nav.data::<CalendarId>(id).copied() {
            let visible = self
                .storage
                .lock()
                .unwrap()
                .get_calendar(calendar_id)
                .map(|meta| !meta.visible)
                .unwrap_or(true);
            return cosmic::task::message(Message::ToggleCalendarVisibility(calendar_id, visible));
        }

        // Activate the page in the model.
        self.nav.activate(id);

        self.update_title()
    }
}

impl AppModel {
    /// The about page for this app.
    pub fn about<'a>(&'a self) -> Element<'a, Message> {
        let cosmic_theme::Spacing { space_xxs, .. } = theme::active().cosmic().spacing;

        let icon = widget::svg(widget::svg::Handle::from_memory(APP_ICON));

        let title = widget::text::title3(fl!("app-title"));

        let hash = env!("VERGEN_GIT_SHA");
        let short_hash: String = hash.chars().take(7).collect();
        let date = env!("VERGEN_GIT_COMMIT_DATE");

        let link = widget::button::link(REPOSITORY)
            .on_press(Message::OpenRepositoryUrl)
            .padding(0);

        widget::column(Vec::new())
            .push(icon)
            .push(title)
            .push(link)
            .push(
                widget::button::link(fl!(
                    "git-description",
                    hash = short_hash.as_str(),
                    date = date
                ))
                .on_press(Message::LaunchUrl(format!("{REPOSITORY}/commits/{hash}")))
                .padding(0),
            )
            .align_x(Alignment::Center)
            .spacing(space_xxs)
            .into()
    }

    /// The event creation/editing form for the context drawer.
    pub fn event_editor_view<'a>(&'a self) -> Element<'a, Message> {
        match &self.editing_event {
            Some(state) => {
                let storage = self.storage.lock().unwrap();
                let mut calendars = storage.list_calendars();
                calendars.sort_by(|a, b| a.name.cmp(&b.name));
                crate::components::event_editor::view(state, &calendars, &self.reminders_model)
            }
            None => widget::text::body("").into(),
        }
    }

    /// Tears down and rebuilds the "Calendars" section of the nav bar from
    /// current storage state. `segmented_button::Model` has no in-place
    /// update API, so every calendar CRUD change calls this to resync.
    pub fn refresh_calendar_nav(&mut self) {
        for id in self.calendar_nav_ids.drain(..) {
            self.nav.remove(id);
        }

        let calendars: Vec<(CalendarId, String, bool)> = {
            let storage = self.storage.lock().unwrap();
            let mut metas = storage.list_calendars();
            metas.sort_by(|a, b| a.name.cmp(&b.name));
            metas
                .into_iter()
                .map(|meta| (meta.id, meta.name.clone(), meta.visible))
                .collect()
        };

        if calendars.is_empty() {
            return;
        }

        let mut new_ids = Vec::with_capacity(calendars.len() + 1);

        self.nav
            .insert()
            .text(fl!("calendars"))
            .with_id(|id| new_ids.push(id));

        for (id, name, visible) in calendars {
            let icon = if visible {
                "checkbox-checked-symbolic"
            } else {
                "checkbox-symbolic"
            };
            self.nav
                .insert()
                .indent(1)
                .text(name)
                .icon(widget::icon::from_name(icon))
                .data(id)
                .with_id(|entity| new_ids.push(entity));
        }

        self.calendar_nav_ids = new_ids;
    }

    /// The calendar creation/rename form for the context drawer.
    pub fn calendar_manager_view<'a>(&'a self) -> Element<'a, Message> {
        crate::components::calendar_manager::view(&self.calendar_manager)
    }

    /// The list of events matching `self.search_query`, shown in place of
    /// the month/week/day grids while a search is active.
    fn search_results_view<'a>(&'a self) -> Element<'a, Message> {
        let storage = self.storage.lock().unwrap();
        let results = storage.search(&self.search_query);

        if results.is_empty() {
            return widget::text::body(fl!("no-results"))
                .apply(widget::container)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Horizontal::Center)
                .align_y(Vertical::Center)
                .into();
        }

        let mut list = widget::column(Vec::new())
            .spacing(spacing().space_xs)
            .width(Length::Fill);

        for event in results {
            let id = event.id;
            let start = event.start();
            let label = format!("{} — {}", start.date(), event.summary);
            list = list.push(
                widget::button::text(label)
                    .width(Length::Fill)
                    .class(cosmic::style::Button::Standard)
                    .on_press(Message::SelectSearchResult(id, start)),
            );
        }

        widget::scrollable(list).width(Length::Fill).height(Length::Fill).into()
    }

    /// Updates the header and window titles.
    pub fn update_title(&mut self) -> Task<cosmic::Action<Message>> {
        let mut window_title = fl!("app-title");

        if let Some(page) = self.nav.text(self.nav.active()) {
            window_title.push_str(" — ");
            window_title.push_str(page);
        }

        if let Some(id) = self.core.main_window_id() {
            self.set_window_title(window_title, id)
        } else {
            Task::none()
        }
    }
}

/// The tab to display in the application.
pub enum Tab {
    Month,
    Week,
    Day,
}

/// The context page to display in the context drawer.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum ContextPage {
    #[default]
    About,
    CalendarManager,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
    About,
    ImportIcs,
    ExportIcs,
}

impl menu::action::MenuAction for MenuAction {
    type Message = Message;

    fn message(&self) -> Self::Message {
        match self {
            MenuAction::About => Message::ToggleContextPage(ContextPage::About),
            MenuAction::ImportIcs => Message::ImportIcs,
            MenuAction::ExportIcs => Message::ExportIcs,
        }
    }
}

/// Actions offered by the nav bar's per-item context menu (right-click),
/// covering calendar creation/rename/delete now that calendar management is
/// merged into the nav bar instead of a separate list drawer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavMenuAction {
    NewCalendar,
    RenameCalendar(CalendarId),
    DeleteCalendar(CalendarId),
}

impl menu::action::MenuAction for NavMenuAction {
    type Message = cosmic::Action<Message>;

    fn message(&self) -> Self::Message {
        match self {
            NavMenuAction::NewCalendar => cosmic::Action::App(Message::OpenNewCalendarForm),
            NavMenuAction::RenameCalendar(id) => {
                cosmic::Action::App(Message::StartEditCalendar(*id))
            }
            NavMenuAction::DeleteCalendar(id) => cosmic::Action::App(Message::DeleteCalendar(*id)),
        }
    }
}

#[cfg(test)]
mod recurrence_rule_tests {
    use super::{RecurrenceEnd, RecurrenceFreq, RecurrenceRule};

    #[test]
    fn round_trips_weekly_interval_by_weekday_count() {
        let mut by_weekday = [false; 7];
        by_weekday[1] = true; // Tuesday
        by_weekday[3] = true; // Thursday

        let rule = RecurrenceRule {
            freq: RecurrenceFreq::Weekly,
            interval: 2,
            by_weekday,
            end: RecurrenceEnd::Count(10),
        };

        let rrule_string = rule.to_rrule_string().expect("weekly rule has an RRULE");
        assert_eq!(rrule_string, "FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,TH;COUNT=10");

        let round_tripped = RecurrenceRule::from_rrule_string(Some(&rrule_string));
        assert_eq!(round_tripped.freq, RecurrenceFreq::Weekly);
        assert_eq!(round_tripped.interval, 2);
        assert_eq!(round_tripped.by_weekday, by_weekday);
        assert_eq!(round_tripped.end, RecurrenceEnd::Count(10));
    }

    #[test]
    fn round_trips_monthly_until() {
        let until = time::Date::from_calendar_date(2026, time::Month::March, 15).unwrap();
        let rule = RecurrenceRule {
            freq: RecurrenceFreq::Monthly,
            interval: 1,
            by_weekday: [false; 7],
            end: RecurrenceEnd::Until(until),
        };

        let rrule_string = rule.to_rrule_string().expect("monthly rule has an RRULE");
        assert_eq!(rrule_string, "FREQ=MONTHLY;UNTIL=20260315T235959Z");

        let round_tripped = RecurrenceRule::from_rrule_string(Some(&rrule_string));
        assert_eq!(round_tripped.freq, RecurrenceFreq::Monthly);
        assert_eq!(round_tripped.interval, 1);
        assert_eq!(round_tripped.end, RecurrenceEnd::Until(until));
    }

    #[test]
    fn none_frequency_has_no_rrule() {
        assert_eq!(RecurrenceRule::NONE.to_rrule_string(), None);
        assert_eq!(
            RecurrenceRule::from_rrule_string(None).freq,
            RecurrenceFreq::None
        );
    }
}
