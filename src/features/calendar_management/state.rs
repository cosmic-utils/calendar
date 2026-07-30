use crate::models::CalendarId;

/// State of an in-progress edit to a calendar's name/color, shown inline in
/// the calendar manager.
#[derive(Debug, Clone)]
pub struct CalendarEditState {
    pub id: CalendarId,
    pub name: String,
    pub color: String,
}

/// State backing the calendar management drawer.
#[derive(Debug, Clone, Default)]
pub struct CalendarManagerState {
    pub new_name: String,
    pub editing: Option<CalendarEditState>,
}
