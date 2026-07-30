use crate::models::{CalendarKey, Event, EventId};
use crate::Result;
use cosmic::{
    iced::{alignment::Horizontal, Length},
    theme::spacing,
    widget, Element,
};
use std::collections::BTreeMap;
use std::ops::Range;
use time::{Month, OffsetDateTime, Weekday};

mod day;
mod month;
mod week;

/// A single occurrence of an [`Event`] to render on a grid: owns its own
/// data so recurring events can appear more than once (once per occurrence)
/// without juggling per-occurrence borrows against the source `Event`.
#[derive(Debug, Clone)]
pub(crate) struct EventChip {
    pub id: EventId,
    pub calendar: CalendarKey,
    pub summary: String,
    /// The start of this specific occurrence (may differ from the parent
    /// event's own stored start for recurring events).
    pub start: OffsetDateTime,
}

/// Expands `events` into one [`EventChip`] per occurrence that falls within
/// `range`, using [`Event::occurrences`] (a no-op expansion for
/// non-recurring events).
fn expand_occurrences(events: &[Event], range: Range<OffsetDateTime>) -> Vec<EventChip> {
    let mut chips = Vec::new();
    for event in events {
        for start in event.occurrences(range.clone()) {
            chips.push(EventChip {
                id: event.id,
                calendar: event.calendar.clone(),
                summary: event.summary.clone(),
                start,
            });
        }
    }
    chips
}

/// Buckets `chips` by the calendar date they occur on, for the month grid.
fn chips_by_day(chips: Vec<EventChip>) -> BTreeMap<time::Date, Vec<EventChip>> {
    let mut map: BTreeMap<time::Date, Vec<EventChip>> = BTreeMap::new();
    for chip in chips {
        map.entry(chip.start.date()).or_default().push(chip);
    }
    map
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalCalendar {
    pub current_date: OffsetDateTime,
    pub selected_date: OffsetDateTime,
}

impl Default for LocalCalendar {
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

impl LocalCalendar {
    #[allow(unused)]
    pub fn new(current_date: OffsetDateTime) -> Self {
        Self {
            current_date,
            selected_date: current_date,
        }
    }

    pub fn month_view<'a>(
        &'a self,
        events: &[Event],
    ) -> impl Into<Element<'a, crate::app::Message>> {
        let weekday_headers = widget::row::with_children(vec![
            month::weekday_header("Sun"),
            month::weekday_header("Mon"),
            month::weekday_header("Tue"),
            month::weekday_header("Wed"),
            month::weekday_header("Thu"),
            month::weekday_header("Fri"),
            month::weekday_header("Sat"),
        ])
        .spacing(spacing().space_xxs);

        // The grid always displays 6 weeks (42 days); pad generously on
        // both ends so occurrences from the leading/trailing prior/next
        // month days are included too.
        let range_start = self.selected_date.replace_day(1).unwrap() - time::Duration::days(7);
        let range_end = range_start + time::Duration::days(49);
        let chips = chips_by_day(expand_occurrences(events, range_start..range_end));
        let calendar_grid = month::month_grid(&self.selected_date, &self.current_date, &chips);

        widget::column(Vec::new())
            .push(weekday_headers)
            .push(calendar_grid)
            .spacing(spacing().space_xs)
            .align_x(Horizontal::Center)
            .height(Length::Fill)
            .padding([0, 0, spacing().space_xxs, 0])
    }

    pub fn week_view<'a>(
        &'a self,
        events: &[Event],
    ) -> impl Into<Element<'a, crate::app::Message>> {
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

        let mut header_row = widget::row(Vec::new()).padding([0, spacing().space_xs, 0, 0]);

        let day_names = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        for i in -1..7 {
            if i == -1 {
                header_row = header_row.push(week::empty_day_header());
                continue;
            }

            let day_date = week_start.checked_add(time::Duration::days(i)).unwrap();
            let is_today = day_date.date() == self.current_date.date();

            let day_header =
                week::day_header(day_names[i as usize], day_date.day(), is_today, day_date);
            header_row = header_row.push(day_header);
        }

        let range = week_start..(week_start + time::Duration::days(7));
        let chips = expand_occurrences(events, range);
        let time_grid = week::week_grid(&week_start, &chips);

        widget::column(Vec::new())
            .push(header_row)
            .push(
                widget::scrollable(time_grid)
                    .height(Length::Fill)
                    .width(Length::Fill),
            )
            .padding([0, 0, spacing().space_xxs, 0])
    }

    pub fn day_view<'a>(
        &'a self,
        events: &[Event],
    ) -> impl Into<Element<'a, crate::app::Message>> {
        let day_start = self.selected_date.replace_time(time::Time::MIDNIGHT);
        let range = day_start..(day_start + time::Duration::days(1));
        let chips = expand_occurrences(events, range);
        let time_grid = day::single_day_time_grid(&self.selected_date, &chips);

        widget::column(Vec::new())
            .push(widget::scrollable(time_grid).height(Length::Fill))
            .padding([0, 0, spacing().space_xxs, 0])
    }

    pub fn set_today(&mut self) {
        self.selected_date = self.current_date;
    }

    pub fn next_day(&mut self) -> Result<()> {
        let new_date = self
            .selected_date
            .checked_add(time::Duration::days(1))
            .ok_or_else(|| crate::Error::DateCalculation("Failed to calculate next day".into()))?;
        self.selected_date = new_date;
        Ok(())
    }

    pub fn previous_day(&mut self) -> Result<()> {
        let new_date = self
            .selected_date
            .checked_sub(time::Duration::days(1))
            .ok_or_else(|| {
                crate::Error::DateCalculation("Failed to calculate previous day".into())
            })?;
        self.selected_date = new_date;
        Ok(())
    }

    pub fn next_month(&mut self) -> Result<()> {
        let next_month = self.selected_date.month().next();
        let new_date = self.selected_date.replace_month(next_month)?;
        self.selected_date = new_date;
        Ok(())
    }

    pub fn previous_month(&mut self) -> Result<()> {
        let previous_month = self.selected_date.month().previous();
        let new_date = self.selected_date.replace_month(previous_month)?;
        self.selected_date = new_date;
        Ok(())
    }

    pub fn next_year(&mut self) -> Result<()> {
        let next_year = self.selected_date.year();
        let new_date = self.selected_date.replace_year(next_year + 1)?;
        self.selected_date = new_date;
        Ok(())
    }

    pub fn previous_year(&mut self) -> Result<()> {
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

    pub fn days(&self) -> Vec<u8> {
        let mut days = vec![];
        let days_in_month = self.selected_date.month().length(self.selected_date.year());
        for day in 1..=days_in_month {
            days.push(day);
        }
        days
    }
}
