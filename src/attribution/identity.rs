//! Attribution: token-identity / provenance graph.

use serde::{Deserialize, Serialize};

/// One document line with stable identity across revisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    /// Unique identifier for this line.
    pub id: usize,

    /// The line's text content.
    pub text: String,

    /// Index of the revision where this line first appeared.
    pub origin_rev: usize,

    /// Author who introduced this line.
    pub origin_author: String,

    /// Revision index when this line was introduced.
    pub introduced_in: usize,

    /// Revision index when this line was deleted (None = still alive).
    pub deleted_in: Option<usize>,
}

/// Provenance record: for a given revision, which line occupies each position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinePosition {
    /// ID of the line occupying this position.
    pub line_id: usize,

    /// Zero-based line index within the revision.
    pub line_index: usize,
}

/// Complete author grid: for each revision, the author of each line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorGrid {
    /// Number of revisions in the grid.
    pub revisions: usize,

    /// 2D grid: grid[rev_index][line_index] = author name.
    pub grid: Vec<Vec<String>>,
}

/// Error during attribution.
#[derive(Debug, thiserror::Error)]
pub enum AttributionError {
    /// Diff chain length doesn't match expected (revisions - 1).
    #[error("diff chain length ({0}) != revisions count ({1}) - 1")]
    DiffLenMismatch(usize, usize),

    /// No revisions provided to attribute.
    #[error("empty revision list")]
    EmptyRevisions,

    /// Referenced line ID does not exist.
    #[error("line not found: {0}")]
    LineNotFound(usize),
}
