use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Serialize, Deserialize)]
pub struct Metadata {
    pub version: String,
    pub asset_name: String,
    pub download_url: String,
    pub sha256: String,
    pub installed_at: OffsetDateTime,
}
