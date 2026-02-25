//! Token-level diff using Myers algorithm via the `similar` crate.
//!
//! Operates on the normalized form of each token so that minor case or
//! diacritic differences do not produce spurious diffs.
//!
//! Consecutive operations of the same kind are grouped into a single
//! [`TokenDiff`] entry to produce compact, human-readable output.

use serde::{Deserialize, Serialize};
use similar::{Algorithm, DiffOp};

use rt_core::Token;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Disposition of a group of tokens in the diff output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind {
    Equal,
    Inserted,
    Deleted,
    Substituted,
}

/// A grouped, token-level diff entry.
///
/// `left_tokens` and `right_tokens` hold the **display** text (not normalized)
/// of the tokens involved in this diff group. For `Equal` groups both vecs
/// have the same content; for `Inserted` only `right_tokens` is populated;
/// for `Deleted` only `left_tokens`; for `Substituted` both are non-empty.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenDiff {
    pub kind: DiffKind,
    pub left_tokens: Vec<String>,
    pub right_tokens: Vec<String>,
    /// Byte offset of the first left token within the block's canonical text,
    /// or 0 if there is no left token (insertion).
    pub left_offset: usize,
    /// Byte offset of the first right token within the block's canonical text,
    /// or 0 if there is no right token (deletion).
    pub right_offset: usize,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Compute a token-level diff between `left` and `right` token sequences.
///
/// Uses the Myers diff algorithm (via the [`similar`] crate) on the normalized
/// token text. Consecutive changes of the same kind are grouped into single
/// [`TokenDiff`] entries. Adjacent `Deleted`+`Inserted` groups are merged into
/// `Substituted` entries.
pub fn token_diff(left: &[Token], right: &[Token]) -> Vec<TokenDiff> {
    // Build string slices of normalized tokens for the diff engine.
    let left_norm: Vec<&str> = left.iter().map(|t| t.normalized.as_str()).collect();
    let right_norm: Vec<&str> = right.iter().map(|t| t.normalized.as_str()).collect();

    let ops = similar::capture_diff_slices(Algorithm::Myers, &left_norm, &right_norm);

    // Expand DiffOps into a flat change stream.
    let mut changes: Vec<RawChange> = Vec::new();
    for op in &ops {
        match op {
            DiffOp::Equal { old_index, new_index, len } => {
                for k in 0..*len {
                    changes.push(RawChange {
                        tag: RawTag::Equal,
                        left_token: Some(&left[old_index + k]),
                        right_token: Some(&right[new_index + k]),
                    });
                }
            }
            DiffOp::Delete { old_index, old_len, .. } => {
                for k in 0..*old_len {
                    changes.push(RawChange {
                        tag: RawTag::Delete,
                        left_token: Some(&left[old_index + k]),
                        right_token: None,
                    });
                }
            }
            DiffOp::Insert { new_index, new_len, .. } => {
                for k in 0..*new_len {
                    changes.push(RawChange {
                        tag: RawTag::Insert,
                        left_token: None,
                        right_token: Some(&right[new_index + k]),
                    });
                }
            }
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                // Decompose Replace into Delete + Insert to enable Substituted merging.
                for k in 0..*old_len {
                    changes.push(RawChange {
                        tag: RawTag::Delete,
                        left_token: Some(&left[old_index + k]),
                        right_token: None,
                    });
                }
                for k in 0..*new_len {
                    changes.push(RawChange {
                        tag: RawTag::Insert,
                        left_token: None,
                        right_token: Some(&right[new_index + k]),
                    });
                }
            }
        }
    }

    group_and_merge(changes)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

#[derive(PartialEq, Eq, Clone, Copy)]
enum RawTag {
    Equal,
    Delete,
    Insert,
}

struct RawChange<'a> {
    tag: RawTag,
    left_token: Option<&'a Token>,
    right_token: Option<&'a Token>,
}

