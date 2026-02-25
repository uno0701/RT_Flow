use crate::event::EventType;
use crate::state::WorkflowState;

/// Validate that `event` is a legal transition from `current` and return the
/// resulting `WorkflowState`.  Returns `Err(InvalidInput)` when the
/// combination is not permitted.
pub fn validate_transition(
    current: &WorkflowState,
    event: &EventType,
) -> Result<WorkflowState, rt_core::RtError> {
    let next = match (current, event) {
        // Draft transitions
        (WorkflowState::Draft, EventType::WorkflowCreated) => WorkflowState::Draft,
        (WorkflowState::Draft, EventType::CompareStarted) => WorkflowState::CompareRunning,
        (WorkflowState::Draft, EventType::WorkflowAborted) => WorkflowState::Aborted,

        // CompareRunning transitions
        (WorkflowState::CompareRunning, EventType::CompareCompleted) => WorkflowState::FlowCreated,

        // FlowCreated transitions
        (WorkflowState::FlowCreated, EventType::ReviewStarted) => WorkflowState::InReview,

        // InReview transitions
        (WorkflowState::InReview, EventType::ReviewerAssigned) => WorkflowState::InReview,
        (WorkflowState::InReview, EventType::DeltaSubmitted) => WorkflowState::InReview,
        (WorkflowState::InReview, EventType::ReviewClosed) => WorkflowState::ReviewClosed,
        (WorkflowState::InReview, EventType::WorkflowAborted) => WorkflowState::Aborted,

        // ReviewClosed transitions
        (WorkflowState::ReviewClosed, EventType::EditCompilationStarted) => {
            WorkflowState::CompilingEdits
        }
        (WorkflowState::ReviewClosed, EventType::WorkflowAborted) => WorkflowState::Aborted,

        // CompilingEdits transitions
        (WorkflowState::CompilingEdits, EventType::EditCompilationCompleted) => {
            WorkflowState::ReadyForFinalization
        }

        // ReadyForFinalization transitions
        (WorkflowState::ReadyForFinalization, EventType::WorkflowCompleted) => {
            WorkflowState::Completed
        }

        // Terminal states – nothing is legal
        (WorkflowState::Completed, _) => {
            return Err(rt_core::RtError::InvalidInput(format!(
                "workflow is already COMPLETED; event '{}' is not permitted",
                event.as_str()
            )));
        }
        (WorkflowState::Aborted, _) => {
            return Err(rt_core::RtError::InvalidInput(format!(
                "workflow is already ABORTED; event '{}' is not permitted",
                event.as_str()
            )));
        }

        // All other combinations are illegal
        (state, ev) => {
            return Err(rt_core::RtError::InvalidInput(format!(
                "illegal transition: event '{}' is not permitted in state '{}'",
                ev.as_str(),
                state.as_str()
            )));
        }
    };
    Ok(next)
}

