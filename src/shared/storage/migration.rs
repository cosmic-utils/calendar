use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

use crate::{Error, Result};

/// On-disk envelope wrapping a versioned payload, so future schema changes
/// can add a migration arm in [`unwrap_current`] instead of breaking old
/// files outright.
#[derive(Serialize, Deserialize)]
struct Envelope {
    version: u32,
    data: Value,
}

pub fn wrap<T: Serialize>(version: u32, data: &T) -> Result<Value> {
    let envelope = Envelope {
        version,
        data: serde_json::to_value(data)?,
    };
    Ok(serde_json::to_value(envelope)?)
}

/// Deserializes `raw` as `T`, requiring it to already be at
/// `current_version`. When a future schema bump happens, add a `match` arm
/// here that migrates older payload shapes forward before deserializing.
pub fn unwrap_current<T: DeserializeOwned>(
    raw: Value,
    current_version: u32,
    type_name: &str,
) -> Result<T> {
    let envelope: Envelope = serde_json::from_value(raw)?;
    if envelope.version != current_version {
        return Err(Error::Storage(format!(
            "unsupported {type_name} schema version {} (expected {current_version})",
            envelope.version
        )));
    }
    Ok(serde_json::from_value(envelope.data)?)
}
