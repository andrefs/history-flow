//! Attribution: compute per-line author provenance from revision history.

pub mod diff;
pub mod identity;

pub use diff::DiffOp;
pub use identity::{
    AttributionError, AuthorGrid, GridCell, Line, LinePosition, build_identity_graph,
};

/// High-level attribution pipeline: given revisions and diffs, produce AuthorGrid.
pub fn run_attribution(
    revisions: &[crate::import::Revision],
    diffs: &[Vec<DiffOp>],
) -> Result<AuthorGrid, AttributionError> {
    identity::build_identity_graph(revisions, diffs)
}
