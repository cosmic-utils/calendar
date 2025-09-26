use cosmic::{
    iced::{alignment::Horizontal, Length},
    theme::spacing,
    widget, Apply, Element,
};
use time::OffsetDateTime;

pub fn week_grid<'a>(week_start: &OffsetDateTime) -> impl Into<Element<'a, crate::app::Message>> {
    let mut main_column = widget::column().spacing(0);

    for hour in 0..24 {
        main_column = main_column.push(widget::divider::horizontal::default());

        let row = hour_row(hour, false, week_start);
        main_column = main_column.push(row);

        main_column = main_column.push(widget::divider::horizontal::light());

        let half_hour_row = hour_row(hour, true, week_start);
        main_column = main_column.push(half_hour_row);
    }

    main_column
}

fn hour_row<'a>(
    hour: u8,
    is_half_hour: bool,
    week_start: &OffsetDateTime,
) -> impl Into<Element<'a, crate::app::Message>> {
    let mut row = widget::row().height(60);

    let time_label = if is_half_hour {
        String::new()
    } else {
        format!("{:02}:00", hour)
    };

    let time_container =
        widget::container(widget::text::body(time_label).align_x(Horizontal::Right))
            .width(30)
            .height(30)
            .align_top(Length::Fill)
            .center_x(Length::Fill)
            .padding([spacing().space_xxxs, 0, 0, 0]);

    row = row.push(time_container);
    row = row.push(widget::divider::vertical::default());

    for day in 0..7 {
        let day_date = week_start.checked_add(time::Duration::days(day)).unwrap();

        let cell = time_cell(hour, is_half_hour, day_date);
        row = row.push(cell);
        row = row.push(widget::divider::vertical::default());
    }

    row
}

fn time_cell<'a>(
    hour: u8,
    is_half_hour: bool,
    date: OffsetDateTime,
) -> impl Into<Element<'a, crate::app::Message>> {
    let minute = if is_half_hour { 30 } else { 0 };
    let cell_time = date
        .replace_hour(hour)
        .and_then(|d| d.replace_minute(minute))
        .and_then(|d| d.replace_second(0))
        .unwrap_or(date);

    let container = widget::container(widget::text(""))
        .width(Length::Fill)
        .height(30);

    widget::button::custom(container)
        .width(Length::Fill)
        .height(60)
        .class(cosmic::style::Button::Text)
        .on_press(crate::app::Message::AddEvent(cell_time))
}

pub fn day_header<'a>(
    day_name: &'static str,
    day_number: u8,
    is_today: bool,
    date: OffsetDateTime,
) -> impl Into<Element<'a, crate::app::Message>> {
    let header = widget::column()
        .push(
            widget::text::body(day_name)
                .font(if is_today {
                    cosmic::font::bold()
                } else {
                    cosmic::font::default()
                })
                .align_x(Horizontal::Center),
        )
        .push(widget::text::title4(day_number.to_string()).align_x(Horizontal::Center))
        .align_x(Horizontal::Center);

    widget::button::custom(header)
        .width(Length::Fill)
        .class(cosmic::style::Button::Text)
        .on_press(crate::app::Message::SelectDate(date))
        .height(60)
}

pub fn empty_day_header<'a>() -> Element<'a, crate::app::Message> {
    let header = widget::column()
        .push(
            widget::text::body("")
                .width(Length::Fill)
                .align_x(Horizontal::Center),
        )
        .push(
            widget::text::title4("")
                .width(Length::Fill)
                .align_x(Horizontal::Center),
        )
        .padding(spacing().space_xxxs)
        .align_x(Horizontal::Center)
        .apply(widget::container);

    widget::button::custom(header)
        .width(Length::Fill)
        .height(60)
        .class(cosmic::style::Button::Text)
        .into()
}
