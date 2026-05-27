use serde::{Deserialize, Serialize};
use std::fs;
use time::OffsetDateTime;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct State {
    pub last_update_check: Option<OffsetDateTime>,
    pub current_version: Option<String>,
    pub last_known_latest_version: Option<String>,
}

impl State {
    pub fn load() -> anyhow::Result<Self> {
        let path = crate::paths::state_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&path)?;
        let state: State = serde_json::from_str(&content)?;
        Ok(state)
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = crate::paths::state_path();
        fs::create_dir_all(path.parent().unwrap())?;
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }
}
