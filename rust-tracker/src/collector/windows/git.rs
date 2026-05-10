use std::process::Command;

#[cfg(target_os = "windows")]
pub fn get_git_changes() -> Option<String> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()?;

    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn get_git_changes() -> Option<String> {
    None
}
