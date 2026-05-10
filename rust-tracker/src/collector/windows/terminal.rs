use std::fs;

#[cfg(target_os = "windows")]
pub fn read_ps_history() -> Vec<String> {
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    let path = format!("{}\\Microsoft\\Windows\\PowerShell\\PSReadLine\\ConsoleHost_history.txt", appdata);

    let contents = fs::read_to_string(path).unwrap_or_default();

    contents
        .lines()
        .map(|s| s.to_string())
        .collect()
}

#[cfg(not(target_os = "windows"))]
pub fn read_ps_history() -> Vec<String> {
    Vec::new()
}
