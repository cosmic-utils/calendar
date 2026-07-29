use std::path::Path;

use serde::Serialize;

use crate::Result;

/// Writes `value` as pretty JSON to `path`, via a temp-file-then-rename so a
/// crash mid-write can never leave a corrupt/truncated file at `path`.
pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(value)?;
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
