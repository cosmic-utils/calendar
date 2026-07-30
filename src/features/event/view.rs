use cosmic::widget::segmented_button::{self, MultiSelect};
use cosmic::{iced::Length, theme::spacing, widget, Element};

use crate::app::Message;
use crate::features::event::recurrence::{RecurrenceEnd, RecurrenceFreq};
use crate::features::event::state::{
    event_editor_months, EventEditorState, EVENT_EDITOR_MINUTES, EVENT_EDITOR_YEARS,
};
use crate::fl;
use crate::models::{CalendarKey, LocalCalendarMeta};

const WEEKDAY_LABELS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// The reminder presets as a multi-select segmented button: several presets
/// can be active for the same event (e.g. both "10m" and "1h"), so this uses
/// `segmented_button::MultiSelect` rather than the tab-style `SingleSelect`
/// used elsewhere in the app. Unlike the rest of this file's widgets, the
/// model backing this control is a persistent `AppModel` field (see
/// `app::build_reminders_model`) rather than rebuilt on every render — a
/// freshly built model would be dropped at the end of this function while
/// the returned `Element` still needed to borrow it.
///
/// `width(Shrink)` matters here: the default `Fill` divides the available
/// width evenly across however many chips exist, so adding custom reminders
/// would squeeze every chip smaller instead of the row scrolling. `Shrink`
/// sizes each chip to its natural width and switches on the widget's built-in
/// prev/next arrow overflow (the same one browser-style tab bars use) once
/// they no longer all fit.
fn reminders_control<'a>(model: &'a segmented_button::Model<MultiSelect>) -> Element<'a, Message> {
    segmented_button::horizontal(model)
        .on_activate(Message::EventEditorReminderToggled)
        .button_alignment(cosmic::iced::Alignment::Center)
        .into()
}

fn hour_labels() -> Vec<String> {
    (0..24).map(|h| format!("{h:02}:00")).collect()
}

fn minute_labels() -> Vec<String> {
    EVENT_EDITOR_MINUTES
        .iter()
        .map(|m| format!(":{m:02}"))
        .collect()
}

fn date_row<'a>(state: &'a EventEditorState) -> Element<'a, Message> {
    let year_labels: Vec<String> = EVENT_EDITOR_YEARS.iter().map(i32::to_string).collect();
    let year_selected = EVENT_EDITOR_YEARS
        .iter()
        .position(|year| *year == state.start.year());

    let month_labels: Vec<String> = event_editor_months()
        .iter()
        .map(|month| month.to_string())
        .collect();
    let month_selected = event_editor_months()
        .iter()
        .position(|month| *month == state.start.month());

    let day_count = state.start.month().length(state.start.year());
    let day_labels: Vec<String> = (1..=day_count).map(|d| d.to_string()).collect();
    let day_selected = Some((state.start.day() - 1) as usize);

    widget::row(Vec::new())
        .spacing(spacing().space_xs)
        .push(widget::dropdown(
            day_labels,
            day_selected,
            Message::EventEditorStartDayChanged,
        ))
        .push(widget::dropdown(
            month_labels,
            month_selected,
            Message::EventEditorStartMonthChanged,
        ))
        .push(widget::dropdown(
            year_labels,
            year_selected,
            Message::EventEditorStartYearChanged,
        ))
        .into()
}

fn time_row<'a>(
    hour: u8,
    minute: u8,
    on_hour: fn(usize) -> Message,
    on_minute: fn(usize) -> Message,
) -> Element<'a, Message> {
    let hour_selected = Some(hour as usize);
    let minute_selected = EVENT_EDITOR_MINUTES.iter().position(|m| *m == minute);

    widget::row(Vec::new())
        .spacing(spacing().space_xs)
        .push(widget::dropdown(hour_labels(), hour_selected, on_hour))
        .push(widget::dropdown(
            minute_labels(),
            minute_selected,
            on_minute,
        ))
        .into()
}

