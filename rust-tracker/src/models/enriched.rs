use serde::{Deserialize, Serialize};

use super::event::Event;

/// An event enriched with deterministic context
/// extracted from window titles, git data, and file paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedEvent {

    #[serde(flatten)]
    pub event: Event,

    /// Project name extracted from editor title
    pub project: Option<String>,

    /// Active file extracted from editor title
    pub file: Option<String>,

    /// Programming language inferred from file extension
    pub language: Option<String>,

    /// Canonical application name
    pub normalized_app: String,

    /// Repository path from git context
    pub repo: Option<String>,

    /// Branch name from git context
    pub branch: Option<String>,
}
