//! Attribution: token-identity / provenance graph.

use crate::attribution::diff::DiffOp;
use crate::import::Revision;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

/// Build the line-identity graph from a revision list and diff chain.
/// Returns an AuthorGrid with the author of every line in every revision.
pub fn build_identity_graph(
    revisions: &[Revision],
    diffs: &[Vec<DiffOp>],
) -> Result<AuthorGrid, AttributionError> {
    if revisions.is_empty() {
        return Err(AttributionError::EmptyRevisions);
    }
    if diffs.len() != revisions.len().saturating_sub(1) {
        return Err(AttributionError::DiffLenMismatch(
            diffs.len(),
            revisions.len(),
        ));
    }

    let mut lines: Vec<Line> = Vec::new();
    let mut next_line_id = 0;

    // Initial revision: all lines are new
    let first_lines: Vec<String> = revisions[0]
        .content
        .lines()
        .map(|s| s.to_string())
        .collect();

    for text in first_lines.iter() {
        lines.push(Line {
            id: next_line_id,
            text: text.clone(),
            origin_rev: 0,
            origin_author: revisions[0].author.clone(),
            introduced_in: 0,
            deleted_in: None,
        });
        next_line_id += 1;
    }

    // Initialize line positions for the first revision
    let mut rev_line_positions: Vec<Vec<LinePosition>> = vec![];
    rev_line_positions.push(
        first_lines
            .iter()
            .enumerate()
            .map(|(i, _)| LinePosition {
                line_id: i,
                line_index: i,
            })
            .collect(),
    );

    // Walk the diff chain
    for (rev_index, diff) in diffs.iter().enumerate() {
        let current_rev = rev_index + 1;
        let mut new_positions = Vec::new();
        let mut line_map: HashMap<usize, usize> = HashMap::new();

        for op in diff {
            match op {
                DiffOp::Equal {
                    old_index,
                    new_index,
                } => {
                    let line_id = rev_line_positions[rev_index][*old_index].line_id;
                    line_map.insert(*old_index, *new_index);
                    new_positions.push(LinePosition {
                        line_id,
                        line_index: *new_index,
                    });
                }
                DiffOp::Insert { new_index } => {
                    let text = revisions[current_rev]
                        .content
                        .lines()
                        .nth(*new_index)
                        .unwrap_or("");
                    lines.push(Line {
                        id: next_line_id,
                        text: text.to_string(),
                        origin_rev: current_rev,
                        origin_author: revisions[current_rev].author.clone(),
                        introduced_in: current_rev,
                        deleted_in: None,
                    });
                    new_positions.push(LinePosition {
                        line_id: next_line_id,
                        line_index: *new_index,
                    });
                    next_line_id += 1;
                }
                DiffOp::Delete { old_index } => {
                    let line_id = rev_line_positions[rev_index][*old_index].line_id;
                    lines[line_id].deleted_in = Some(current_rev);
                    line_map.insert(*old_index, usize::MAX);
                }
            }
        }

        new_positions.sort_by_key(|p| p.line_index);
        rev_line_positions.push(new_positions);
    }

    // Build AuthorGrid
    let mut grid = Vec::with_capacity(revisions.len());
    for positions in rev_line_positions.iter() {
        let mut rev_authors = Vec::with_capacity(positions.len());
        for pos in positions {
            rev_authors.push(lines[pos.line_id].origin_author.clone());
        }
        grid.push(rev_authors);
    }

    Ok(AuthorGrid {
        revisions: revisions.len(),
        grid,
    })
}
