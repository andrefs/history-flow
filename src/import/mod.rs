//! Import: fetch revisions from a source (Wikipedia or Git).
//!
//! `probe` is an intentionally cheap, metadata-only pass: it counts revisions
//! and reports the time range, without ever downloading full revision bodies.

use crate::config::Source;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fmt;

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
