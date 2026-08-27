//! Configuration: the `Config` type, section structs, and built-in defaults.

use serde::{Deserialize, Serialize};

/// Which source backend to use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
#[value(rename_all = "snake_case")]
pub enum Source {
    /// The Wikipedia Action API.
    Wikipedia,
    /// A single tracked file in a git repository.
    Git,
}

/// How per-revision line authorship is computed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
#[value(rename_all = "snake_case")]
pub enum AttributionMode {
    /// Reverted/re-added text re-links to its original author.
    #[default]
    Provenance,
    /// Attribute each line to whoever most recently introduced it.
    LastEditor,
}

/// How reintroduced text is matched back to its earlier incarnation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
#[value(rename_all = "snake_case")]
pub enum MatchMode {
    /// Match reintroduced lines on exact text equality.
    #[default]
    Exact,
    /// Match on fuzzy similarity above `fuzzy_thresh`.
    Fuzzy,
}

/// How revisions are selected (`all` / `last=N` / `nth=N`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
#[value(rename_all = "snake_case")]
pub enum ImportMode {
    /// Import every revision.
    #[default]
    All,
    /// Import only the N most recent revisions.
    Last,
    /// Import every Nth revision.
    Nth,
}

fn default_last() -> usize {
    200
}
fn default_nth() -> usize {
    5
}
fn default_fuzzy_thresh() -> f64 {
    0.95
}

/// Import section: where revisions come from and how many to take.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImportConfig {
    /// Backend to use; `None` means "user has not chosen" (no implicit default).
    #[serde(default)]
    pub source: Option<Source>,
    /// Target document: Wikipedia title or path to one tracked git file.
    #[serde(default)]
    pub page: Option<String>,
    /// Alternative to `source` + `page`: a Wikipedia or GitHub URL to classify.
    #[serde(default)]
    pub url: Option<String>,
    /// How to select revisions (`all` / `last=N` / `nth=N`).
    #[serde(default)]
    pub mode: ImportMode,
    /// N when `mode = "last"`.
    #[serde(default = "default_last")]
    pub last: usize,
    /// N when `mode = "nth"`.
    #[serde(default = "default_nth")]
    pub nth: usize,
}

impl Default for ImportConfig {
    fn default() -> Self {
        Self {
            source: None,
            page: None,
            url: None,
            mode: ImportMode::All,
            last: default_last(),
            nth: default_nth(),
        }
    }
}

/// Attribution section: how per-line authorship is computed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttributionConfig {
    /// Provenance re-links reverted text to its original author; last_editor does not.
    #[serde(default)]
    pub mode: AttributionMode,
    /// Reintroduction matching: exact text or fuzzy similarity.
    #[serde(default)]
    pub match_mode: MatchMode,
    /// Similarity threshold when `match_mode = "fuzzy"` (0.0–1.0).
    #[serde(default = "default_fuzzy_thresh")]
    pub fuzzy_thresh: f64,
}

impl Default for AttributionConfig {
    fn default() -> Self {
        Self {
            mode: AttributionMode::Provenance,
            match_mode: MatchMode::Exact,
            fuzzy_thresh: default_fuzzy_thresh(),
        }
    }
}

/// Full configuration for a history-flow run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Config {
    /// Import section.
    #[serde(default)]
    pub import: ImportConfig,
    /// Attribution section.
    #[serde(default)]
    pub attribution: AttributionConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_defaults() {
        let c = Config::default();
        assert_eq!(c.import.source, None);
        assert_eq!(c.import.page, None);
        assert_eq!(c.import.url, None);
        assert_eq!(c.import.mode, ImportMode::All);
        assert_eq!(c.import.last, 200);
        assert_eq!(c.import.nth, 5);
        assert_eq!(c.attribution.mode, AttributionMode::Provenance);
        assert_eq!(c.attribution.match_mode, MatchMode::Exact);
        assert_eq!(c.attribution.fuzzy_thresh, 0.95);
    }
}
