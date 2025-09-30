use std::collections::HashMap;

use accounts::models::Provider;
use gcal_rs::CalendarListItem;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::services;

/// A unified Calendar model that works across providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Calendar {
    /// Unique identifier of the calendar (Google: "id", Microsoft: "id").
    pub id: String,
    /// Human-readable name of the calendar (Google: "summary", Microsoft: "name").
    pub name: String,
    /// Optional description (Google: "description", Microsoft: "hexColor" -> could go in metadata).
    pub description: Option<String>,
    /// Timezone associated with the calendar.
    pub timezone: Option<String>,
    /// Background color (Google: "backgroundColor", Microsoft: "color").
    pub color: Option<String>,
    /// Access/role (Google: "accessRole", Microsoft: "canEdit"/"owner").
    pub access_role: String,
    /// Whether this is the primary calendar for the account. pub primary: bool, /// The source provider (Google, Microsoft, etc.)
    pub provider: Provider,
    /// A bag for provider-specific raw fields you may want later. /// Useful if you don’t want to lose information during normalization.
    pub extra: HashMap<String, Value>,
}

impl From<services::microsoft::models::Calendar> for Calendar {
    fn from(mc: services::microsoft::models::Calendar) -> Self {
        let mut extra = HashMap::new();
        extra.insert(
            "groupClassId".to_string(),
            Value::String(mc.group_class_id.clone()),
        );
        extra.insert(
            "changeKey".to_string(),
            Value::String(mc.change_key.clone()),
        );
        extra.insert(
            "isTallyingResponses".to_string(),
            Value::Bool(mc.is_tallying_responses),
        );
        extra.insert("isRemovable".to_string(), Value::Bool(mc.is_removable));
        extra.insert(
            "allowedOnlineMeetingProviders".to_string(),
            Value::Array(
                mc.allowed_online_meeting_providers
                    .into_iter()
                    .map(Value::String)
                    .collect(),
            ),
        );
        extra.insert(
            "defaultOnlineMeetingProvider".to_string(),
            Value::String(mc.default_online_meeting_provider),
        );

        Calendar {
            id: mc.id,
            name: mc.name,
            description: None,
            timezone: None,
            color: if mc.color == "auto" {
                None
            } else {
                Some(mc.color)
            },
            access_role: if mc.can_edit {
                "owner".to_string()
            } else {
                "reader".to_string()
            },
            provider: Provider::Microsoft,
            extra,
        }
    }
}

impl From<CalendarListItem> for Calendar {
    fn from(item: CalendarListItem) -> Self {
        let mut extra = HashMap::new();

        if let Some(location) = &item.location {
            extra.insert("location".to_string(), Value::String(location.clone()));
        }
        if let Some(summary_override) = &item.summary_override {
            extra.insert(
                "summaryOverride".to_string(),
                Value::String(summary_override.clone()),
            );
        }
        if let Some(foreground_color) = &item.foreground_color {
            extra.insert(
                "foregroundColor".to_string(),
                Value::String(foreground_color.clone()),
            );
        }
        if let Some(color_id) = &item.color_id {
            extra.insert("colorId".to_string(), Value::String(color_id.clone()));
        }
        if let Some(conference_properties) = &item.conference_properties {
            extra.insert(
                "conferenceProperties".to_string(),
                serde_json::to_value(conference_properties).unwrap_or(Value::Null),
            );
        }
        extra.insert(
            "deleted".to_string(),
            Value::Bool(item.deleted.unwrap_or(false)),
        );
        extra.insert(
            "hidden".to_string(),
            Value::Bool(item.hidden.unwrap_or(false)),
        );
        extra.insert(
            "selected".to_string(),
            Value::Bool(item.selected.unwrap_or(false)),
        );
        extra.insert(
            "defaultReminders".to_string(),
            Value::Array(
                item.default_reminders
                    .into_iter()
                    .map(|r| serde_json::to_value(r).unwrap_or(Value::Null))
                    .collect(),
            ),
        );

        Calendar {
            id: item.id,
            name: item.summary,
            description: item.description,
            timezone: item.time_zone,
            color: item.background_color,
            access_role: item.access_role.to_string(),
            provider: Provider::Google,
            extra,
        }
    }
}
