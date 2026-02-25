use crate::block::BlockType;
use crate::hash::sha256_hex;

/// Primary anchor signature.
///
/// Computed as SHA256 of the concatenation:
///   `{block_type_str}|{structural_path}|{first_128_chars_of_canonical_text}`
///
/// Using only the first 128 characters of the canonical text keeps the anchor
/// stable through minor textual edits while still discriminating between
/// structurally co-located blocks with meaningfully different content.
pub fn compute_anchor_signature(
    block_type: &BlockType,
    structural_path: &str,
    canonical_text: &str,
) -> String {
    let type_str = block_type_str(block_type);
    let prefix: String = canonical_text.chars().take(128).collect();
    let payload = format!("{}|{}|{}", type_str, structural_path, prefix);
    sha256_hex(&payload)
}

/// Secondary discriminator — SHA256 of the full canonical text.
///
/// Use this when you need to detect even minor textual changes that the
/// anchor (which only hashes the first 128 chars) might miss.
pub fn compute_full_text_hash(canonical_text: &str) -> String {
    sha256_hex(canonical_text)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn block_type_str(bt: &BlockType) -> &'static str {
    match bt {
        BlockType::Section => "section",
        BlockType::Clause => "clause",
        BlockType::Subclause => "subclause",
        BlockType::Paragraph => "paragraph",
        BlockType::Table => "table",
        BlockType::TableRow => "table_row",
        BlockType::TableCell => "table_cell",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_is_deterministic() {
        let sig1 = compute_anchor_signature(
            &BlockType::Clause,
            "1.2(a)",
            "The borrower shall repay the loan.",
        );
        let sig2 = compute_anchor_signature(
            &BlockType::Clause,
            "1.2(a)",
            "The borrower shall repay the loan.",
        );
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn anchor_differs_by_type() {
        let sig_clause = compute_anchor_signature(
            &BlockType::Clause,
            "1.2(a)",
            "Same text",
        );
        let sig_para = compute_anchor_signature(
            &BlockType::Paragraph,
            "1.2(a)",
            "Same text",
        );
        assert_ne!(sig_clause, sig_para);
    }

    #[test]
    fn anchor_differs_by_path() {
        let sig1 = compute_anchor_signature(&BlockType::Clause, "1.1", "Text");
        let sig2 = compute_anchor_signature(&BlockType::Clause, "1.2", "Text");
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn anchor_stable_after_128_chars() {
        let base: String = "a".repeat(200);
        let extended = format!("{}{}", base, "b".repeat(50));

        // Both have the same first 128 chars and the same path/type.
        let sig1 = compute_anchor_signature(&BlockType::Paragraph, "2.1", &base);
        let sig2 = compute_anchor_signature(&BlockType::Paragraph, "2.1", &extended);
        // First 128 chars are identical, so anchors should match.
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn full_text_hash_detects_tail_change() {
        let base: String = "a".repeat(200);
        let extended = format!("{}{}", base, "extra");
        assert_ne!(
            compute_full_text_hash(&base),
            compute_full_text_hash(&extended)
        );
    }
}
