use std::collections::HashMap;
use std::ops::Range;

use time::OffsetDateTime;
use tracing::warn;

use crate::models::{
    CalendarId, CalendarKey, Event, EventId, LocalCalendarMeta, CURRENT_CALENDAR_SCHEMA_VERSION,
    CURRENT_EVENT_SCHEMA_VERSION,
};
use super::{atomic, index::EventIndex, migration, paths};
use crate::Result;

/// The local, file-backed store for calendars and events created in this
/// app. Provider-fetched calendars (`crate::models::Calendar`) are not
/// persisted here; they're refetched from the network as today.
pub struct LocalStorage {
    index: EventIndex,
    calendars: HashMap<CalendarId, LocalCalendarMeta>,
}

impl LocalStorage {
    /// An empty store, used as a fallback when [`Self::load`] fails (e.g. the
    /// data directory is inaccessible), mirroring how a corrupt app config
    /// falls back to `Config::default()` today.
    pub fn empty() -> Self {
        Self {
            index: EventIndex::default(),
            calendars: HashMap::new(),
        }
    }

    /// Loads all calendars and events from disk, skipping (and logging) any
    /// individual file that fails to parse rather than failing the whole
    /// load — a single corrupt event shouldn't take down the app.
    pub fn load() -> Result<Self> {
        let mut calendars = HashMap::new();
        for entry in read_dir_json(&paths::calendars_dir())? {
            match load_calendar_file(&entry) {
                Ok(meta) => {
                    calendars.insert(meta.id, meta);
                }
                Err(err) => warn!("failed to load calendar {entry:?}: {err}"),
            }
        }

        let mut index = EventIndex::default();
        let events_root = paths::data_dir().join("events");
        if events_root.is_dir() {
            let calendar_dirs = std::fs::read_dir(&events_root)?;
            for calendar_dir in calendar_dirs.flatten() {
                let path = calendar_dir.path();
                if !path.is_dir() {
                    continue;
                }
                for entry in read_dir_json(&path)? {
                    match load_event_file(&entry) {
                        Ok(event) => index.upsert(event),
                        Err(err) => warn!("failed to load event {entry:?}: {err}"),
                    }
                }
            }
        }

        Ok(Self { index, calendars })
    }

    // -- Calendars --------------------------------------------------------

    pub fn list_calendars(&self) -> Vec<&LocalCalendarMeta> {
        self.calendars.values().collect()
    }

    pub fn get_calendar(&self, id: CalendarId) -> Option<&LocalCalendarMeta> {
        self.calendars.get(&id)
    }

    pub fn create_calendar(&mut self, meta: LocalCalendarMeta) -> Result<()> {
        let path = paths::calendar_path(&meta.id);
        let wrapped = migration::wrap(CURRENT_CALENDAR_SCHEMA_VERSION, &meta)?;
        atomic::write_json_atomic(&path, &wrapped)?;
        self.calendars.insert(meta.id, meta);
        Ok(())
    }

    pub fn update_calendar(&mut self, meta: LocalCalendarMeta) -> Result<()> {
        self.create_calendar(meta)
    }

    pub fn delete_calendar(&mut self, id: CalendarId) -> Result<()> {
        let events_dir = paths::events_dir(&id);
        if events_dir.is_dir() {
            std::fs::remove_dir_all(&events_dir)?;
        }
        let calendar_path = paths::calendar_path(&id);
        if calendar_path.is_file() {
            std::fs::remove_file(&calendar_path)?;
        }

        let event_ids: Vec<EventId> = self
            .index
            .events_for_calendar(&CalendarKey::Local(id))
            .into_iter()
            .map(|event| event.id)
            .collect();
        for event_id in event_ids {
            self.index.remove(event_id);
        }

        self.calendars.remove(&id);
        Ok(())
    }

    // -- Events -------------------------------------------------------------

    pub fn get_event(&self, id: EventId) -> Option<&Event> {
        self.index.get(id)
    }

    pub fn events_in_range(&self, range: Range<OffsetDateTime>) -> Vec<&Event> {
        self.index.events_in_range(range)
    }

    /// All events currently in storage, regardless of date — used for ICS
    /// export.
    pub fn all_events(&self) -> Vec<&Event> {
        self.index.all().collect()
    }

    pub fn search(&self, query: &str) -> Vec<&Event> {
        self.index.search(query)
    }

    pub fn create_event(&mut self, event: Event) -> Result<()> {
        self.write_event(&event)?;
        self.index.upsert(event);
        Ok(())
    }

    pub fn update_event(&mut self, event: Event) -> Result<()> {
        self.write_event(&event)?;
        self.index.upsert(event);
        Ok(())
    }

    pub fn delete_event(&mut self, calendar: &CalendarKey, id: EventId) -> Result<()> {
        if let CalendarKey::Local(calendar_id) = calendar {
            let path = paths::event_path(calendar_id, &id);
            if path.is_file() {
                std::fs::remove_file(&path)?;
            }
        }
        self.index.remove(id);
        Ok(())
    }

    fn write_event(&self, event: &Event) -> Result<()> {
        let CalendarKey::Local(calendar_id) = &event.calendar else {
            // Provider-backed events aren't persisted locally yet (phase 5).
            return Ok(());
        };
        let path = paths::event_path(calendar_id, &event.id);
        let wrapped = migration::wrap(CURRENT_EVENT_SCHEMA_VERSION, event)?;
        atomic::write_json_atomic(&path, &wrapped)
    }
}

fn read_dir_json(dir: &std::path::Path) -> Result<Vec<std::path::PathBuf>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            paths.push(path);
        }
    }
    Ok(paths)
}

fn load_calendar_file(path: &std::path::Path) -> Result<LocalCalendarMeta> {
    let bytes = std::fs::read(path)?;
    let raw: serde_json::Value = serde_json::from_slice(&bytes)?;
    migration::unwrap_current(raw, CURRENT_CALENDAR_SCHEMA_VERSION, "LocalCalendarMeta")
}

fn load_event_file(path: &std::path::Path) -> Result<Event> {
    let bytes = std::fs::read(path)?;
    let raw: serde_json::Value = serde_json::from_slice(&bytes)?;
    migration::unwrap_current(raw, CURRENT_EVENT_SCHEMA_VERSION, "Event")
}
