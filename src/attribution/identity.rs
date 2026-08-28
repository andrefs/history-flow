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

/// A single author + size cell in the attribution grid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridCell {
    /// Author credited with this line in this revision.
    pub author: String,

    /// Line length in characters (segment height in the chart).
    pub size: usize,
}

/// Complete author grid: for each revision, the author of each line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorGrid {
    /// Number of revisions in the grid.
    pub revisions: usize,

    /// RFC3339 timestamp of each revision (index = revision index).
    pub dates: Vec<String>,

    /// 2D grid: grid\[rev_index]\[line_index] = GridCell for that line in that revision.
    pub grid: Vec<Vec<GridCell>>,
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

    relink_reverts(&mut lines);

    // Build AuthorGrid
    let mut grid = Vec::with_capacity(revisions.len());
    for positions in rev_line_positions.iter() {
        let mut rev_authors = Vec::with_capacity(positions.len());
        for pos in positions {
            rev_authors.push(GridCell {
                author: lines[pos.line_id].origin_author.clone(),
                size: lines[pos.line_id].text.len().max(1),
            });
        }
        grid.push(rev_authors);
    }

    Ok(AuthorGrid {
        revisions: revisions.len(),
        dates: revisions.iter().map(|r| r.timestamp.to_rfc3339()).collect(),
        grid,
    })
}

/// Re-link re-added lines to their original author (revert detection).
fn relink_reverts(lines: &mut [Line]) {
    // Map from text to (origin_rev, origin_author) of the earliest deleted line with that text
    let mut deleted_by_text: HashMap<String, (usize, String)> = HashMap::new();
    for line in lines.iter() {
        if line.deleted_in.is_some() {
            deleted_by_text
                .entry(line.text.clone())
                .and_modify(|e| {
                    if line.origin_rev < e.0 {
                        e.0 = line.origin_rev;
                        e.1 = line.origin_author.clone();
                    }
                })
                .or_insert((line.origin_rev, line.origin_author.clone()));
        }
    }

    // Apply re-link
    for line in lines.iter_mut() {
        if line.deleted_in.is_none()
            && line.introduced_in > 0
            && let Some((orig_rev, orig_author)) = deleted_by_text.get(&line.text)
            && *orig_rev < line.origin_rev
        {
            line.origin_author = orig_author.clone();
            line.origin_rev = *orig_rev;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribution::diff::DiffOp;
    use chrono::{TimeZone, Utc};

    fn rev(id: &str, author: &str, content: &str) -> Revision {
        Revision {
            id: id.into(),
            author: author.into(),
            timestamp: Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
            content: content.into(),
        }
    }

    #[test]
    fn revert_relink_add_delete_readd() {
        // Rev 0: Alice adds "foo"
        // Rev 1: Bob deletes "foo"
        // Rev 2: Carol re-adds "foo" -> should re-link to Alice
        let revisions = vec![
            rev("0", "Alice", "foo\n"),
            rev("1", "Bob", ""),
            rev("2", "Carol", "foo\n"),
        ];
        let diffs = vec![
            vec![DiffOp::Delete { old_index: 0 }], // 0→1: delete "foo"
            vec![DiffOp::Insert { new_index: 0 }], // 1→2: insert "foo"
        ];

        let grid = build_identity_graph(&revisions, &diffs).unwrap();
        // Rev 0 line 0 = Alice
        assert_eq!(grid.grid[0][0].author, "Alice");
        assert_eq!(grid.grid[0][0].size, 3);
        // Rev 1 = empty
        assert_eq!(grid.grid[1].len(), 0);
        // Rev 2 line 0 = Alice (re-linked, not Carol)
        assert_eq!(grid.grid[2][0].author, "Alice");
        assert_eq!(grid.grid[2][0].size, 3);
    }

    #[test]
    fn no_relink_for_new_content() {
        // Rev 0: Alice adds "foo"
        // Rev 1: Bob adds "bar" (new, not a revert)
        let revisions = vec![rev("0", "Alice", "foo\n"), rev("1", "Bob", "foo\nbar\n")];
        let diffs = vec![vec![
            DiffOp::Equal {
                old_index: 0,
                new_index: 0,
            },
            DiffOp::Insert { new_index: 1 },
        ]];

        let grid = build_identity_graph(&revisions, &diffs).unwrap();
        assert_eq!(grid.grid[1][0].author, "Alice"); // foo stays Alice
        assert_eq!(grid.grid[1][0].size, 3); // foo length
        assert_eq!(grid.grid[1][1].author, "Bob"); // bar is Bob
        assert_eq!(grid.grid[1][1].size, 3); // bar length
    }

    #[test]
    fn multiple_reverts_pick_earliest() {
        // Rev 0: Alice adds "foo"
        // Rev 1: Bob deletes "foo"
        // Rev 2: Carol adds "foo"
        // Rev 3: Dave deletes "foo"
        // Rev 4: Eve adds "foo" -> should re-link to Alice (earliest)
        let revisions = vec![
            rev("0", "Alice", "foo\n"),
            rev("1", "Bob", ""),
            rev("2", "Carol", "foo\n"),
            rev("3", "Dave", ""),
            rev("4", "Eve", "foo\n"),
        ];
        let diffs = vec![
            vec![DiffOp::Delete { old_index: 0 }],
            vec![DiffOp::Insert { new_index: 0 }],
            vec![DiffOp::Delete { old_index: 0 }],
            vec![DiffOp::Insert { new_index: 0 }],
        ];

        let grid = build_identity_graph(&revisions, &diffs).unwrap();
        assert_eq!(grid.grid[4][0].author, "Alice"); // earliest origin wins
        assert_eq!(grid.grid[4][0].size, 3); // foo length
    }

    #[test]
    fn size_tracks_line_length() {
        let revisions = vec![rev("0", "Alice", "a\nlonger line\nthree words here\n")];
        let diffs: Vec<Vec<DiffOp>> = vec![];

        let grid = build_identity_graph(&revisions, &diffs).unwrap();
        assert_eq!(
            grid.grid[0][0],
            GridCell {
                author: "Alice".into(),
                size: 1
            }
        );
        assert_eq!(
            grid.grid[0][1],
            GridCell {
                author: "Alice".into(),
                size: 11
            }
        );
        assert_eq!(
            grid.grid[0][2],
            GridCell {
                author: "Alice".into(),
                size: 16
            }
        );
    }
}
