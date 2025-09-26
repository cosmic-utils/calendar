use cosmic::{
    iced::{alignment::Horizontal, Color, Length},
    theme::spacing,
    widget, Apply, Element,
};
use time::{Month, OffsetDateTime, Weekday};

pub struct Calendar {
    pub current_date: OffsetDateTime,
    pub selected_date: OffsetDateTime,
}

impl Default for Calendar {
    fn default() -> Self {
        let current_date = match OffsetDateTime::now_local() {
            Ok(date) => date,
            Err(err) => {
                tracing::error!("Failed to get local time: {}", err);
                panic!("Failed to get local time")
            }
        };
        Self {
            current_date,
            selected_date: current_date,
        }
    }
}

impl Calendar {
    #[allow(unused)]
    pub fn new(current_date: OffsetDateTime) -> Self {
        Self {
            current_date,
            selected_date: current_date,
        }
    }

    pub fn month_view<'a>(&'a self) -> impl Into<Element<'a, crate::app::Message>> {
        let weekday_headers = widget::row::with_children(vec![
            self.weekday_header("Sun"),
            self.weekday_header("Mon"),
            self.weekday_header("Tue"),
            self.weekday_header("Wed"),
            self.weekday_header("Thu"),
            self.weekday_header("Fri"),
            self.weekday_header("Sat"),
        ])
        .spacing(spacing().space_xxs);

        let calendar_grid = self.month_grid();

        widget::column()
            .push(weekday_headers)
            .push(calendar_grid)
            .spacing(spacing().space_xs)
            .padding(spacing().space_xxs)
            .align_x(Horizontal::Center)
            .height(Length::Fill)
    }

    pub fn week_view<'a>(&'a self) -> impl Into<Element<'a, crate::app::Message>> {
        let selected_date = self.selected_date;
        let days_since_sunday = match selected_date.weekday() {
            Weekday::Sunday => 0,
            Weekday::Monday => 1,
            Weekday::Tuesday => 2,
            Weekday::Wednesday => 3,
            Weekday::Thursday => 4,
            Weekday::Friday => 5,
            Weekday::Saturday => 6,
        };

        let week_start = selected_date
            .checked_sub(time::Duration::days(days_since_sunday as i64))
            .unwrap();

        let mut header_row = widget::row().padding([0, spacing().space_xs, 0, 0]);

        let day_names = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        for i in -1..7 {
            if i == -1 {
                header_row = header_row.push(self.empty_day_header());
                continue;
            }

            let day_date = week_start.checked_add(time::Duration::days(i)).unwrap();
            let is_today = day_date.date() == self.current_date.date();

            let day_header =
                self.day_header(day_names[i as usize], day_date.day(), is_today, day_date);
            header_row = header_row.push(day_header);
        }

        let time_grid = self.week_grid(week_start);

        widget::column()
            .push(header_row)
            .push(
                widget::scrollable(time_grid)
                    .spacing(spacing().space_xxxs)
                    .height(Length::Fill)
                    .width(Length::Fill),
            )
            .padding(spacing().space_xs)
    }

    pub fn day_view<'a>(&'a self) -> impl Into<Element<'a, crate::app::Message>> {
        widget::container(widget::text("Day view"))
            .center(Length::Fill)
            .padding(spacing().space_xs)
    }

    fn weekday_header<'a>(&self, day: &'static str) -> Element<'a, crate::app::Message> {
        widget::text::body(day)
            .apply(widget::container)
            .width(Length::Fill)
            .center_x(Length::Fill)
            .padding(spacing().space_xxxs)
            .into()
    }

    fn day_header<'a>(
        &self,
        day_name: &'static str,
        day_number: u8,
        is_today: bool,
        date: OffsetDateTime,
    ) -> Element<'a, crate::app::Message> {
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
            .into()
    }

    fn empty_day_header<'a>(&self) -> Element<'a, crate::app::Message> {
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

    fn month_grid<'a>(&'a self) -> Element<'a, crate::app::Message> {
        let current_date = self.selected_date;
        let first_of_month = current_date.replace_day(1).unwrap();

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

                let is_current_month = display_date.month() == current_date.month();
                let is_today = display_date == self.current_date;
                let is_selected = display_date == self.selected_date;

                let day_button = self.day_button(
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
        &self,
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

    fn week_grid<'a>(&'a self, week_start: OffsetDateTime) -> Element<'a, crate::app::Message> {
        let mut main_column = widget::column().spacing(0);

        for hour in 0..24 {
            main_column = main_column.push(widget::divider::horizontal::default());

            let hour_row = self.hour_row(hour, false, week_start);
            main_column = main_column.push(hour_row);

            main_column = main_column.push(widget::divider::horizontal::light());

            let half_hour_row = self.hour_row(hour, true, week_start);
            main_column = main_column.push(half_hour_row);
        }

        main_column.into()
    }

    fn hour_row<'a>(
        &self,
        hour: u8,
        is_half_hour: bool,
        week_start: OffsetDateTime,
    ) -> Element<'a, crate::app::Message> {
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

            let cell = self.time_cell(hour, is_half_hour, day_date);
            row = row.push(cell);
            row = row.push(widget::divider::vertical::default());
        }

        row.into()
    }

    fn time_cell<'a>(
        &self,
        hour: u8,
        is_half_hour: bool,
        date: OffsetDateTime,
    ) -> Element<'a, crate::app::Message> {
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
            .into()
    }

    pub fn set_today(&mut self) {
        self.selected_date = self.current_date;
    }

    pub fn next_month(&mut self) -> Result<(), crate::Error> {
        let next_month = self.selected_date.month().next();
        let new_date = self.selected_date.replace_month(next_month)?;
        self.selected_date = new_date;
        Ok(())
    }

    pub fn previous_month(&mut self) -> Result<(), crate::Error> {
        let previous_month = self.selected_date.month().previous();
        let new_date = self.selected_date.replace_month(previous_month)?;
        self.selected_date = new_date;
        Ok(())
    }

    pub fn next_year(&mut self) -> Result<(), crate::Error> {
        let next_year = self.selected_date.year();
        let new_date = self.selected_date.replace_year(next_year + 1)?;
        self.selected_date = new_date;
        Ok(())
    }

    pub fn previous_year(&mut self) -> Result<(), crate::Error> {
        let next_year = self.selected_date.year();
        let new_date = self.selected_date.replace_year(next_year - 1)?;
        self.selected_date = new_date;
        Ok(())
    }

    pub fn today(&self) -> bool {
        self.selected_date == self.current_date
    }

    pub fn set_date(&mut self, date: OffsetDateTime) {
        self.selected_date = date;
    }

    pub fn months(&self) -> [Month; 12] {
        [
            Month::January,
            Month::February,
            Month::March,
            Month::April,
            Month::May,
            Month::June,
            Month::July,
            Month::August,
            Month::September,
            Month::October,
            Month::November,
            Month::December,
        ]
    }

    pub fn years(&self) -> Vec<i32> {
        let mut years = vec![];
        for year in 2000..=2099 {
            years.push(year);
        }
        years
    }
}
