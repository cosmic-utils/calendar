use std::path::PathBuf;

use directories::ProjectDirs;

use crate::models::{CalendarId, EventId};

fn project_dirs() -> ProjectDirs {
    ProjectDirs::from("dev", "edfloreshz", "Calendar")
        .expect("failed to determine a home directory for local storage")
}

pub fn data_dir() -> PathBuf {
    project_dirs().data_dir().to_path_buf()
}

pub fn calendars_dir() -> PathBuf {
    data_dir().join("calendars")
}

pub fn calendar_path(calendar_id: &CalendarId) -> PathBuf {
    calendars_dir().join(format!("{}.json", calendar_id.0))
}

pub fn events_dir(calendar_id: &CalendarId) -> PathBuf {
    data_dir().join("events").join(calendar_id.0.to_string())
}

pub fn event_path(calendar_id: &CalendarId, event_id: &EventId) -> PathBuf {
    events_dir(calendar_id).join(format!("{}.json", event_id.0))
}

pub fn index_path() -> PathBuf {
    data_dir().join("index.json")
}
