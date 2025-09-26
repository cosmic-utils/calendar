use cosmic::{
    iced::{Color, Length},
    theme::spacing,
    widget, Apply, Element,
};
use time::{OffsetDateTime, Weekday};

pub fn weekday_header<'a>(day: &'static str) -> Element<'a, crate::app::Message> {
    widget::text::body(day)
        .apply(widget::container)
        .width(Length::Fill)
        .center_x(Length::Fill)
        .padding(spacing().space_xxxs)
        .into()
}

pub fn month_grid<'a>(
    selected_date: &OffsetDateTime,
    current_date: &OffsetDateTime,
) -> Element<'a, crate::app::Message> {
    let first_of_month = selected_date.replace_day(1).unwrap();

    let first_weekday = first_of_month.weekday();
    let days_from_prev_month = match first_weekday {
        Weekday::Sunday => 0,
        Weekday::Monday => 1,
        Weekday::Tuesday => 2,
        Weekday::Wednesday => 3,
        Weekday::Thursday => 4,
        Weekday::Friday => 5,
        Weekday::Saturday => 6,
    };

    let start_date = first_of_month
        .checked_sub(time::Duration::days(days_from_prev_month as i64))
        .unwrap();

    let mut calendar_column = widget::column().spacing(spacing().space_xxs);

    for week in 0..6 {
        let mut week_row = widget::row().spacing(spacing().space_xxs);

        for day in 0..7 {
            let current_day_offset = week * 7 + day;
            let display_date = start_date
                .checked_add(time::Duration::days(current_day_offset as i64))
                .unwrap();

            let is_current_month = display_date.month() == selected_date.month();
            let is_today = display_date.date() == current_date.date();
            let is_selected = display_date.date() == selected_date.date();

            let day_button = day_button(
                display_date.day(),
                is_current_month,
                is_today,
                is_selected,
                display_date,
            );
            week_row = week_row.push(day_button);
        }

        calendar_column = calendar_column.push(week_row);
    }

    calendar_column.into()
}

fn day_button<'a>(
    day: u8,
    is_current_month: bool,
    is_today: bool,
    is_selected: bool,
    date: OffsetDateTime,
) -> impl Into<Element<'a, crate::app::Message>> {
    let mut day_text = widget::text::body(day.to_string());

    if !is_current_month {
        day_text = day_text.class(cosmic::style::Text::Color(Color::from_rgb(0.5, 0.5, 0.5)));
    }

    let mut day_button = widget::button::custom(day_text)
        .width(Length::Fill)
        .height(Length::Fill)
        .class(cosmic::style::Button::MenuFolder)
        .on_press(crate::app::Message::SelectDate(
            date.replace_day(day).unwrap(),
        ));

    if is_today {
        day_button = day_button.class(cosmic::style::Button::Suggested);
    } else if is_selected {
        day_button = day_button.class(cosmic::style::Button::Standard);
    }

    day_button
}
