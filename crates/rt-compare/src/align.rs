//! Block alignment engine.
//!
//! Aligns two sequences of blocks using a multi-pass strategy:
//!
//! 1. **Exact structural_path match** — blocks whose `structural_path` is
//!    identical are paired first.
//! 2. **Anchor signature match** — among unmatched blocks, those with
//!    identical `anchor_signature` are paired.
//! 3. **Similarity scoring** — remaining blocks are scored pairwise using the
//!    token Jaccard index; pairs above the similarity threshold are matched.
//! 4. **LCS-based alignment** — any still-unmatched blocks are aligned using
//!    a longest-common-subsequence approach on their position in the flat list.
//! 5. **Move detection** — pairs matched by content (anchor or similarity ≥ 0.85)
//!    whose `structural_path` differs are reclassified as `Moved`.

use std::collections::{HashMap, HashSet};

use rt_core::Block;

/// Similarity threshold: a pair with Jaccard ≥ 0.7 counts as a content match.
const SIMILARITY_THRESHOLD: f64 = 0.7;

/// Move detection threshold: a pair with Jaccard ≥ 0.85 and a differing
/// structural_path is classified as `Moved` rather than `Modified`.
const MOVE_THRESHOLD: f64 = 0.85;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// The outcome of aligning a single block from the left document against a
/// block from the right document (or declaring it an insertion/deletion).
#[derive(Debug, Clone)]
pub enum BlockAlignment {
    /// Both blocks exist and their content is similar enough to be treated as
    /// the same logical block. `similarity` is the Jaccard token score.
    Matched {
        left: usize,
        right: usize,
        similarity: f64,
    },
    /// A block that appears only in the right document (new content).
    InsertedRight { right: usize },
    /// A block that appears only in the left document (removed content).
    DeletedLeft { left: usize },
    /// Content-equivalent blocks whose `structural_path` has changed.
    Moved {
        left: usize,
        right: usize,
        similarity: f64,
    },
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Align two flat block lists and produce an ordered sequence of
/// [`BlockAlignment`] values describing the relationship of each block.
///
/// The output is ordered: left-document blocks appear in their original order,
/// with inserted right-document blocks interleaved at the position where they
/// were first encountered.
pub fn align_blocks(left: &[Block], right: &[Block]) -> Vec<BlockAlignment> {
    // Track which indices have been matched so far.
    let mut left_matched: HashSet<usize> = HashSet::new();
    let mut right_matched: HashSet<usize> = HashSet::new();

    // paired[left_idx] = (right_idx, similarity)
    let mut pairs: Vec<(usize, usize, f64, bool)> = Vec::new(); // (l, r, sim, is_move)

    // -----------------------------------------------------------------------
    // Pass 1: exact structural_path match
    // -----------------------------------------------------------------------
    let right_by_path: HashMap<&str, usize> = right
        .iter()
        .enumerate()
        .map(|(i, b)| (b.structural_path.as_str(), i))
        .collect();

    for (li, lb) in left.iter().enumerate() {
        if let Some(&ri) = right_by_path.get(lb.structural_path.as_str()) {
            if !right_matched.contains(&ri) {
                let sim = block_similarity(lb, &right[ri]);
                pairs.push((li, ri, sim, false));
                left_matched.insert(li);
                right_matched.insert(ri);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Pass 2: anchor_signature match for still-unmatched blocks
    // -----------------------------------------------------------------------
    let right_by_anchor: HashMap<&str, usize> = right
        .iter()
        .enumerate()
        .filter(|(i, _)| !right_matched.contains(i))
        .map(|(i, b)| (b.anchor_signature.as_str(), i))
        .collect();

    for (li, lb) in left.iter().enumerate() {
        if left_matched.contains(&li) {
            continue;
        }
        if let Some(&ri) = right_by_anchor.get(lb.anchor_signature.as_str()) {
            if !right_matched.contains(&ri) {
                let sim = block_similarity(lb, &right[ri]);
                // Anchor matched but structural_path may differ → could be moved.
                let is_move = lb.structural_path != right[ri].structural_path;
                pairs.push((li, ri, sim, is_move));
                left_matched.insert(li);
                right_matched.insert(ri);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Pass 3: similarity scoring for remaining unmatched blocks
    // -----------------------------------------------------------------------
    let unmatched_left: Vec<usize> = (0..left.len())
        .filter(|i| !left_matched.contains(i))
        .collect();
    let unmatched_right: Vec<usize> = (0..right.len())
        .filter(|i| !right_matched.contains(i))
        .collect();

    // Compute all pairwise similarities for unmatched blocks.
    // For large documents this could be O(n*m); in practice legal documents
    // have bounded block counts per section so this is acceptable.
    let mut candidates: Vec<(usize, usize, f64)> = Vec::new();
    for &li in &unmatched_left {
        for &ri in &unmatched_right {
            let sim = block_similarity(&left[li], &right[ri]);
            if sim >= SIMILARITY_THRESHOLD {
                candidates.push((li, ri, sim));
            }
        }
    }

    // Greedy best-first matching: sort by descending similarity, then pick
    // the highest-scoring pair first, removing used indices.
    candidates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    let mut sim_left_used: HashSet<usize> = HashSet::new();
    let mut sim_right_used: HashSet<usize> = HashSet::new();

    for (li, ri, sim) in candidates {
        if sim_left_used.contains(&li) || sim_right_used.contains(&ri) {
            continue;
        }
        let is_move = left[li].structural_path != right[ri].structural_path && sim >= MOVE_THRESHOLD;
        pairs.push((li, ri, sim, is_move));
        left_matched.insert(li);
        right_matched.insert(ri);
        sim_left_used.insert(li);
        sim_right_used.insert(ri);
    }

    // -----------------------------------------------------------------------
    // Pass 4: LCS-based alignment for any blocks still unmatched after scoring
    // -----------------------------------------------------------------------
    // Collect the truly unmatched after Pass 3.
    let remaining_left: Vec<usize> = (0..left.len())
        .filter(|i| !left_matched.contains(i))
        .collect();
    let remaining_right: Vec<usize> = (0..right.len())
        .filter(|i| !right_matched.contains(i))
        .collect();

    // Run LCS on remaining_left x remaining_right using normalized canonical_text
    // as the comparison key.
    let lcs_pairs = lcs_align(&remaining_left, &remaining_right, left, right);
    for (li, ri) in lcs_pairs {
        let sim = block_similarity(&left[li], &right[ri]);
        if sim >= SIMILARITY_THRESHOLD {
            let is_move = left[li].structural_path != right[ri].structural_path
                && sim >= MOVE_THRESHOLD;
            pairs.push((li, ri, sim, is_move));
            left_matched.insert(li);
            right_matched.insert(ri);
        }
    }

    // -----------------------------------------------------------------------
    // Assemble final output in left-document order, interleaving insertions
    // -----------------------------------------------------------------------
    // Build a lookup from left_idx → (right_idx, sim, is_move).
    let pair_map: HashMap<usize, (usize, f64, bool)> = pairs
        .iter()
        .map(|&(l, r, s, m)| (l, (r, s, m)))
        .collect();

    // Track which right blocks have been emitted.
    let mut right_emitted: HashSet<usize> = HashSet::new();
    let mut result: Vec<BlockAlignment> = Vec::new();

    // We'll emit insertions for unmatched right blocks that appear before
    // each matched right block (i.e., maintain relative right-document order).
    // Build sorted list of matched right indices so we know insertion points.
    let mut matched_right_sorted: Vec<usize> = pair_map.values().map(|&(r, _, _)| r).collect();
    matched_right_sorted.sort_unstable();

    // Emit in left-document traversal order.
    for li in 0..left.len() {
        if let Some(&(ri, sim, is_move)) = pair_map.get(&li) {
            // Before emitting this matched pair, emit any right blocks that
            // come before ri and have not been matched (insertions).
            emit_insertions_before(ri, right, &mut right_emitted, &right_matched, &mut result);
            right_emitted.insert(ri);

            let alignment = if is_move {
                BlockAlignment::Moved { left: li, right: ri, similarity: sim }
            } else {
                BlockAlignment::Matched { left: li, right: ri, similarity: sim }
            };
            result.push(alignment);
        } else {
            // This left block has no match → deleted.
            result.push(BlockAlignment::DeletedLeft { left: li });
        }
    }

    // Emit any remaining unmatched right blocks (pure insertions at the end).
    for ri in 0..right.len() {
        if !right_emitted.contains(&ri) && !right_matched.contains(&ri) {
            result.push(BlockAlignment::InsertedRight { right: ri });
        }
    }
    // Also emit right blocks that were in right_matched but not in pair_map
    // (shouldn't happen, but be defensive).
    for ri in 0..right.len() {
        if !right_emitted.contains(&ri) && right_matched.contains(&ri) {
            // This means it was matched but left side was already processed.
            // Could be a duplicate anchor situation; skip.
        }
    }

    result
}

/// Compute the Jaccard similarity between two blocks using their token sets.
///
/// The Jaccard index is `|A ∩ B| / |A ∪ B|` where A and B are the
/// multisets of normalized token texts from each block.
///
/// Returns 0.0 for blocks with no tokens, 1.0 for identical token sets.
pub fn block_similarity(left: &Block, right: &Block) -> f64 {
    // If both blocks have tokens, use them; otherwise fall back to
    // tokenizing the canonical text on the fly.
    let left_tokens = token_set(left);
    let right_tokens = token_set(right);

    if left_tokens.is_empty() && right_tokens.is_empty() {
        // Two empty blocks are identical.
        return 1.0;
    }
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return 0.0;
    }

    // Use multiset Jaccard: count each normalized token.
    let mut left_counts: HashMap<&str, usize> = HashMap::new();
    for t in &left_tokens {
        *left_counts.entry(t.as_str()).or_insert(0) += 1;
    }
    let mut right_counts: HashMap<&str, usize> = HashMap::new();
    for t in &right_tokens {
        *right_counts.entry(t.as_str()).or_insert(0) += 1;
    }

    // Intersection: sum of min counts for tokens present in both.
    let mut intersection: usize = 0;
    for (tok, &lc) in &left_counts {
        if let Some(&rc) = right_counts.get(tok) {
            intersection += lc.min(rc);
        }
    }

    // Union = |L| + |R| - |intersection| (multiset union).
    let total = left_tokens.len() + right_tokens.len() - intersection;
    if total == 0 {
        1.0
    } else {
        intersection as f64 / total as f64
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Extract normalized token strings from a block.
/// If the block's token list is populated, use that; otherwise tokenize
/// the canonical text on the fly.
fn token_set(block: &Block) -> Vec<String> {
    if !block.tokens.is_empty() {
        block
            .tokens
            .iter()
            .filter(|t| !matches!(t.kind, rt_core::TokenKind::Whitespace))
            .map(|t| t.normalized.clone())
            .collect()
    } else {
        crate::tokenize::tokenize(&block.canonical_text)
            .into_iter()
            .map(|t| t.normalized)
            .collect()
    }
}

/// Emit `InsertedRight` entries for unmatched right blocks with index < `before_ri`.
/// Updates `emitted` so that each insertion is only emitted once.
fn emit_insertions_before(
    before_ri: usize,
    _right: &[Block],
    emitted: &mut HashSet<usize>,
    matched: &HashSet<usize>,
    result: &mut Vec<BlockAlignment>,
) {
    for ri in 0..before_ri {
        if !emitted.contains(&ri) && !matched.contains(&ri) {
            result.push(BlockAlignment::InsertedRight { right: ri });
            emitted.insert(ri);
        }
    }
}

/// Longest Common Subsequence alignment on two index sequences.
///
/// Uses normalized canonical text equality as the match predicate.
/// Returns a list of (left_idx, right_idx) pairs.
fn lcs_align(
    left_indices: &[usize],
    right_indices: &[usize],
    left: &[Block],
    right: &[Block],
) -> Vec<(usize, usize)> {
    let n = left_indices.len();
    let m = right_indices.len();
    if n == 0 || m == 0 {
        return Vec::new();
    }

    // DP table: dp[i][j] = LCS length for left[..i], right[..j]
    let mut dp = vec![vec![0usize; m + 1]; n + 1];

    for i in 1..=n {
        for j in 1..=m {
            let li = left_indices[i - 1];
            let ri = right_indices[j - 1];
            if left[li].canonical_text == right[ri].canonical_text {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    // Backtrack to recover pairs.
    let mut pairs = Vec::new();
    let mut i = n;
    let mut j = m;
    while i > 0 && j > 0 {
        let li = left_indices[i - 1];
        let ri = right_indices[j - 1];
        if left[li].canonical_text == right[ri].canonical_text {
            pairs.push((li, ri));
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] >= dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    pairs.reverse();
    pairs
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rt_core::{Block, BlockType};
    use uuid::Uuid;

    fn doc_id() -> Uuid {
        Uuid::new_v4()
    }

    fn make_block(doc: Uuid, path: &str, text: &str, idx: i32) -> Block {
        Block::new(BlockType::Clause, path, text, text, None, doc, idx)
    }

    #[test]
    fn exact_path_match() {
        let doc = doc_id();
        let left = vec![make_block(doc, "1.1", "the borrower shall repay", 0)];
        let right = vec![make_block(doc, "1.1", "the borrower shall repay", 0)];
        let alignments = align_blocks(&left, &right);
        assert_eq!(alignments.len(), 1);
        assert!(matches!(alignments[0], BlockAlignment::Matched { left: 0, right: 0, .. }));
    }

    #[test]
    fn insertion_detected() {
        let doc = doc_id();
        let left: Vec<Block> = vec![];
        let right = vec![make_block(doc, "1.1", "new clause text", 0)];
        let alignments = align_blocks(&left, &right);
        assert_eq!(alignments.len(), 1);
        assert!(matches!(alignments[0], BlockAlignment::InsertedRight { right: 0 }));
    }

    #[test]
    fn deletion_detected() {
        let doc = doc_id();
        let left = vec![make_block(doc, "1.1", "old clause text", 0)];
        let right: Vec<Block> = vec![];
        let alignments = align_blocks(&left, &right);
        assert_eq!(alignments.len(), 1);
        assert!(matches!(alignments[0], BlockAlignment::DeletedLeft { left: 0 }));
    }

    #[test]
    fn anchor_match_after_path_change() {
        let doc = doc_id();
        // Same content, different structural paths → anchor should still match.
        let mut left_block = make_block(doc, "1.1", "the borrower shall repay the full amount", 0);
        let right_block = make_block(doc, "2.1", "the borrower shall repay the full amount", 0);
        // Make them share the same anchor by using Block::new (which auto-computes anchor).
        // They won't share it unless we patch, but similarity will be 1.0 → move.
        left_block.anchor_signature = right_block.anchor_signature.clone();

        let left = vec![left_block];
        let right = vec![right_block];
        let alignments = align_blocks(&left, &right);
        // Should produce either Matched or Moved (anchor matched but path differs → Moved).
        assert_eq!(alignments.len(), 1);
        assert!(matches!(
            alignments[0],
            BlockAlignment::Matched { .. } | BlockAlignment::Moved { .. }
        ));
    }

    #[test]
    fn move_detection_via_similarity() {
        let doc = doc_id();
        // Left: block at path "1.1"; Right: same content at path "3.1".
        let left = vec![make_block(
            doc,
            "1.1",
            "the lender may assign its rights under this agreement",
            0,
        )];
        let right = vec![make_block(
            doc,
            "3.1",
            "the lender may assign its rights under this agreement",
            0,
        )];
        let alignments = align_blocks(&left, &right);
        assert_eq!(alignments.len(), 1);
        // Similarity = 1.0 ≥ 0.85, path differs → Moved.
        assert!(matches!(alignments[0], BlockAlignment::Moved { .. }));
    }

    #[test]
    fn multiple_blocks_ordered() {
        let doc = doc_id();
        let left = vec![
            make_block(doc, "1.1", "definitions clause text here", 0),
            make_block(doc, "1.2", "payment obligations stated here", 1),
            make_block(doc, "1.3", "termination rights described here", 2),
        ];
        let right = vec![
            make_block(doc, "1.1", "definitions clause text here", 0),
            make_block(doc, "1.2", "payment obligations stated here modified", 1),
            make_block(doc, "1.4", "new indemnity clause added right here", 2),
            make_block(doc, "1.3", "termination rights described here", 3),
        ];
        let alignments = align_blocks(&left, &right);
        // Expect: 1.1 matched, 1.2 matched (modified), 1.3 matched (or moved), plus insertion.
        assert!(!alignments.is_empty());
        let inserted = alignments
            .iter()
            .filter(|a| matches!(a, BlockAlignment::InsertedRight { .. }))
            .count();
        assert_eq!(inserted, 1, "should detect exactly one insertion");
    }

    #[test]
    fn block_similarity_identical() {
        let doc = doc_id();
        let b1 = make_block(doc, "1.1", "the borrower shall repay", 0);
        let b2 = make_block(doc, "1.1", "the borrower shall repay", 0);
        let sim = block_similarity(&b1, &b2);
        assert!((sim - 1.0).abs() < 1e-9, "identical blocks should have similarity 1.0");
    }

    #[test]
    fn block_similarity_disjoint() {
        let doc = doc_id();
        let b1 = make_block(doc, "1.1", "alpha beta gamma", 0);
        let b2 = make_block(doc, "1.2", "delta epsilon zeta", 0);
        let sim = block_similarity(&b1, &b2);
        assert!(sim < 0.1, "disjoint blocks should have near-zero similarity");
    }

    #[test]
    fn block_similarity_partial_overlap() {
        let doc = doc_id();
        let b1 = make_block(doc, "1.1", "the borrower shall repay the loan", 0);
        let b2 = make_block(doc, "1.2", "the borrower shall repay the principal", 0);
        let sim = block_similarity(&b1, &b2);
        // Most tokens overlap; should be well above threshold.
        assert!(sim > 0.5, "partially overlapping blocks: got {}", sim);
    }

    #[test]
    fn both_empty_blocks() {
        let doc = doc_id();
        let b1 = make_block(doc, "1.1", "", 0);
        let b2 = make_block(doc, "1.1", "", 0);
        let sim = block_similarity(&b1, &b2);
        assert!((sim - 1.0).abs() < 1e-9, "two empty blocks are identical");
    }
}
