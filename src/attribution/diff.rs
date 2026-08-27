//! Diff: Myers O(ND) line diff via the `similar` crate.

use std::fmt;

/// One diff operation between old (left) and new (right) line sequences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffOp {
    /// Lines are equal in both sequences.
    Equal {
        /// Index in the old sequence.
        old_index: usize,
        /// Index in the new sequence.
        new_index: usize,
    },

    /// Line was inserted in the new sequence.
    Insert {
        /// Index in the new sequence.
        new_index: usize,
    },

    /// Line was deleted from the old sequence.
    Delete {
        /// Index in the old sequence.
        old_index: usize,
    },
}

impl fmt::Display for DiffOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffOp::Equal {
                old_index,
                new_index,
            } => write!(f, "  = {} {}", old_index, new_index),
            DiffOp::Insert { new_index } => write!(f, "  + {}", new_index),
            DiffOp::Delete { old_index } => write!(f, "  - {}", old_index),
        }
    }
}

/// Compute the Myers diff of `old` → `new` as a sequence of per-line ops.
pub fn diff_lines(old: &[String], new: &[String]) -> Vec<DiffOp> {
    let mut ops = Vec::new();
    let diff = similar::capture_diff_slices(similar::Algorithm::Myers, old, new);
    for change in diff {
        match change {
            similar::DiffOp::Equal {
                old_index,
                new_index,
                len,
            } => {
                for i in 0..len {
                    ops.push(DiffOp::Equal {
                        old_index: old_index + i,
                        new_index: new_index + i,
                    });
                }
            }
            similar::DiffOp::Insert {
                new_index, new_len, ..
            } => {
                for i in 0..new_len {
                    ops.push(DiffOp::Insert {
                        new_index: new_index + i,
                    });
                }
            }
            similar::DiffOp::Delete {
                old_index, old_len, ..
            } => {
                for i in 0..old_len {
                    ops.push(DiffOp::Delete {
                        old_index: old_index + i,
                    });
                }
            }

            similar::DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
                ..
            } => {
                for i in 0..old_len {
                    ops.push(DiffOp::Delete {
                        old_index: old_index + i,
                    });
                }
                for i in 0..new_len {
                    ops.push(DiffOp::Insert {
                        new_index: new_index + i,
                    });
                }
            }
        }
    }
    ops
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_one_line() {
        let old = vec!["a".into(), "b".into()];
        let new = vec!["a".into(), "x".into(), "b".into()];
        let ops = diff_lines(&old, &new);
        assert_eq!(
            ops,
            vec![
                DiffOp::Equal {
                    old_index: 0,
                    new_index: 0
                },
                DiffOp::Insert { new_index: 1 },
                DiffOp::Equal {
                    old_index: 1,
                    new_index: 2
                },
            ]
        );
    }

    #[test]
    fn delete_one_line() {
        let old = vec!["a".into(), "x".into(), "b".into()];
        let new = vec!["a".into(), "b".into()];
        let ops = diff_lines(&old, &new);
        assert_eq!(
            ops,
            vec![
                DiffOp::Equal {
                    old_index: 0,
                    new_index: 0
                },
                DiffOp::Delete { old_index: 1 },
                DiffOp::Equal {
                    old_index: 2,
                    new_index: 1
                },
            ]
        );
    }

    #[test]
    fn re_add_is_not_equal() {
        let old = vec!["a".into()];
        let new = vec!["a".into(), "b".into(), "a".into()];
        let ops = diff_lines(&old, &new);
        assert_eq!(
            ops,
            vec![
                DiffOp::Equal {
                    old_index: 0,
                    new_index: 0
                },
                DiffOp::Insert { new_index: 1 },
                DiffOp::Insert { new_index: 2 },
            ]
        );
    }
}
