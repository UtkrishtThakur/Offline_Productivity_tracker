// processing/terminal.rs
//
// Deterministic terminal command normalization.
// Maps raw shell commands to workflow labels using
// prefix-based pattern matching — no AI, pure rules.

/// Workflow label produced from terminal command classification.
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalWorkflow {
    RustBuild,
    GitCommit,
    NodeJs,
    Python,
    Docker,
    FileNavigation,
    SystemAdmin,
    Unknown,
}

impl TerminalWorkflow {

    pub fn label(&self) -> &'static str {
        match self {
            Self::RustBuild => "Rust build workflow",
            Self::GitCommit => "Git commit workflow",
            Self::NodeJs => "Node.js workflow",
            Self::Python => "Python workflow",
            Self::Docker => "Docker workflow",
            Self::FileNavigation => "File navigation",
            Self::SystemAdmin => "System administration",
            Self::Unknown => "Terminal command",
        }
    }
}

/// Classify a single command into a workflow category.
pub fn classify_command(cmd: &str) -> TerminalWorkflow {

    let trimmed = cmd.trim();

    if trimmed.is_empty() {
        return TerminalWorkflow::Unknown;
    }

    // Extract the base command (first word)
    let base = trimmed
        .split_whitespace()
        .next()
        .unwrap_or("");

    match base {
        // Rust ecosystem
        "cargo" | "rustc" | "rustup" | "clippy" =>
            TerminalWorkflow::RustBuild,

        // Git operations
        "git" =>
            TerminalWorkflow::GitCommit,

        // Node.js ecosystem
        "npm" | "npx" | "yarn" | "pnpm" | "node" | "bun" | "deno" =>
            TerminalWorkflow::NodeJs,

        // Python ecosystem
        "python" | "python3" | "pip" | "pip3"
        | "pytest" | "poetry" | "pdm" | "uv"
        | "conda" | "virtualenv" | "venv" =>
            TerminalWorkflow::Python,

        // Docker / containers
        "docker" | "docker-compose" | "podman" =>
            TerminalWorkflow::Docker,

        // File navigation
        "cd" | "ls" | "ll" | "la" | "cat" | "less"
        | "head" | "tail" | "find" | "fd" | "tree"
        | "mkdir" | "rmdir" | "cp" | "mv" | "ln"
        | "pwd" | "exa" | "bat" | "rg" =>
            TerminalWorkflow::FileNavigation,

        // System administration
        "sudo" | "systemctl" | "journalctl"
        | "apt" | "pacman" | "dnf" | "yum"
        | "brew" | "snap" | "flatpak"
        | "chmod" | "chown" | "kill" | "ps"
        | "top" | "htop" | "df" | "du"
        | "mount" | "umount" | "ssh" | "scp" =>
            TerminalWorkflow::SystemAdmin,

        _ => TerminalWorkflow::Unknown,
    }
}

/// Given a sequence of commands, produce deduplicated
/// workflow labels in order.
///
/// Consecutive commands of the same workflow type are
/// collapsed into a single label.
pub fn detect_workflows(
    commands: &[String],
) -> Vec<String> {

    let mut workflows: Vec<String> = Vec::new();
    let mut last_label: Option<&str> = None;

    for cmd in commands {

        let workflow = classify_command(cmd);
        let label = workflow.label();

        if Some(label) != last_label {
            workflows.push(label.to_string());
            last_label = Some(label);
        }
    }

    workflows
}

/// Deduplicate a list of commands, preserving order.
/// Consecutive identical commands are collapsed.
pub fn deduplicate_commands(
    commands: &[String],
) -> Vec<String> {

    let mut deduped: Vec<String> = Vec::new();

    for cmd in commands {
        if deduped.last().map(|s| s.as_str()) != Some(cmd.as_str()) {
            deduped.push(cmd.clone());
        }
    }

    deduped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_cargo() {
        assert_eq!(
            classify_command("cargo build --release"),
            TerminalWorkflow::RustBuild
        );
    }

    #[test]
    fn test_classify_git() {
        assert_eq!(
            classify_command("git add ."),
            TerminalWorkflow::GitCommit
        );
    }

    #[test]
    fn test_classify_unknown() {
        assert_eq!(
            classify_command("some-custom-tool"),
            TerminalWorkflow::Unknown
        );
    }

    #[test]
    fn test_detect_workflows() {
        let cmds = vec![
            "cargo build".to_string(),
            "cargo run".to_string(),
            "git add .".to_string(),
            "git commit -m 'test'".to_string(),
        ];

        let wf = detect_workflows(&cmds);

        assert_eq!(wf, vec![
            "Rust build workflow",
            "Git commit workflow",
        ]);
    }

    #[test]
    fn test_deduplicate() {
        let cmds = vec![
            "ls".to_string(),
            "ls".to_string(),
            "cd foo".to_string(),
            "ls".to_string(),
        ];

        let deduped = deduplicate_commands(&cmds);

        assert_eq!(deduped, vec![
            "ls".to_string(),
            "cd foo".to_string(),
            "ls".to_string(),
        ]);
    }
}
