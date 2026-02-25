use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkflowState {
    Draft,
    CompareRunning,
    FlowCreated,
    InReview,
    ReviewClosed,
    CompilingEdits,
    ReadyForFinalization,
    Completed,
    Aborted,
}

impl WorkflowState {
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkflowState::Draft => "DRAFT",
            WorkflowState::CompareRunning => "COMPARE_RUNNING",
            WorkflowState::FlowCreated => "FLOW_CREATED",
            WorkflowState::InReview => "IN_REVIEW",
            WorkflowState::ReviewClosed => "REVIEW_CLOSED",
            WorkflowState::CompilingEdits => "COMPILING_EDITS",
            WorkflowState::ReadyForFinalization => "READY_FOR_FINALIZATION",
            WorkflowState::Completed => "COMPLETED",
            WorkflowState::Aborted => "ABORTED",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, rt_core::RtError> {
        match s {
            "DRAFT" => Ok(WorkflowState::Draft),
            "COMPARE_RUNNING" => Ok(WorkflowState::CompareRunning),
            "FLOW_CREATED" => Ok(WorkflowState::FlowCreated),
            "IN_REVIEW" => Ok(WorkflowState::InReview),
            "REVIEW_CLOSED" => Ok(WorkflowState::ReviewClosed),
            "COMPILING_EDITS" => Ok(WorkflowState::CompilingEdits),
            "READY_FOR_FINALIZATION" => Ok(WorkflowState::ReadyForFinalization),
            "COMPLETED" => Ok(WorkflowState::Completed),
            "ABORTED" => Ok(WorkflowState::Aborted),
            other => Err(rt_core::RtError::InvalidInput(format!(
                "unknown workflow state: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: Uuid,
    pub document_id: Uuid,
    pub state: WorkflowState,
    pub initiator_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Workflow {
    pub fn new(document_id: Uuid, initiator_id: &str) -> Self {
        let now = Utc::now();
        Workflow {
            id: Uuid::new_v4(),
            document_id,
            state: WorkflowState::Draft,
            initiator_id: initiator_id.to_string(),
            created_at: now,
            updated_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_round_trips() {
        let states = [
            WorkflowState::Draft,
            WorkflowState::CompareRunning,
            WorkflowState::FlowCreated,
            WorkflowState::InReview,
            WorkflowState::ReviewClosed,
            WorkflowState::CompilingEdits,
            WorkflowState::ReadyForFinalization,
            WorkflowState::Completed,
            WorkflowState::Aborted,
        ];
        for state in &states {
            let s = state.as_str();
            let parsed = WorkflowState::from_str(s).expect("round-trip should succeed");
            assert_eq!(*state, parsed, "round-trip failed for {s}");
        }
    }

    #[test]
    fn from_str_unknown_returns_err() {
        let result = WorkflowState::from_str("NOT_A_STATE");
        assert!(result.is_err());
    }

    #[test]
    fn new_workflow_starts_in_draft() {
        let doc_id = Uuid::new_v4();
        let wf = Workflow::new(doc_id, "user-1");
        assert_eq!(wf.state, WorkflowState::Draft);
        assert_eq!(wf.document_id, doc_id);
        assert_eq!(wf.initiator_id, "user-1");
    }
}
