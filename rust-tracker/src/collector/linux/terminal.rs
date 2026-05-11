use std::fs;

pub fn get_latest_command() -> Option<String> {

    let home =
        std::env::var("HOME").ok()?;

    let path =
        format!("{}/.bash_history", home);

    let contents =
        fs::read_to_string(path).ok()?;

    contents
        .lines()
        .last()
        .map(|s| s.to_string())
}