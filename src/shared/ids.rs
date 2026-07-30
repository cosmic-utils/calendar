use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Stable identifier for a locally-created event or task.
///
/// Kept stable across edits (and across a future provider sync) so that
/// [`crate::models::ExternalRef`] can be attached without changing identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EventId(pub Uuid);

impl EventId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

/// Stable identifier for a calendar created locally (not backed by a provider).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CalendarId(pub Uuid);

impl CalendarId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for CalendarId {
    fn default() -> Self {
        Self::new()
    }
}