/// The "Repeat" settings section. Only the frequency row is always shown;
/// interval/weekday/end-condition rows are added as the form deepens, so a
/// one-off event never has to look at recurrence controls at all.
fn recurrence_section<'a>(state: &'a EventEditorState) -> widget::settings::Section<'a, Message> {
    let recurrence_labels: Vec<String> = RecurrenceFreq::ALL
        .iter()
        .map(|freq| freq.label().to_string())
        .collect();
    let recurrence_selected = RecurrenceFreq::ALL
        .iter()
        .position(|freq| *freq == state.recurrence.freq);

    let mut section =
        widget::settings::section()
            .title(fl!("recurrence"))
            .add(widget::settings::item(
                fl!("recurrence"),
                widget::dropdown(
                    recurrence_labels,
                    recurrence_selected,
                    Message::EventEditorRecurrenceChanged,
                ),
            ));

    if state.recurrence.freq == RecurrenceFreq::None {
        return section;
    }

    let interval_labels: Vec<String> = (1..=30).map(|n| n.to_string()).collect();
    let interval_selected = Some((state.recurrence.interval.max(1) - 1) as usize);
    section = section.add(widget::settings::item(
        fl!("every"),
        widget::dropdown(
            interval_labels,
            interval_selected,
            Message::EventEditorRecurrenceIntervalChanged,
        ),
    ));

    if state.recurrence.freq == RecurrenceFreq::Weekly {
        let mut weekdays = widget::row(Vec::new()).spacing(spacing().space_xxs);
        for (idx, label) in WEEKDAY_LABELS.iter().enumerate() {
            let on = state.recurrence.by_weekday[idx];
            weekdays = weekdays.push(
                widget::button::custom(widget::text::body(*label))
                    .class(if on {
                        cosmic::style::Button::Suggested
                    } else {
                        cosmic::style::Button::Standard
                    })
                    .on_press(Message::EventEditorRecurrenceWeekdayToggled(idx, !on)),
            );
        }
        section = section.add(widget::settings::item(fl!("on-days"), weekdays));
    }

    let end_labels: Vec<String> = vec![fl!("end-never"), fl!("end-until"), fl!("end-count")];
    let end_selected = Some(match state.recurrence.end {
        RecurrenceEnd::Never => 0,
        RecurrenceEnd::Until(_) => 1,
        RecurrenceEnd::Count(_) => 2,
    });
    section = section.add(widget::settings::item(
        fl!("recurrence-ends"),
        widget::dropdown(
            end_labels,
            end_selected,
            Message::EventEditorRecurrenceEndChanged,
        ),
    ));

    match state.recurrence.end {
        RecurrenceEnd::Until(until) => {
            let year_labels: Vec<String> = EVENT_EDITOR_YEARS.iter().map(i32::to_string).collect();
            let year_selected = EVENT_EDITOR_YEARS.iter().position(|y| *y == until.year());
            let month_labels: Vec<String> = event_editor_months()
                .iter()
                .map(|m| m.to_string())
                .collect();
            let month_selected = event_editor_months()
                .iter()
                .position(|m| *m == until.month());
            let day_count = until.month().length(until.year());
            let day_labels: Vec<String> = (1..=day_count).map(|d| d.to_string()).collect();
            let day_selected = Some((until.day() - 1) as usize);

            let until_row = widget::row(Vec::new())
                .spacing(spacing().space_xs)
                .push(widget::dropdown(
                    day_labels,
                    day_selected,
                    Message::EventEditorRecurrenceUntilDayChanged,
                ))
                .push(widget::dropdown(
                    month_labels,
                    month_selected,
                    Message::EventEditorRecurrenceUntilMonthChanged,
                ))
                .push(widget::dropdown(
                    year_labels,
                    year_selected,
                    Message::EventEditorRecurrenceUntilYearChanged,
                ));
            section = section.add(widget::settings::item(fl!("end-until"), until_row));
        }
        RecurrenceEnd::Count(count) => {
            let count_labels: Vec<String> = (1..=30).map(|n| n.to_string()).collect();
            let count_selected = Some((count.max(1) - 1) as usize);
            let count_row = widget::row(Vec::new())
                .spacing(spacing().space_xs)
                .align_y(cosmic::iced::alignment::Vertical::Center)
                .push(widget::dropdown(
                    count_labels,
                    count_selected,
                    Message::EventEditorRecurrenceCountChanged,
                ))
                .push(widget::text::body(fl!("occurrences")));
            section = section.add(widget::settings::item(fl!("end-count"), count_row));
        }
        RecurrenceEnd::Never => {}
    }

    section
}

