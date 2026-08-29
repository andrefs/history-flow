//! Import: fetch revisions from a source (Wikipedia or Git).
//!
//! `probe` is an intentionally cheap, metadata-only pass: it counts revisions
//! and reports the time range, without ever downloading full revision bodies.

use crate::config::{Config, ImportMode, Source};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
pub mod git;
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

    /// A git URL named a repository but no tracked file.
    RepositoryNeedsFile,

    /// The `git` command failed or is not installed.
    Git(String),
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
            ImportError::RepositoryNeedsFile => {
                write!(f, "git url needs a file: use owner/repo/blob/<rev>/<path>")
            }
            ImportError::Git(e) => write!(f, "git: {e}"),
        }
    }
}

impl std::error::Error for ImportError {}

/// Size up a source: revision count + newest/oldest timestamps. Metadata only.
pub fn probe(config: &Config) -> Result<SourceProbe, ImportError> {
    let (source, page) = resolve_target(config)?;
    match source {
        Source::Wikipedia => wikipedia::probe(&page),
        Source::Git => git::probe(config),
    }
}

/// Fetch all revisions with full content from a source.
pub fn import_revisions(config: &Config) -> Result<Vec<Revision>, ImportError> {
    let (source, page) = resolve_target(config)?;
    match source {
        Source::Wikipedia => wikipedia::fetch_revisions(&page),
        Source::Git => git::fetch_revisions(config),
    }
}

/// Apply `mode` to a fetched revision list: all, last=N, or every Nth.
pub fn select_revisions(
    revisions: Vec<Revision>,
    mode: ImportMode,
    last: usize,
    nth: usize,
) -> Vec<Revision> {
    match mode {
        ImportMode::All => revisions,
        ImportMode::Last => {
            let skip = revisions.len().saturating_sub(last);
            revisions.into_iter().skip(skip).collect()
        }
        ImportMode::Nth => revisions.into_iter().step_by(nth).collect(),
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
        parse_github_url(what).map(|(_, path)| (Source::Git, path))
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

/// Parse a GitHub blob URL into (owner/repo, file path).
/// Accepts `https://github.com/` or `github.com/` prefixes; path must be a
/// `blob/<rev>/<path...>` (bare repo URLs are an error).
pub(crate) fn parse_github_url(what: &str) -> Result<(String, String), ImportError> {
    let rest = what
        .strip_prefix("https://github.com/")
        .or_else(|| what.strip_prefix("github.com/"))
        .ok_or(ImportError::RepositoryNeedsFile)?;
    let mut parts = rest.split('/');
    let owner = parts.next();
    let repo = parts.next();
    match (owner, repo, parts.next()) {
        (Some(o), Some(r), Some("blob")) => {
            let _rev = parts.next();
            let path = parts.collect::<Vec<_>>().join("/");
            if !path.is_empty() {
                Ok((format!("{o}/{r}"), path))
            } else {
                Err(ImportError::RepositoryNeedsFile)
            }
        }
        _ => Err(ImportError::RepositoryNeedsFile),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use chrono::TimeZone;
    use std::path::Path;

    use crate::import::git::{commit_file, fresh_repo_dir};

    fn git_cfg(dir: &Path) -> Config {
        let mut c = Config::default();
        c.import.source = Some(Source::Git);
        c.import.repo = Some(dir.to_string_lossy().into_owned());
        c.import.page = Some("notes.txt".to_string());
        c
    }

    fn fake_revisions(n: usize) -> Vec<Revision> {
        (0..n)
            .map(|i| Revision {
                id: i.to_string(),
                author: "tester".to_string(),
                timestamp: Utc
                    .with_ymd_and_hms(2024, 1, 1 + i as u32, 0, 0, 0)
                    .unwrap(),
                content: format!("line {i}\n"),
            })
            .collect()
    }

    #[test]
    fn select_revisions_modes() {
        let revs = fake_revisions(6);

        assert_eq!(
            select_revisions(revs.clone(), ImportMode::All, 200, 5).len(),
            6
        );

        let last = select_revisions(revs.clone(), ImportMode::Last, 2, 5);
        let last_ids: Vec<&str> = last.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(last_ids, ["4", "5"]);

        let nth = select_revisions(revs.clone(), ImportMode::Nth, 200, 3);
        assert_eq!(nth.len(), 2);
        assert_eq!(nth[0].id.as_str(), "0");
        assert_eq!(nth[1].id.as_str(), "3");
    }

    #[test]
    fn source_probe_serializes() {
        let p = SourceProbe {
            revision_count: 3,
            oldest_revision: None,
            newest_revision: None,
            source: Source::Git,
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"revision_count\":3"));
        assert!(json.contains("\"source\":\"git\""));
    }

    #[test]
    fn probe_rejects_no_source() {
        let c = Config::default();
        assert!(matches!(probe(&c), Err(ImportError::NoSource)));
    }

    #[test]
    fn probe_dispatches_to_git_repo() {
        let dir = fresh_repo_dir();
        commit_file(&dir, "notes.txt", "one\n");
        commit_file(&dir, "notes.txt", "one\ntwo\n");

        let p = probe(&git_cfg(&dir)).unwrap();

        assert_eq!(p.revision_count, 2);
        assert_eq!(p.source, Source::Git);
        assert!(p.oldest_revision.unwrap() <= p.newest_revision.unwrap());
    }

    #[test]
    fn fetch_dispatches_to_git_repo() {
        let dir = fresh_repo_dir();
        commit_file(&dir, "notes.txt", "one\n");
        commit_file(&dir, "notes.txt", "one\ntwo\n");

        let revs = import_revisions(&git_cfg(&dir)).unwrap();

        assert_eq!(revs.len(), 2);
        assert_eq!(revs[0].content, "one\n");
        assert_eq!(revs[1].content, "one\ntwo\n");
        assert_eq!(revs[0].author, "tester");
        assert!(revs[0].timestamp <= revs[1].timestamp);
    }

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
    fn classify_github_blob_is_git() {
        let mut c = Config::default();
        c.import.url = Some("https://github.com/o/r/blob/main/README.md".into());
        assert_eq!(
            resolve_target(&c).unwrap(),
            (Source::Git, "README.md".to_string())
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

    #[test]
    fn classify_github_deep_path() {
        let mut c = Config::default();
        c.import.url = Some("https://github.com/o/r/blob/main/src/deep/nested.rs".into());
        assert_eq!(
            resolve_target(&c).unwrap(),
            (Source::Git, "src/deep/nested.rs".to_string())
        );
    }

    #[test]
    fn classify_github_bare_repo_errors() {
        let mut c = Config::default();
        c.import.url = Some("https://github.com/owner/repo".into());
        assert!(matches!(
            resolve_target(&c),
            Err(ImportError::RepositoryNeedsFile)
        ));
    }
}
