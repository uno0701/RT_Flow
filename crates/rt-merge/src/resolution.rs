use rt_core::RtError;

use crate::conflict::{ConflictResolution, MergeConflict};

// ---------------------------------------------------------------------------
// Resolution state machine
// ---------------------------------------------------------------------------

/// Validate a proposed resolution transition and return an error if the
/// transition is illegal.
///
/// Legal transitions:
/// ```text
/// Pending → AcceptedBase
/// Pending → AcceptedIncoming
/// Pending → Manual
/// ```
///
/// Illegal transitions:
/// - Any resolved state (`AcceptedBase`, `AcceptedIncoming`, `Manual`) →
///   `Pending` (cannot revert to unresolved).
/// - A state → itself (no-op transitions are not permitted; the caller must
///   apply a distinct target).
pub fn validate_resolution(
    current: &ConflictResolution,
    target: &ConflictResolution,
) -> Result<(), RtError> {
    // A no-op (same state → same state) is not a valid transition.
    if current == target {
        return Err(RtError::InvalidInput(format!(
            "conflict is already in the '{}' state; target resolution must differ",
            resolution_name(current)
        )));
    }

    match (current, target) {
        // Legal: Pending can transition to any resolved state.
        (ConflictResolution::Pending, ConflictResolution::AcceptedBase)
        | (ConflictResolution::Pending, ConflictResolution::AcceptedIncoming)
        | (ConflictResolution::Pending, ConflictResolution::Manual) => Ok(()),

        // Illegal: once resolved, cannot revert to Pending.
        (_, ConflictResolution::Pending) => Err(RtError::InvalidInput(format!(
            "cannot revert conflict from '{}' back to 'pending'; \
             once resolved a conflict cannot be unresolved",
            resolution_name(current)
        ))),

        // Illegal: resolved → different resolved state (re-resolution not
        // permitted without explicit manual override pathway).
        (current_state, target_state) => Err(RtError::InvalidInput(format!(
            "cannot transition conflict from '{}' to '{}'; \
             only pending conflicts may be resolved",
            resolution_name(current_state),
            resolution_name(target_state)
        ))),
    }
}

/// Return `true` when every conflict in `conflicts` has been resolved
/// (i.e., none has `resolution == Pending`).
pub fn all_resolved(conflicts: &[MergeConflict]) -> bool {
    conflicts.iter().all(|c| c.resolution != ConflictResolution::Pending)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn resolution_name(r: &ConflictResolution) -> &'static str {
    match r {
        ConflictResolution::Pending => "pending",
        ConflictResolution::AcceptedBase => "accepted_base",
        ConflictResolution::AcceptedIncoming => "accepted_incoming",
        ConflictResolution::Manual => "manual",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conflict::{ConflictResolution, ConflictType, MergeConflict};
    use uuid::Uuid;

    fn pending_conflict() -> MergeConflict {
        MergeConflict::new(
            Uuid::new_v4(),
            ConflictType::ContentOverlap,
            Some("base text".to_string()),
            Some("incoming text".to_string()),
        )
    }

    fn resolved_conflict(res: ConflictResolution) -> MergeConflict {
        let mut c = pending_conflict();
        c.resolution = res;
        c
    }

    // -----------------------------------------------------------------------
    // validate_resolution: legal transitions
    // -----------------------------------------------------------------------

    #[test]
    fn pending_to_accepted_base_is_legal() {
        assert!(
            validate_resolution(&ConflictResolution::Pending, &ConflictResolution::AcceptedBase)
                .is_ok()
        );
    }

    #[test]
    fn pending_to_accepted_incoming_is_legal() {
        assert!(
            validate_resolution(
                &ConflictResolution::Pending,
                &ConflictResolution::AcceptedIncoming
            )
            .is_ok()
        );
    }

    #[test]
    fn pending_to_manual_is_legal() {
        assert!(
            validate_resolution(&ConflictResolution::Pending, &ConflictResolution::Manual).is_ok()
        );
    }

    // -----------------------------------------------------------------------
    // validate_resolution: illegal transitions
    // -----------------------------------------------------------------------

    #[test]
    fn accepted_base_to_pending_is_illegal() {
        let result = validate_resolution(
            &ConflictResolution::AcceptedBase,
            &ConflictResolution::Pending,
        );
        assert!(result.is_err());
    }

    #[test]
    fn accepted_incoming_to_pending_is_illegal() {
        let result = validate_resolution(
            &ConflictResolution::AcceptedIncoming,
            &ConflictResolution::Pending,
        );
        assert!(result.is_err());
    }

    #[test]
    fn manual_to_pending_is_illegal() {
        let result =
            validate_resolution(&ConflictResolution::Manual, &ConflictResolution::Pending);
        assert!(result.is_err());
    }

    #[test]
    fn same_state_transition_is_illegal() {
        // Pending → Pending is a no-op and must be rejected.
        let result =
            validate_resolution(&ConflictResolution::Pending, &ConflictResolution::Pending);
        assert!(result.is_err());
    }

    #[test]
    fn resolved_to_different_resolved_is_illegal() {
        let result = validate_resolution(
            &ConflictResolution::AcceptedBase,
            &ConflictResolution::AcceptedIncoming,
        );
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // all_resolved tests
    // -----------------------------------------------------------------------

    #[test]
    fn all_resolved_returns_true_when_empty() {
        assert!(all_resolved(&[]));
    }

    #[test]
    fn all_resolved_returns_true_when_all_resolved() {
        let conflicts = vec![
            resolved_conflict(ConflictResolution::AcceptedBase),
            resolved_conflict(ConflictResolution::AcceptedIncoming),
            resolved_conflict(ConflictResolution::Manual),
        ];
        assert!(all_resolved(&conflicts));
    }

    #[test]
    fn all_resolved_returns_false_when_pending_remains() {
        let conflicts = vec![
            resolved_conflict(ConflictResolution::AcceptedBase),
            pending_conflict(), // still pending
        ];
        assert!(!all_resolved(&conflicts));
    }

    #[test]
    fn all_resolved_returns_false_when_all_pending() {
        let conflicts = vec![pending_conflict(), pending_conflict()];
        assert!(!all_resolved(&conflicts));
    }
}
