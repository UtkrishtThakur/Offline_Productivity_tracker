use serde_json::Value;
use std::fs;

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub app: String,
    pub title: String,
    pub workspace: i64,
}

pub fn get_active_window() -> Option<WindowInfo> {

    let home =
        std::env::var("HOME").ok()?;

    let path = format!(
        "{}/.config/gnomectl/activewindow.json",
        home
    );

    let contents =
        fs::read_to_string(path).ok()?;

    let parsed: Value =
        serde_json::from_str(&contents).ok()?;

    let app = parsed["app"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    let title = parsed["title"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    let workspace = parsed["workspace"]
        .as_i64()
        .unwrap_or(-1);

    Some(WindowInfo {
        app,
        title,
        workspace,
    })
}