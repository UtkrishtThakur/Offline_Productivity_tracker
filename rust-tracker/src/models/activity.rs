use serde::{Deserialize, Serialize};

/// Structured summary of git activity within an activity group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitSummary {

    pub repo: String,

    pub branch: String,

    pub commit_count: u32,

    pub unpushed: u32,

    pub changed_files: Vec<String>,

    pub dev_areas: Vec<String>,
}

/// A grouped activity session — the output of the reconstruction pipeline.
/// Represents a contiguous block of related work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityGroup {

    pub start_time: String,

    pub end_time: String,

    pub project: Option<String>,

    pub app: String,

    pub total_duration_sec: u64,

    pub files_touched: Vec<String>,

    pub languages: Vec<String>,

    pub terminal_workflows: Vec<String>,

    pub git_summary: Option<GitSummary>,
}
