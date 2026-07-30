use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CalendarsResponse {
    #[serde(rename = "@odata.context")]
    pub odata_context: String,
    pub value: Vec<Calendar>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Calendar {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "color")]
    pub color: String,
    #[serde(rename = "hexColor")]
    pub hex_color: String,
    #[serde(rename = "groupClassId")]
    pub group_class_id: String,
    #[serde(rename = "isDefaultCalendar")]
    pub is_default_calendar: bool,
    #[serde(rename = "changeKey")]
    pub change_key: String,
    #[serde(rename = "canShare")]
    pub can_share: bool,
    #[serde(rename = "canViewPrivateItems")]
    pub can_view_private_items: bool,
    #[serde(rename = "canEdit")]
    pub can_edit: bool,
    #[serde(rename = "allowedOnlineMeetingProviders")]
    pub allowed_online_meeting_providers: Vec<String>,
    #[serde(rename = "defaultOnlineMeetingProvider")]
    pub default_online_meeting_provider: String,
    #[serde(rename = "isTallyingResponses")]
    pub is_tallying_responses: bool,
    #[serde(rename = "isRemovable")]
    pub is_removable: bool,
    #[serde(rename = "owner")]
    pub owner: Owner,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Owner {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "address")]
    pub address: String,
}
