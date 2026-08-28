//! Import: fetch revisions from a source (Wikipedia or Git).
//!
//! `probe` is an intentionally cheap, metadata-only pass: it counts revisions
//! and reports the time range, without ever downloading full revision bodies.

use crate::config::{Config, Source};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
pub mod wikipedia;

/// Metadata about a source: how many revisions exist and over what time range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SourceProbe {
    /// Number of revisions in the source's history.
    pub revision_count: u64,

    /// Timestamp of the oldest (first) revision.
    pub oldest_revision: Option<DateTime<Utc>>,

    /// Timestamp of the newest (latest) revision.
    pub newest_revision: Option<DateTime<Utc>>,

    /// Which source backend was probed.
    pub source: Source,
}

/// A single revision from any source (Wikipedia, Git, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision {
    /// Revision identifier (pageid|revid for Wikipedia, commit hash for Git).
    pub id: String,
    /// Author who made this revision.
    pub author: String,
    /// Timestamp of the revision.
    pub timestamp: DateTime<Utc>,
    /// Full text content of the revision.
    pub content: String,
}

/// Errors produced while probing or importing a source.
#[derive(Debug)]
pub enum ImportError {
    /// No source was named by flags or config.
    NoSource,

    /// Both `url` and `source`/`page` were given.
    AmbiguousSource,

    /// The probed source backend is not implemented yet.
    Unsupported(Source),

    /// A transport or HTTP error occurred.
    Network(String),

    /// The remote API returned an unexpected response.
    Api(String),

    /// The requested page/file does not exist on the source.
    MissingPage(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::NoSource => write!(f, "no source specified"),
            ImportError::AmbiguousSource => {
                write!(f, "ambiguous source: both url and source/page given")
            }
            ImportError::Unsupported(s) => write!(f, "source not implemented yet: {s:?}"),
            ImportError::Network(e) => write!(f, "network error: {e}"),
            ImportError::Api(m) => write!(f, "api error: {m}"),
            ImportError::MissingPage(t) => write!(f, "page not found: {t}"),
        }
    }
}

impl std::error::Error for ImportError {}

/// Size up a source: revision count + newest/oldest timestamps. Metadata only.
pub fn probe(config: &Config) -> Result<SourceProbe, ImportError> {
    let (source, page) = resolve_target(config)?;
    match source {
        Source::Wikipedia => wikipedia::probe(&page),
        Source::Git => Err(ImportError::Unsupported(Source::Git)),
    }
}

/// Fetch all revisions with full content from a source.
pub fn import_revisions(config: &Config) -> Result<Vec<Revision>, ImportError> {
    let (source, page) = resolve_target(config)?;
    match source {
        Source::Wikipedia => wikipedia::fetch_revisions(&page),
        Source::Git => Err(ImportError::Unsupported(Source::Git)),
    }
}

/// Resolve which source and target page a config names. Applies the
/// `url` vs `source`+`page` rules from the plan's input-dispatch decision.
fn resolve_target(config: &Config) -> Result<(Source, String), ImportError> {
    match (
        &config.import.url,
        config.import.source,
        &config.import.page,
    ) {
        (Some(_url), Some(_), _) | (Some(_url), None, Some(_)) => Err(ImportError::AmbiguousSource),
        (Some(url), None, None) => classify(url),
        (None, Some(source), Some(page)) => Ok((source, page.clone())),
        (None, Some(_), None) | (None, None, Some(_)) | (None, None, None) => {
            Err(ImportError::NoSource)
        }
    }
}

/// Classify a free-form identifier into a (source, page) pair.
fn classify(what: &str) -> Result<(Source, String), ImportError> {
    if let Some(rest) = what.strip_prefix("https://en.wikipedia.org/wiki/") {
        Ok((Source::Wikipedia, clean_title(rest)))
    } else if what.starts_with("https://github.com/") || what.starts_with("github.com/") {
        Ok((Source::Git, what.to_string()))
    } else {
        // Not a URL: treat as a Wikipedia page title (plan's dispatch rule).
        Ok((Source::Wikipedia, what.to_string()))
    }
}

/// Strip query/fragment and translate underscores from a wiki URL path segment.
fn clean_title(rest: &str) -> String {
    rest.split(['#', '?'])
        .next()
        .unwrap_or(rest)
        .replace('_', " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn classify_wiki_url() {
        let mut c = Config::default();
        c.import.url = Some("https://en.wikipedia.org/wiki/Evolution".into());
        assert_eq!(
            resolve_target(&c).unwrap(),
            (Source::Wikipedia, "Evolution".to_string())
        );
    }

    #[test]
    fn classify_plain_title() {
        let mut c = Config::default();
        c.import.url = Some("Evolution".into());
        assert_eq!(
            resolve_target(&c).unwrap(),
            (Source::Wikipedia, "Evolution".to_string())
        );
    }

    #[test]
    fn classify_github_flag_is_git() {
        let mut c = Config::default();
        c.import.url = Some("https://github.com/owner/repo/blob/main/README.md".into());
        assert_eq!(
            resolve_target(&c).unwrap(),
            (
                Source::Git,
                "https://github.com/owner/repo/blob/main/README.md".into()
            )
        );
    }

    #[test]
    fn ambiguous_url_plus_source_errors() {
        let mut c = Config::default();
        c.import.url = Some("Evolution".into());
        c.import.source = Some(Source::Wikipedia);
        assert!(matches!(
            resolve_target(&c),
            Err(ImportError::AmbiguousSource)
        ));
    }

    #[test]
    fn nothing_named_is_no_source() {
        assert!(matches!(
            resolve_target(&Config::default()),
            Err(ImportError::NoSource)
        ));
    }
}
