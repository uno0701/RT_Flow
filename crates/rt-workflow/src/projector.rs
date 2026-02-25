use crate::event::WorkflowEvent;
use crate::state::Workflow;
use crate::validator::validate_transition;

/// Replay `events` onto `workflow` (sorted by `seq`) and return the resulting
/// `Workflow`.  The original `workflow` is treated as the snapshot to apply
/// events on top of; it is not mutated.
///
/// Returns `Err` if any event in the sequence would cause an illegal
/// state transition.
pub fn project_state(
    workflow: &Workflow,
    events: &[WorkflowEvent],
) -> Result<Workflow, rt_core::RtError> {
    let mut current = workflow.clone();
    let mut sorted_events = events.to_vec();
    sorted_events.sort_by_key(|e| e.seq);
    for event in &sorted_events {
        let new_state = validate_transition(&current.state, &event.event_type)?;
        current.state = new_state;
        current.updated_at = event.created_at;
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventType;
    use crate::state::WorkflowState;
    use chrono::Utc;
    use uuid::Uuid;

    fn make_event(workflow_id: Uuid, seq: i64, event_type: EventType) -> WorkflowEvent {
        WorkflowEvent {
            id: Uuid::new_v4(),
            workflow_id,
            event_type,
            actor: "system".to_string(),
            payload: serde_json::Value::Null,
            created_at: Utc::now(),
            seq,
        }
    }

    fn base_workflow() -> Workflow {
        let doc_id = Uuid::new_v4();
        Workflow::new(doc_id, "initiator")
    }

    #[test]
    fn empty_events_returns_unchanged_workflow() {
        let wf = base_workflow();
        let projected = project_state(&wf, &[]).expect("should succeed");
        assert_eq!(projected.state, WorkflowState::Draft);
        assert_eq!(projected.id, wf.id);
    }

    #[test]
    fn full_lifecycle_replay() {
        let wf = base_workflow();
        let wid = wf.id;

        let events = vec![
            make_event(wid, 1, EventType::WorkflowCreated),
            make_event(wid, 2, EventType::CompareStarted),
            make_event(wid, 3, EventType::CompareCompleted),
            make_event(wid, 4, EventType::ReviewStarted),
            make_event(wid, 5, EventType::ReviewerAssigned),
            make_event(wid, 6, EventType::DeltaSubmitted),
            make_event(wid, 7, EventType::ReviewClosed),
            make_event(wid, 8, EventType::EditCompilationStarted),
            make_event(wid, 9, EventType::EditCompilationCompleted),
            make_event(wid, 10, EventType::WorkflowCompleted),
        ];

        let projected = project_state(&wf, &events).expect("full lifecycle should succeed");
        assert_eq!(projected.state, WorkflowState::Completed);
    }

    #[test]
    fn events_are_sorted_by_seq_before_replay() {
        let wf = base_workflow();
        let wid = wf.id;

        // Provide out-of-order events; projector must sort them.
        let events = vec![
            make_event(wid, 2, EventType::CompareStarted),
            make_event(wid, 1, EventType::WorkflowCreated),
        ];

        let projected = project_state(&wf, &events).expect("should handle unsorted events");
        assert_eq!(projected.state, WorkflowState::CompareRunning);
    }

    #[test]
    fn invalid_mid_sequence_event_returns_err() {
        let wf = base_workflow();
        let wid = wf.id;

        // WorkflowCreated is fine (Draft → Draft), but then ReviewStarted is
        // illegal from Draft.
        let events = vec![
            make_event(wid, 1, EventType::WorkflowCreated),
            make_event(wid, 2, EventType::ReviewStarted),
        ];

        let result = project_state(&wf, &events);
        assert!(
            result.is_err(),
            "should fail on illegal transition in mid-sequence"
        );
    }

    #[test]
    fn abort_from_draft_terminates_in_aborted() {
        let wf = base_workflow();
        let wid = wf.id;
        let events = vec![make_event(wid, 1, EventType::WorkflowAborted)];
        let projected = project_state(&wf, &events).expect("abort from Draft should succeed");
        assert_eq!(projected.state, WorkflowState::Aborted);
    }
}
