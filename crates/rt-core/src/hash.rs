use sha2::{Digest, Sha256};

/// Generic SHA256 helper — returns a lowercase hex-encoded digest.
pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// SHA256 hash of `canonical_text`.
///
/// Always applied after normalization so identical semantic content
/// produces an identical hash regardless of source formatting.
pub fn compute_clause_hash(canonical_text: &str) -> String {
    sha256_hex(canonical_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_known_vector() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let digest = sha256_hex("");
        assert_eq!(
            digest,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn compute_clause_hash_is_deterministic() {
        let text = "The borrower shall repay the principal.";
        assert_eq!(compute_clause_hash(text), compute_clause_hash(text));
    }

    #[test]
    fn compute_clause_hash_differs_on_different_input() {
        assert_ne!(
            compute_clause_hash("foo"),
            compute_clause_hash("bar")
        );
    }
}
