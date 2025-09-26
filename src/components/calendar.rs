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
        // Create weekday headers
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

        // Generate the calendar grid
        let calendar_grid = self.generate_month_grid();

        widget::column()
            .push(weekday_headers)
            .push(calendar_grid)
            .spacing(spacing().space_xs)
            .padding(spacing().space_xxs)
            .align_x(Horizontal::Center)
            .height(Length::Fill)
    }

    pub fn week_view<'a>(&'a self) -> impl Into<Element<'a, crate::app::Message>> {
        widget::container(widget::text("Week view"))
            .center(Length::Fill)
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

    fn generate_month_grid<'a>(&'a self) -> Element<'a, crate::app::Message> {
        let current_date = self.selected_date;
        let first_of_month = current_date.replace_day(1).unwrap();

        // Find the first day to display (may be from previous month)
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

        // Create 6 weeks (42 days)
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

                let day_button = self.create_day_button(
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

    fn create_day_button<'a>(
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
