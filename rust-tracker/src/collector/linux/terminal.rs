use std::fs;

pub fn read_bash_history() -> Vec<String> {

    let home =
        std::env::var("HOME")
            .unwrap_or_default();

    let path =
        format!("{}/.bash_history", home);

    let contents =
        fs::read_to_string(path)
            .unwrap_or_default();

    contents
        .lines()
        .map(|s| s.to_string())
        .collect()
}