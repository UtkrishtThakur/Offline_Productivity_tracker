use std::process::Command;

pub fn get_idle_ms() -> Option<u64> {

    let output = Command::new("xprintidle")
        .output()
        .ok()?;

    let stdout =
        String::from_utf8_lossy(&output.stdout);

    stdout.trim().parse::<u64>().ok()
}