use cosmic::{
    iced::{alignment::Horizontal, Length},
    theme::spacing,
    widget, Element,
};
use time::OffsetDateTime;

pub fn single_day_time_grid<'a>(
    selected_date: &OffsetDateTime,
) -> impl Into<Element<'a, crate::app::Message>> {
    let mut main_column = widget::column().spacing(0);

    for hour in 0..24 {
        main_column = main_column.push(widget::divider::horizontal::default());

        let hour_row = single_day_hour_row(hour, false, selected_date);
        main_column = main_column.push(hour_row);

        main_column = main_column.push(widget::divider::horizontal::light());

        let half_hour_row = single_day_hour_row(hour, true, selected_date);
        main_column = main_column.push(half_hour_row);
    }

    main_column
}

fn single_day_hour_row<'a>(
    hour: u8,
    is_half_hour: bool,
    selected_date: &OffsetDateTime,
) -> impl Into<Element<'a, crate::app::Message>> {
    let mut row = widget::row();

    let time_label = if is_half_hour {
        format!("{:02}:30", hour)
    } else {
        format!("{:02}:00", hour)
    };

    let time_container =
        widget::container(widget::text::body(time_label).align_x(Horizontal::Right))
            .width(100)
            .height(40)
            .center_y(Length::Fill)
            .padding([0, spacing().space_s, 0, 0]);

    row = row.push(time_container);
    row = row.push(widget::divider::vertical::default());

    let time_cell = single_day_time_cell(hour, is_half_hour, selected_date);
    row = row.push(time_cell);

    row.height(80)
}

fn single_day_time_cell<'a>(
    hour: u8,
    is_half_hour: bool,
    date: &OffsetDateTime,
) -> impl Into<Element<'a, crate::app::Message>> {
    let minute = if is_half_hour { 30 } else { 0 };
    let cell_time = date
        .replace_hour(hour)
        .and_then(|d| d.replace_minute(minute))
        .unwrap_or(date.clone());

    widget::button::text("")
        .width(Length::Fill)
        .height(80)
        .class(cosmic::style::Button::Text)
        .on_press(crate::app::Message::AddEvent(cell_time))
}
