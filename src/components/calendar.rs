use cosmic::{iced::Length, theme::spacing, widget, Element};
use time::OffsetDateTime;

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
        widget::container(widget::text("Month view"))
            .center(Length::Fill)
            .padding(spacing().space_xs)
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
}