/// Group consecutive raw changes of the same tag, then merge adjacent
/// Delete+Insert groups into Substituted groups.
fn group_and_merge(changes: Vec<RawChange<'_>>) -> Vec<TokenDiff> {
    // Step 1: group consecutive same-tag runs.
    // Each group is (tag, left_texts, right_texts, left_offset, right_offset).
    let mut groups: Vec<(RawTag, Vec<String>, Vec<String>, usize, usize)> = Vec::new();

    for ch in changes {
        let lt = ch.left_token.map(|t| t.text.clone()).unwrap_or_default();
        let rt = ch.right_token.map(|t| t.text.clone()).unwrap_or_default();
        let lo = ch.left_token.map(|t| t.offset).unwrap_or(0);
        let ro = ch.right_token.map(|t| t.offset).unwrap_or(0);

        if let Some(last) = groups.last_mut() {
            if last.0 == ch.tag {
                if !lt.is_empty() {
                    last.1.push(lt);
                }
                if !rt.is_empty() {
                    last.2.push(rt);
                }
                continue;
            }
        }

        let mut left_texts = Vec::new();
        let mut right_texts = Vec::new();
        if !lt.is_empty() {
            left_texts.push(lt);
        }
        if !rt.is_empty() {
            right_texts.push(rt);
        }
        groups.push((ch.tag, left_texts, right_texts, lo, ro));
    }

    // Step 2: merge adjacent Delete+Insert pairs into Substituted.
    let mut result: Vec<TokenDiff> = Vec::new();
    let mut i = 0;
    while i < groups.len() {
        let (tag, ref lt, ref rt, lo, ro) = groups[i];
        if tag == RawTag::Delete
            && i + 1 < groups.len()
            && groups[i + 1].0 == RawTag::Insert
        {
            let (_, ref rt2, _, _, ro2) = groups[i + 1];
            result.push(TokenDiff {
                kind: DiffKind::Substituted,
                left_tokens: lt.clone(),
                right_tokens: rt2.clone(),
                left_offset: lo,
                right_offset: ro2,
            });
            i += 2;
        } else {
            let kind = match tag {
                RawTag::Equal => DiffKind::Equal,
                RawTag::Delete => DiffKind::Deleted,
                RawTag::Insert => DiffKind::Inserted,
            };
            result.push(TokenDiff {
                kind,
                left_tokens: lt.clone(),
                right_tokens: rt.clone(),
                left_offset: lo,
                right_offset: ro,
            });
            i += 1;
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rt_core::{Token, TokenKind};

    fn word(text: &str, offset: usize) -> Token {
        Token {
            text: text.to_string(),
            kind: TokenKind::Word,
            normalized: text.to_lowercase(),
            offset,
        }
    }

    fn make_tokens(words: &[&str]) -> Vec<Token> {
        let mut offset = 0;
        words
            .iter()
            .map(|w| {
                let t = word(w, offset);
                offset += w.len() + 1;
                t
            })
            .collect()
    }

    #[test]
    fn equal_sequences_produce_equal_diff() {
        let tokens = make_tokens(&["the", "borrower", "shall", "repay"]);
        let diffs = token_diff(&tokens, &tokens);
        assert!(!diffs.is_empty());
        for d in &diffs {
            assert_eq!(d.kind, DiffKind::Equal, "unexpected kind: {:?}", d.kind);
        }
    }

    #[test]
    fn insertion_at_end() {
        let left = make_tokens(&["the", "borrower"]);
        let right = make_tokens(&["the", "borrower", "shall", "repay"]);
        let diffs = token_diff(&left, &right);
        let has_inserted = diffs.iter().any(|d| d.kind == DiffKind::Inserted);
        assert!(has_inserted, "should detect insertion: {:?}", diffs);
        let all_right: Vec<&str> = diffs
            .iter()
            .filter(|d| d.kind == DiffKind::Inserted)
            .flat_map(|d| d.right_tokens.iter().map(|s| s.as_str()))
            .collect();
        assert!(all_right.contains(&"shall"));
        assert!(all_right.contains(&"repay"));
    }

    #[test]
    fn deletion_at_end() {
        let left = make_tokens(&["the", "borrower", "shall", "repay"]);
        let right = make_tokens(&["the", "borrower"]);
        let diffs = token_diff(&left, &right);
        let has_deleted = diffs.iter().any(|d| d.kind == DiffKind::Deleted);
        assert!(has_deleted, "should detect deletion");
        let all_left: Vec<&str> = diffs
            .iter()
            .filter(|d| d.kind == DiffKind::Deleted)
            .flat_map(|d| d.left_tokens.iter().map(|s| s.as_str()))
            .collect();
        assert!(all_left.contains(&"shall"));
        assert!(all_left.contains(&"repay"));
    }

    #[test]
    fn substitution_detected() {
        let left = make_tokens(&["the", "borrower", "shall", "repay"]);
        let right = make_tokens(&["the", "lender", "shall", "repay"]);
        let diffs = token_diff(&left, &right);
        let has_change = diffs.iter().any(|d| {
            d.kind == DiffKind::Substituted
                || d.kind == DiffKind::Deleted
                || d.kind == DiffKind::Inserted
        });
        assert!(has_change, "should detect substitution: {:?}", diffs);
    }

    #[test]
    fn fully_disjoint_produces_substituted_or_delete_insert() {
        let left = make_tokens(&["alpha", "beta"]);
        let right = make_tokens(&["gamma", "delta"]);
        let diffs = token_diff(&left, &right);
        let has_sub = diffs.iter().any(|d| d.kind == DiffKind::Substituted);
        let has_del = diffs.iter().any(|d| d.kind == DiffKind::Deleted);
        let has_ins = diffs.iter().any(|d| d.kind == DiffKind::Inserted);
        assert!(
            has_sub || (has_del && has_ins),
            "should detect substitution or delete+insert: {:?}",
            diffs
        );
    }

    #[test]
    fn empty_left_all_inserted() {
        let left: Vec<Token> = vec![];
        let right = make_tokens(&["new", "clause"]);
        let diffs = token_diff(&left, &right);
        assert!(
            diffs.iter().all(|d| d.kind == DiffKind::Inserted),
            "all diffs should be insertions: {:?}",
            diffs
        );
    }

    #[test]
    fn empty_right_all_deleted() {
        let left = make_tokens(&["old", "clause"]);
        let right: Vec<Token> = vec![];
        let diffs = token_diff(&left, &right);
        assert!(
            diffs.iter().all(|d| d.kind == DiffKind::Deleted),
            "all diffs should be deletions: {:?}",
            diffs
        );
    }

    #[test]
    fn both_empty_no_diffs() {
        let diffs = token_diff(&[], &[]);
        assert!(diffs.is_empty(), "no diffs for empty sequences");
    }

    #[test]
    fn normalized_comparison_ignores_case() {
        // "Borrower" vs "borrower" — normalized both to "borrower" → Equal.
        let left = vec![Token {
            text: "Borrower".to_string(),
            kind: TokenKind::Word,
            normalized: "borrower".to_string(),
            offset: 0,
        }];
        let right = vec![Token {
            text: "borrower".to_string(),
            kind: TokenKind::Word,
            normalized: "borrower".to_string(),
            offset: 0,
        }];
        let diffs = token_diff(&left, &right);
        assert!(
            diffs.iter().all(|d| d.kind == DiffKind::Equal),
            "case-only difference should be Equal: {:?}",
            diffs
        );
    }

    #[test]
    fn middle_insertion_produces_one_inserted_token() {
        let left = make_tokens(&["the", "borrower", "shall", "repay"]);
        let right = make_tokens(&["the", "borrower", "promptly", "shall", "repay"]);
        let diffs = token_diff(&left, &right);
        let inserted_count: usize = diffs
            .iter()
            .filter(|d| d.kind == DiffKind::Inserted)
            .map(|d| d.right_tokens.len())
            .sum();
        assert_eq!(inserted_count, 1, "one token inserted: {:?}", diffs);
    }

    #[test]
    fn left_offset_populated_for_equal() {
        let left = make_tokens(&["alpha", "beta"]);
        let right = make_tokens(&["alpha", "gamma"]);
        let diffs = token_diff(&left, &right);
        // First group should be Equal for "alpha" at offset 0.
        assert_eq!(diffs[0].left_offset, 0);
    }

    #[test]
    fn token_diff_serializes_to_json() {
        let left = make_tokens(&["a"]);
        let right = make_tokens(&["b"]);
        let diffs = token_diff(&left, &right);
        let json = serde_json::to_string(&diffs).expect("should serialize");
        assert!(json.contains("\"deleted\"") || json.contains("\"substituted\""));
    }
}