pub fn view<'a>(
    state: &'a EventEditorState,
    calendars: &[&LocalCalendarMeta],
    reminders_model: &'a segmented_button::Model<MultiSelect>,
) -> Element<'a, Message> {
    let calendar_labels: Vec<String> = calendars.iter().map(|meta| meta.name.clone()).collect();
    let calendar_selected = calendars
        .iter()
        .position(|meta| matches!(&state.calendar, CalendarKey::Local(id) if *id == meta.id));

    // "Details": what the event is and where it lives. The two toggles use
    // the settings-item helper so they read as switches in a list, not
    // free-floating checkboxes.
    let details = widget::settings::section()
        .title(fl!("summary"))
        .add(widget::settings::item(
            fl!("calendars"),
            widget::dropdown(
                calendar_labels,
                calendar_selected,
                Message::EventEditorCalendarChanged,
            ),
        ))
        .add(widget::settings::item(
            fl!("summary"),
            widget::text_input("", &state.summary)
                .on_input(Message::EventEditorSummaryChanged)
                .width(Length::Fixed(280.0)),
        ))
        .add(widget::settings::item(
            fl!("description"),
            widget::text_input("", &state.description)
                .on_input(Message::EventEditorDescriptionChanged)
                .width(Length::Fixed(280.0)),
        ))
        .add(widget::settings::item(
            fl!("location"),
            widget::text_input("", &state.location)
                .on_input(Message::EventEditorLocationChanged)
                .width(Length::Fixed(280.0)),
        ))
        .add(
            widget::settings::item::builder(fl!("all-day"))
                .toggler(state.all_day, Message::EventEditorAllDayToggled),
        )
        .add(
            widget::settings::item::builder(fl!("task"))
                .toggler(state.is_task, Message::EventEditorTaskToggled),
        );

    // "Date & time": the date row is always visible; start/end times collapse
    // away entirely for all-day events, since they have no meaning there.
    let mut when = widget::settings::section()
        .title(fl!("date"))
        .add(widget::settings::item(fl!("date"), date_row(state)));

    if !state.all_day {
        when = when
            .add(widget::settings::item(
                fl!("starts"),
                time_row(
                    state.start.hour(),
                    state.start.minute(),
                    Message::EventEditorStartHourChanged,
                    Message::EventEditorStartMinuteChanged,
                ),
            ))
            .add(widget::settings::item(
                fl!("ends"),
                time_row(
                    state.end.hour(),
                    state.end.minute(),
                    Message::EventEditorEndHourChanged,
                    Message::EventEditorEndMinuteChanged,
                ),
            ));
    }

    let custom_reminder_input = widget::editable_input(
        fl!("custom-reminder-placeholder"),
        &state.custom_reminder_input,
        state.custom_reminder_editing,
        Message::EventEditorCustomReminderEditToggled,
    )
    .on_input(Message::EventEditorCustomReminderInputChanged)
    .on_submit(|_| Message::EventEditorAddCustomReminder)
    .width(Length::Fixed(220.0));

    let reminders = widget::settings::section().title(fl!("reminders")).add(
        widget::column(Vec::new())
            .spacing(spacing().space_xxs)
            .push(widget::text::caption(fl!("reminders-caption")))
            .push(reminders_control(reminders_model))
            .push(custom_reminder_input),
    );

    let content = widget::settings::view_column(vec![
        details.into(),
        when.into(),
        recurrence_section(state).into(),
        reminders.into(),
    ]);

    let mut dialog = widget::dialog::dialog()
        .title(if state.event_id.is_some() {
            fl!("edit-event")
        } else {
            fl!("new-event")
        })
        .control(
            widget::scrollable(content)
                .width(Length::Fill)
                .height(Length::Fixed(420.0)),
        )
        .primary_action(widget::button::suggested(fl!("save")).on_press(Message::SaveEvent))
        .secondary_action(
            widget::button::standard(fl!("cancel")).on_press(Message::CloseEventEditor),
        )
        .width(Length::Fixed(640.0));

    if let Some(event_id) = state.event_id {
        let delete_buttons = if state.recurrence.freq != RecurrenceFreq::None {
            widget::row(Vec::new())
                .spacing(spacing().space_xs)
                .push(
                    widget::button::destructive(fl!("delete-occurrence"))
                        .on_press(Message::DeleteOccurrence(event_id, state.occurrence_start)),
                )
                .push(
                    widget::button::destructive(fl!("delete-series"))
                        .on_press(Message::DeleteEvent(event_id, state.calendar.clone())),
                )
        } else {
            widget::row(Vec::new()).push(
                widget::button::destructive(fl!("delete"))
                    .on_press(Message::DeleteEvent(event_id, state.calendar.clone())),
            )
        };
        dialog = dialog.tertiary_action(delete_buttons);
    }

    dialog.into()
}
