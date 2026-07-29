use cosmic::{iced::Length, theme::spacing, widget, Element};

use crate::app::{
    event_editor_months, EventEditorState, Message, RecurrenceEnd, RecurrenceFreq,
    EVENT_EDITOR_MINUTES, EVENT_EDITOR_YEARS, REMINDER_PRESETS_MINUTES,
};
use crate::fl;
use crate::models::{CalendarKey, LocalCalendarMeta};

const WEEKDAY_LABELS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

fn reminder_label(minutes: i64) -> String {
    if minutes == 0 {
        fl!("reminder-at-start")
    } else if minutes % 60 == 0 {
        format!("{}h", minutes / 60)
    } else {
        format!("{minutes}m")
    }
}

fn reminders_row<'a>(state: &'a EventEditorState) -> Element<'a, Message> {
    let mut row = widget::row(Vec::new()).spacing(spacing().space_xxs);
    for minutes in REMINDER_PRESETS_MINUTES {
        let on = state.reminders.contains(&minutes);
        row = row.push(
            widget::button::custom(widget::text::body(reminder_label(minutes)))
                .class(if on {
                    cosmic::style::Button::Suggested
                } else {
                    cosmic::style::Button::Standard
                })
                .on_press(Message::EventEditorReminderToggled(minutes, !on)),
        );
    }
    row.into()
}

fn hour_labels() -> Vec<String> {
    (0..24).map(|h| format!("{h:02}:00")).collect()
}

fn minute_labels() -> Vec<String> {
    EVENT_EDITOR_MINUTES.iter().map(|m| format!(":{m:02}")).collect()
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

fn recurrence_form<'a>(state: &'a EventEditorState) -> Element<'a, Message> {
    let recurrence_labels: Vec<String> = RecurrenceFreq::ALL
        .iter()
        .map(|freq| freq.label().to_string())
        .collect();
    let recurrence_selected = RecurrenceFreq::ALL
        .iter()
        .position(|freq| *freq == state.recurrence.freq);

    let mut form = widget::column(Vec::new())
        .spacing(spacing().space_xs)
        .push(widget::text::body(fl!("recurrence")))
        .push(widget::dropdown(
            recurrence_labels,
            recurrence_selected,
            Message::EventEditorRecurrenceChanged,
        ));

    if state.recurrence.freq == RecurrenceFreq::None {
        return form.into();
    }

    let interval_labels: Vec<String> = (1..=30).map(|n| n.to_string()).collect();
    let interval_selected = Some((state.recurrence.interval.max(1) - 1) as usize);
    form = form.push(
        widget::row(Vec::new())
            .spacing(spacing().space_xs)
            .align_y(cosmic::iced::alignment::Vertical::Center)
            .push(widget::text::body(fl!("every")))
            .push(widget::dropdown(
                interval_labels,
                interval_selected,
                Message::EventEditorRecurrenceIntervalChanged,
            )),
    );

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
        form = form
            .push(widget::text::body(fl!("on-days")))
            .push(weekdays);
    }

    let end_labels: Vec<String> = vec![fl!("end-never"), fl!("end-until"), fl!("end-count")];
    let end_selected = Some(match state.recurrence.end {
        RecurrenceEnd::Never => 0,
        RecurrenceEnd::Until(_) => 1,
        RecurrenceEnd::Count(_) => 2,
    });
    form = form
        .push(widget::text::body(fl!("recurrence-ends")))
        .push(widget::dropdown(
            end_labels,
            end_selected,
            Message::EventEditorRecurrenceEndChanged,
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

            form = form.push(
                widget::row(Vec::new())
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
                    )),
            );
        }
        RecurrenceEnd::Count(count) => {
            let count_labels: Vec<String> = (1..=30).map(|n| n.to_string()).collect();
            let count_selected = Some((count.max(1) - 1) as usize);
            form = form.push(
                widget::row(Vec::new())
                    .spacing(spacing().space_xs)
                    .align_y(cosmic::iced::alignment::Vertical::Center)
                    .push(widget::dropdown(
                        count_labels,
                        count_selected,
                        Message::EventEditorRecurrenceCountChanged,
                    ))
                    .push(widget::text::body(fl!("occurrences"))),
            );
        }
        RecurrenceEnd::Never => {}
    }

    form.into()
}

