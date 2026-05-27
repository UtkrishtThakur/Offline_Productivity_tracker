// utils/paths.rs
//
// Path resolution utilities to handle relative paths and platform-specific
// directories consistently.

use std::path::{Path, PathBuf};

/// Expand a path string, resolving leading tildes to the home directory.
pub fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with('~') {
        if let Ok(home) = std::env::var("HOME") {
            let mut resolved = PathBuf::from(home);
            if path.len() > 1 && (path.starts_with("~/") || path.starts_with("~\\")) {
                resolved.push(&path[2..]);
            }
            return resolved;
        }
    }
    PathBuf::from(path)
}

/// Resolve a path relative to a base directory if it's not already absolute.
/// Also expands tildes.
pub fn resolve_path(raw_path: &str, base: &Path) -> PathBuf {
    let expanded = expand_tilde(raw_path);
    if expanded.is_absolute() {
        expanded
    } else {
        base.join(expanded)
    }
}

/// Get the absolute path to the directory containing the current executable.
pub fn get_exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde() {
        if let Ok(home) = std::env::var("HOME") {
            assert_eq!(expand_tilde("~/test"), PathBuf::from(home).join("test"));
        }
    }

    #[test]
    fn test_resolve_relative() {
        let base = PathBuf::from("/tmp");
        assert_eq!(resolve_path("data", &base), PathBuf::from("/tmp/data"));
        assert_eq!(resolve_path("/etc/config", &base), PathBuf::from("/etc/config"));
    }
}
