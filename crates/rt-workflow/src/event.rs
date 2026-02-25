use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    WorkflowCreated,
    CompareStarted,
    CompareCompleted,
    FlowCreated,
    ReviewStarted,
    ReviewerAssigned,
    DeltaSubmitted,
    ReviewClosed,
    EditCompilationStarted,
    EditCompilationCompleted,
    FinalizationReady,
    WorkflowCompleted,
    WorkflowAborted,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::WorkflowCreated => "workflow_created",
            EventType::CompareStarted => "compare_started",
            EventType::CompareCompleted => "compare_completed",
            EventType::FlowCreated => "flow_created",
            EventType::ReviewStarted => "review_started",
            EventType::ReviewerAssigned => "reviewer_assigned",
            EventType::DeltaSubmitted => "delta_submitted",
            EventType::ReviewClosed => "review_closed",
            EventType::EditCompilationStarted => "edit_compilation_started",
            EventType::EditCompilationCompleted => "edit_compilation_completed",
            EventType::FinalizationReady => "finalization_ready",
            EventType::WorkflowCompleted => "workflow_completed",
            EventType::WorkflowAborted => "workflow_aborted",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, rt_core::RtError> {
        match s {
            "workflow_created" => Ok(EventType::WorkflowCreated),
            "compare_started" => Ok(EventType::CompareStarted),
            "compare_completed" => Ok(EventType::CompareCompleted),
            "flow_created" => Ok(EventType::FlowCreated),
            "review_started" => Ok(EventType::ReviewStarted),
            "reviewer_assigned" => Ok(EventType::ReviewerAssigned),
            "delta_submitted" => Ok(EventType::DeltaSubmitted),
            "review_closed" => Ok(EventType::ReviewClosed),
            "edit_compilation_started" => Ok(EventType::EditCompilationStarted),
            "edit_compilation_completed" => Ok(EventType::EditCompilationCompleted),
            "finalization_ready" => Ok(EventType::FinalizationReady),
            "workflow_completed" => Ok(EventType::WorkflowCompleted),
            "workflow_aborted" => Ok(EventType::WorkflowAborted),
            other => Err(rt_core::RtError::InvalidInput(format!(
                "unknown event type: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowEvent {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub event_type: EventType,
    pub actor: String,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub seq: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_type_as_str_round_trips() {
        let types = [
            EventType::WorkflowCreated,
            EventType::CompareStarted,
            EventType::CompareCompleted,
            EventType::FlowCreated,
            EventType::ReviewStarted,
            EventType::ReviewerAssigned,
            EventType::DeltaSubmitted,
            EventType::ReviewClosed,
            EventType::EditCompilationStarted,
            EventType::EditCompilationCompleted,
            EventType::FinalizationReady,
            EventType::WorkflowCompleted,
            EventType::WorkflowAborted,
        ];
        for et in &types {
            let s = et.as_str();
            let parsed = EventType::from_str(s).expect("round-trip should succeed");
            assert_eq!(*et, parsed, "round-trip failed for {s}");
        }
    }

    #[test]
    fn event_type_from_str_unknown_returns_err() {
        let result = EventType::from_str("not_an_event");
        assert!(result.is_err());
    }
}