/// Return the set of events that are legally applicable to `state`.
pub fn legal_transitions(state: &WorkflowState) -> Vec<EventType> {
    match state {
        WorkflowState::Draft => vec![
            EventType::WorkflowCreated,
            EventType::CompareStarted,
            EventType::WorkflowAborted,
        ],
        WorkflowState::CompareRunning => vec![EventType::CompareCompleted],
        WorkflowState::FlowCreated => vec![EventType::ReviewStarted],
        WorkflowState::InReview => vec![
            EventType::ReviewerAssigned,
            EventType::DeltaSubmitted,
            EventType::ReviewClosed,
            EventType::WorkflowAborted,
        ],
        WorkflowState::ReviewClosed => vec![
            EventType::EditCompilationStarted,
            EventType::WorkflowAborted,
        ],
        WorkflowState::CompilingEdits => vec![EventType::EditCompilationCompleted],
        WorkflowState::ReadyForFinalization => vec![EventType::WorkflowCompleted],
        WorkflowState::Completed => vec![],
        WorkflowState::Aborted => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: assert a transition succeeds and yields the expected state.
    fn ok(current: WorkflowState, event: EventType, expected: WorkflowState) {
        let result = validate_transition(&current, &event);
        assert!(
            result.is_ok(),
            "expected Ok for ({:?}, {:?}), got {:?}",
            current,
            event,
            result
        );
        assert_eq!(
            result.unwrap(),
            expected,
            "wrong next state for ({:?}, {:?})",
            current,
            event
        );
    }

    // Helper: assert a transition fails.
    fn err(current: WorkflowState, event: EventType) {
        let result = validate_transition(&current, &event);
        assert!(
            result.is_err(),
            "expected Err for ({:?}, {:?}), got Ok({:?})",
            current,
            event,
            result.ok()
        );
    }

    #[test]
    fn draft_workflow_created_stays_draft() {
        ok(
            WorkflowState::Draft,
            EventType::WorkflowCreated,
            WorkflowState::Draft,
        );
    }

    #[test]
    fn draft_compare_started_becomes_compare_running() {
        ok(
            WorkflowState::Draft,
            EventType::CompareStarted,
            WorkflowState::CompareRunning,
        );
    }

    #[test]
    fn draft_aborted_becomes_aborted() {
        ok(
            WorkflowState::Draft,
            EventType::WorkflowAborted,
            WorkflowState::Aborted,
        );
    }

    #[test]
    fn compare_running_completed_becomes_flow_created() {
        ok(
            WorkflowState::CompareRunning,
            EventType::CompareCompleted,
            WorkflowState::FlowCreated,
        );
    }

    #[test]
    fn flow_created_review_started_becomes_in_review() {
        ok(
            WorkflowState::FlowCreated,
            EventType::ReviewStarted,
            WorkflowState::InReview,
        );
    }

    #[test]
    fn in_review_side_events_stay_in_review() {
        ok(
            WorkflowState::InReview,
            EventType::ReviewerAssigned,
            WorkflowState::InReview,
        );
        ok(
            WorkflowState::InReview,
            EventType::DeltaSubmitted,
            WorkflowState::InReview,
        );
    }

    #[test]
    fn in_review_closed_becomes_review_closed() {
        ok(
            WorkflowState::InReview,
            EventType::ReviewClosed,
            WorkflowState::ReviewClosed,
        );
    }

    #[test]
    fn in_review_abort_becomes_aborted() {
        ok(
            WorkflowState::InReview,
            EventType::WorkflowAborted,
            WorkflowState::Aborted,
        );
    }

    #[test]
    fn review_closed_compilation_started_becomes_compiling() {
        ok(
            WorkflowState::ReviewClosed,
            EventType::EditCompilationStarted,
            WorkflowState::CompilingEdits,
        );
    }

    #[test]
    fn review_closed_abort_becomes_aborted() {
        ok(
            WorkflowState::ReviewClosed,
            EventType::WorkflowAborted,
            WorkflowState::Aborted,
        );
    }

    #[test]
    fn compiling_edits_completed_becomes_ready_for_finalization() {
        ok(
            WorkflowState::CompilingEdits,
            EventType::EditCompilationCompleted,
            WorkflowState::ReadyForFinalization,
        );
    }

    #[test]
    fn ready_for_finalization_completed_becomes_completed() {
        ok(
            WorkflowState::ReadyForFinalization,
            EventType::WorkflowCompleted,
            WorkflowState::Completed,
        );
    }

    #[test]
    fn completed_any_event_is_illegal() {
        err(WorkflowState::Completed, EventType::WorkflowCreated);
        err(WorkflowState::Completed, EventType::CompareStarted);
        err(WorkflowState::Completed, EventType::WorkflowAborted);
        err(WorkflowState::Completed, EventType::WorkflowCompleted);
    }

    #[test]
    fn aborted_any_event_is_illegal() {
        err(WorkflowState::Aborted, EventType::WorkflowCreated);
        err(WorkflowState::Aborted, EventType::CompareStarted);
        err(WorkflowState::Aborted, EventType::WorkflowAborted);
    }

    #[test]
    fn illegal_transitions_return_err() {
        // compare_running cannot receive review_started
        err(WorkflowState::CompareRunning, EventType::ReviewStarted);
        // flow_created cannot receive compare_completed
        err(WorkflowState::FlowCreated, EventType::CompareCompleted);
        // compiling_edits cannot receive review_closed
        err(WorkflowState::CompilingEdits, EventType::ReviewClosed);
    }

    #[test]
    fn legal_transitions_coverage() {
        assert!(legal_transitions(&WorkflowState::Draft).contains(&EventType::CompareStarted));
        assert!(
            legal_transitions(&WorkflowState::Completed).is_empty(),
            "Completed should have no legal transitions"
        );
        assert!(
            legal_transitions(&WorkflowState::Aborted).is_empty(),
            "Aborted should have no legal transitions"
        );
    }
}
