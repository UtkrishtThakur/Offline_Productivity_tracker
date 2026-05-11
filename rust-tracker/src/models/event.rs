use serde::{Serialize, Deserialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct Event {

    pub timestamp: String,

    pub event_type: String,

    pub source: String,

    pub app: Option<String>,

    pub title: Option<String>,

    pub workspace: Option<i64>,

    pub duration_sec: Option<u64>,

    pub data: Value,
}