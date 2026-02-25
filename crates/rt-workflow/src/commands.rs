use crate::event::{EventType, WorkflowEvent};
use crate::projector::project_state;
use crate::state::{Workflow, WorkflowState};
use chrono::Utc;
use rusqlite::Connection;
use uuid::Uuid;

pub struct WorkflowEngine;

impl WorkflowEngine {
    /// Insert a new workflow row into `workflows`, emit a `WorkflowCreated`
    /// event at seq=1, and return the resulting `Workflow`.
    pub fn create_workflow(
        conn: &Connection,
        document_id: Uuid,
        initiator_id: &str,
    ) -> Result<Workflow, rt_core::RtError> {
        let wf = Workflow::new(document_id, initiator_id);
        let now_str = wf.created_at.to_rfc3339();

        conn.execute(
            "INSERT INTO workflows (id, document_id, state, initiator_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                wf.id.to_string(),
                wf.document_id.to_string(),
                wf.state.as_str(),
                wf.initiator_id,
                now_str,
                now_str,
            ],
        )?;

        let event_id = Uuid::new_v4();
        conn.execute(
            "INSERT INTO workflow_events (id, workflow_id, event_type, actor, payload, created_at, seq)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                event_id.to_string(),
                wf.id.to_string(),
                EventType::WorkflowCreated.as_str(),
                initiator_id,
                "{}",
                now_str,
                1i64,
            ],
        )?;

        Ok(wf)
    }

    /// Validate and apply `event_type` to the workflow identified by
    /// `workflow_id`.  Persists the event and updates the workflow row.
    /// Returns the updated `Workflow`.
    pub fn submit_event(
        conn: &Connection,
        workflow_id: Uuid,
        event_type: EventType,
        actor: &str,
        payload: serde_json::Value,
    ) -> Result<Workflow, rt_core::RtError> {
        // Load current projected state.
        let current = Self::get_workflow(conn, workflow_id)?;

        // Validate the transition upfront so we fail fast without writing.
        let new_state = crate::validator::validate_transition(&current.state, &event_type)?;

        let seq = Self::next_seq(conn, workflow_id)?;
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let event_id = Uuid::new_v4();

        conn.execute(
            "INSERT INTO workflow_events (id, workflow_id, event_type, actor, payload, created_at, seq)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                event_id.to_string(),
                workflow_id.to_string(),
                event_type.as_str(),
                actor,
                payload.to_string(),
                now_str,
                seq,
            ],
        )?;

        conn.execute(
            "UPDATE workflows SET state = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![new_state.as_str(), now_str, workflow_id.to_string()],
        )?;

        // Return the full projected workflow (re-loads to include the new event).
        Self::get_workflow(conn, workflow_id)
    }

    /// Load a workflow by id, replay all of its events, and return the
    /// resulting `Workflow`.  Returns `RtError::NotFound` when no row exists.
    pub fn get_workflow(
        conn: &Connection,
        workflow_id: Uuid,
    ) -> Result<Workflow, rt_core::RtError> {
        let wf = conn
            .query_row(
                "SELECT id, document_id, state, initiator_id, created_at, updated_at
                 FROM workflows WHERE id = ?1",
                rusqlite::params![workflow_id.to_string()],
                |row| {
                    let id_str: String = row.get(0)?;
                    let doc_id_str: String = row.get(1)?;
                    let state_str: String = row.get(2)?;
                    let initiator_id: String = row.get(3)?;
                    let created_at_str: String = row.get(4)?;
                    let updated_at_str: String = row.get(5)?;
                    Ok((
                        id_str,
                        doc_id_str,
                        state_str,
                        initiator_id,
                        created_at_str,
                        updated_at_str,
                    ))
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => rt_core::RtError::NotFound(format!(
                    "workflow not found: {workflow_id}"
                )),
                other => rt_core::RtError::Database(other),
            })?;

        let id = Uuid::parse_str(&wf.0)
            .map_err(|e| rt_core::RtError::InvalidInput(e.to_string()))?;
        let document_id = Uuid::parse_str(&wf.1)
            .map_err(|e| rt_core::RtError::InvalidInput(e.to_string()))?;
        let state = WorkflowState::from_str(&wf.2)?;
        let created_at = wf
            .4
            .parse::<chrono::DateTime<Utc>>()
            .map_err(|e| rt_core::RtError::InvalidInput(e.to_string()))?;
        let updated_at = wf
            .5
            .parse::<chrono::DateTime<Utc>>()
            .map_err(|e| rt_core::RtError::InvalidInput(e.to_string()))?;

        let snapshot = Workflow {
            id,
            document_id,
            state,
            initiator_id: wf.3,
            created_at,
            updated_at,
        };

        // Replay events to arrive at the current projected state.
        // We use the snapshot directly because the DB row already stores the
        // current state; however we still replay to keep the projector as the
        // single source of truth for timestamps and state.
        // Build a base workflow at Draft so we replay from the very beginning.
        let base = Workflow {
            state: WorkflowState::Draft,
            updated_at: snapshot.created_at,
            ..snapshot.clone()
        };

        let events = Self::get_events(conn, workflow_id)?;
        project_state(&base, &events)
    }

    /// Return all events for `workflow_id` sorted by `seq` ascending.
    pub fn get_events(
        conn: &Connection,
        workflow_id: Uuid,
    ) -> Result<Vec<WorkflowEvent>, rt_core::RtError> {
        let mut stmt = conn.prepare(
            "SELECT id, workflow_id, event_type, actor, payload, created_at, seq
             FROM workflow_events
             WHERE workflow_id = ?1
             ORDER BY seq ASC",
        )?;

        let rows = stmt.query_map(rusqlite::params![workflow_id.to_string()], |row| {
            let id_str: String = row.get(0)?;
            let wid_str: String = row.get(1)?;
            let et_str: String = row.get(2)?;
            let actor: String = row.get(3)?;
            let payload_str: String = row.get(4)?;
            let created_at_str: String = row.get(5)?;
            let seq: i64 = row.get(6)?;
            Ok((
                id_str,
                wid_str,
                et_str,
                actor,
                payload_str,
                created_at_str,
                seq,
            ))
        })?;

        let mut events = Vec::new();
        for row in rows {
            let r = row?;
            let id = Uuid::parse_str(&r.0)
                .map_err(|e| rt_core::RtError::InvalidInput(e.to_string()))?;
            let wid = Uuid::parse_str(&r.1)
                .map_err(|e| rt_core::RtError::InvalidInput(e.to_string()))?;
            let event_type = EventType::from_str(&r.2)?;
            let payload: serde_json::Value = serde_json::from_str(&r.4)?;
            let created_at = r
                .5
                .parse::<chrono::DateTime<Utc>>()
                .map_err(|e| rt_core::RtError::InvalidInput(e.to_string()))?;
            events.push(WorkflowEvent {
                id,
                workflow_id: wid,
                event_type,
                actor: r.3,
                payload,
                created_at,
                seq: r.6,
            });
        }
        Ok(events)
    }

    /// Return the next available sequence number for `workflow_id`.
    fn next_seq(conn: &Connection, workflow_id: Uuid) -> Result<i64, rt_core::RtError> {
        let max: Option<i64> = conn.query_row(
            "SELECT MAX(seq) FROM workflow_events WHERE workflow_id = ?1",
            rusqlite::params![workflow_id.to_string()],
            |row| row.get(0),
        )?;
        Ok(max.unwrap_or(0) + 1)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rt_core::schema::run_migrations;
    use rusqlite::Connection;

    /// Insert a minimal documents row so that foreign-key constraints are met.
    fn insert_document(conn: &Connection, doc_id: Uuid) {
        conn.execute(
            "INSERT INTO documents
             (id, name, doc_type, schema_version, normalization_version,
              hash_contract_version, ingested_at, metadata)
             VALUES (?1, 'test-doc', 'CONTRACT', '1.0.0', '1.0.0', '1.0.0',
                     '2024-01-01T00:00:00Z', '{}')",
            rusqlite::params![doc_id.to_string()],
        )
        .expect("insert document");
    }

    fn setup() -> (Connection, Uuid) {
        let conn = Connection::open_in_memory().expect("in-memory db");
        run_migrations(&conn).expect("migrations");
        let doc_id = Uuid::new_v4();
        insert_document(&conn, doc_id);
        (conn, doc_id)
    }

    #[test]
    fn create_workflow_persists_and_returns_draft() {
        let (conn, doc_id) = setup();
        let wf = WorkflowEngine::create_workflow(&conn, doc_id, "alice")
            .expect("create_workflow should succeed");
        assert_eq!(wf.state, WorkflowState::Draft);
        assert_eq!(wf.initiator_id, "alice");
        assert_eq!(wf.document_id, doc_id);

        // Event should exist.
        let events = WorkflowEngine::get_events(&conn, wf.id).expect("get_events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, EventType::WorkflowCreated);
        assert_eq!(events[0].seq, 1);
    }

    #[test]
    fn get_unknown_workflow_returns_not_found() {
        let (conn, _) = setup();
        let result = WorkflowEngine::get_workflow(&conn, Uuid::new_v4());
        assert!(
            matches!(result, Err(rt_core::RtError::NotFound(_))),
            "expected NotFound, got {:?}",
            result
        );
    }

    #[test]
    fn full_lifecycle_eight_events_to_completed() {
        let (conn, doc_id) = setup();
        let wf = WorkflowEngine::create_workflow(&conn, doc_id, "alice").unwrap();
        let wid = wf.id;

        let steps: Vec<(EventType, &str)> = vec![
            (EventType::CompareStarted, "system"),
            (EventType::CompareCompleted, "system"),
            (EventType::ReviewStarted, "alice"),
            (EventType::ReviewerAssigned, "alice"),
            (EventType::DeltaSubmitted, "bob"),
            (EventType::ReviewClosed, "alice"),
            (EventType::EditCompilationStarted, "system"),
            (EventType::EditCompilationCompleted, "system"),
        ];

        let mut last_wf = wf;
        for (et, actor) in steps {
            last_wf = WorkflowEngine::submit_event(
                &conn,
                wid,
                et,
                actor,
                serde_json::Value::Null,
            )
            .expect("submit_event should succeed");
        }

        assert_eq!(
            last_wf.state,
            WorkflowState::ReadyForFinalization,
            "should be ReadyForFinalization after 8 submit_event calls"
        );

        // Final event to reach Completed.
        let final_wf = WorkflowEngine::submit_event(
            &conn,
            wid,
            EventType::WorkflowCompleted,
            "alice",
            serde_json::Value::Null,
        )
        .expect("WorkflowCompleted should succeed");
        assert_eq!(final_wf.state, WorkflowState::Completed);

        // Total events: 1 (WorkflowCreated) + 8 + 1 = 10
        let events = WorkflowEngine::get_events(&conn, wid).unwrap();
        assert_eq!(events.len(), 10);
    }

    #[test]
    fn abort_from_draft() {
        let (conn, doc_id) = setup();
        let wf = WorkflowEngine::create_workflow(&conn, doc_id, "alice").unwrap();
        let result = WorkflowEngine::submit_event(
            &conn,
            wf.id,
            EventType::WorkflowAborted,
            "alice",
            serde_json::Value::Null,
        )
        .expect("abort from Draft should succeed");
        assert_eq!(result.state, WorkflowState::Aborted);
    }

    #[test]
    fn abort_from_in_review() {
        let (conn, doc_id) = setup();
        let wf = WorkflowEngine::create_workflow(&conn, doc_id, "alice").unwrap();
        let wid = wf.id;

        for et in [
            EventType::CompareStarted,
            EventType::CompareCompleted,
            EventType::ReviewStarted,
        ] {
            WorkflowEngine::submit_event(&conn, wid, et, "system", serde_json::Value::Null)
                .unwrap();
        }

        let result = WorkflowEngine::submit_event(
            &conn,
            wid,
            EventType::WorkflowAborted,
            "alice",
            serde_json::Value::Null,
        )
        .expect("abort from InReview should succeed");
        assert_eq!(result.state, WorkflowState::Aborted);
    }

    #[test]
    fn abort_from_review_closed() {
        let (conn, doc_id) = setup();
        let wf = WorkflowEngine::create_workflow(&conn, doc_id, "alice").unwrap();
        let wid = wf.id;

        for et in [
            EventType::CompareStarted,
            EventType::CompareCompleted,
            EventType::ReviewStarted,
            EventType::ReviewClosed,
        ] {
            WorkflowEngine::submit_event(&conn, wid, et, "system", serde_json::Value::Null)
                .unwrap();
        }

        let result = WorkflowEngine::submit_event(
            &conn,
            wid,
            EventType::WorkflowAborted,
            "alice",
            serde_json::Value::Null,
        )
        .expect("abort from ReviewClosed should succeed");
        assert_eq!(result.state, WorkflowState::Aborted);
    }

    #[test]
    fn abort_from_completed_fails() {
        let (conn, doc_id) = setup();
        let wf = WorkflowEngine::create_workflow(&conn, doc_id, "alice").unwrap();
        let wid = wf.id;

        for et in [
            EventType::CompareStarted,
            EventType::CompareCompleted,
            EventType::ReviewStarted,
            EventType::ReviewClosed,
            EventType::EditCompilationStarted,
            EventType::EditCompilationCompleted,
            EventType::WorkflowCompleted,
        ] {
            WorkflowEngine::submit_event(&conn, wid, et, "system", serde_json::Value::Null)
                .unwrap();
        }

        let result = WorkflowEngine::submit_event(
            &conn,
            wid,
            EventType::WorkflowAborted,
            "alice",
            serde_json::Value::Null,
        );
        assert!(
            result.is_err(),
            "aborting a Completed workflow should fail"
        );
    }
}