pub fn view<'a>(
    state: &'a EventEditorState,
    calendars: &[&LocalCalendarMeta],
) -> Element<'a, Message> {
    let calendar_labels: Vec<String> = calendars.iter().map(|meta| meta.name.clone()).collect();
    let calendar_selected = calendars.iter().position(|meta| {
        matches!(&state.calendar, CalendarKey::Local(id) if *id == meta.id)
    });

    let mut content = widget::column(Vec::new())
        .spacing(spacing().space_s)
        .width(Length::Fill)
        .push(widget::text::body(fl!("calendars")))
        .push(widget::dropdown(
            calendar_labels,
            calendar_selected,
            Message::EventEditorCalendarChanged,
        ))
        .push(widget::text::body(fl!("summary")))
        .push(
            widget::text_input("", &state.summary)
                .on_input(Message::EventEditorSummaryChanged)
                .width(Length::Fill),
        )
        .push(widget::text::body(fl!("description")))
        .push(
            widget::text_input("", &state.description)
                .on_input(Message::EventEditorDescriptionChanged)
                .width(Length::Fill),
        )
        .push(widget::text::body(fl!("location")))
        .push(
            widget::text_input("", &state.location)
                .on_input(Message::EventEditorLocationChanged)
                .width(Length::Fill),
        )
        .push(
            widget::row(Vec::new())
                .spacing(spacing().space_xs)
                .push(widget::toggler(state.all_day).on_toggle(Message::EventEditorAllDayToggled))
                .push(widget::text::body(fl!("all-day"))),
        )
        .push(
            widget::row(Vec::new())
                .spacing(spacing().space_xs)
                .push(widget::toggler(state.is_task).on_toggle(Message::EventEditorTaskToggled))
                .push(widget::text::body(fl!("task"))),
        );

    content = content
        .push(widget::text::body(fl!("date")))
        .push(date_row(state));

    if !state.all_day {
        content = content
            .push(widget::text::body(fl!("starts")))
            .push(time_row(
                state.start.hour(),
                state.start.minute(),
                Message::EventEditorStartHourChanged,
                Message::EventEditorStartMinuteChanged,
            ))
            .push(widget::text::body(fl!("ends")))
            .push(time_row(
                state.end.hour(),
                state.end.minute(),
                Message::EventEditorEndHourChanged,
                Message::EventEditorEndMinuteChanged,
            ));
    }

    content = content.push(recurrence_form(state));

    content = content
        .push(widget::text::body(fl!("reminders")))
        .push(reminders_row(state));

    let mut actions = widget::row(Vec::new())
        .spacing(spacing().space_xs)
        .push(widget::button::suggested(fl!("save")).on_press(Message::SaveEvent))
        .push(widget::button::standard(fl!("cancel")).on_press(Message::CloseEventEditor));

    if let Some(event_id) = state.event_id {
        if state.recurrence.freq != RecurrenceFreq::None {
            actions = actions
                .push(
                    widget::button::destructive(fl!("delete-occurrence"))
                        .on_press(Message::DeleteOccurrence(event_id, state.occurrence_start)),
                )
                .push(
                    widget::button::destructive(fl!("delete-series"))
                        .on_press(Message::DeleteEvent(event_id, state.calendar.clone())),
                );
        } else {
            actions = actions.push(
                widget::button::destructive(fl!("delete"))
                    .on_press(Message::DeleteEvent(event_id, state.calendar.clone())),
            );
        }
    }

    content.push(actions).into()
}
