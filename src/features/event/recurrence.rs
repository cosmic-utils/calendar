/// The recurrence frequencies offered in the event editor, mapped to/from a
/// plain RFC 5545 `RRULE` string (no custom intervals/exceptions in the UI
/// yet — those can be added to the RRULE string format later without
/// changing the stored [`crate::models::Event::rrule`] shape).
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
/// only representation actually stored on [`crate::models::Event::rrule`] —
/// the rest of the app (`Event::occurrences`) already handles arbitrary
/// valid RRULE strings, so this is purely an editor-side convenience.
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

    /// Parses an existing RRULE string (as read from [`crate::models::Event::rrule`])
    /// back into editable form. Falls back to [`Self::NONE`] on any parse error
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
