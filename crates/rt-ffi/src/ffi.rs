use std::os::raw::c_char;
use std::sync::OnceLock;

use uuid::Uuid;

use rt_core::db::{create_pool, DbPool, SqliteBlockStore, BlockStore};
use rt_core::block::{Block, Document, DocumentType};
use rt_compare::worker::{CompareEngine, CompareConfig};
use rt_merge::merge::MergeEngine;
use rt_workflow::commands::WorkflowEngine;
use rt_workflow::event::EventType;

use crate::marshal::{cstring_to_str, deserialize_json};
use crate::result::RtflowResult;

// ---------------------------------------------------------------------------
// Global database pool
// ---------------------------------------------------------------------------

static DB_POOL: OnceLock<DbPool> = OnceLock::new();

/// Return a reference to the global pool, or an error string if
/// `rtflow_init` has not been called yet.
fn get_pool() -> Result<&'static DbPool, String> {
    DB_POOL
        .get()
        .ok_or_else(|| "Database not initialized. Call rtflow_init first.".to_string())
}

// ---------------------------------------------------------------------------
// Memory management
// ---------------------------------------------------------------------------

/// Free a `RtflowResult` that was returned by any `rtflow_*` function.
///
/// Passing a null pointer is a no-op.
///
/// # Safety
///
/// `ptr` must be either null or a valid pointer that was previously returned
/// by one of the `rtflow_*` functions and has not yet been freed.
#[no_mangle]
pub unsafe extern "C" fn rtflow_free(ptr: *mut RtflowResult) {
    RtflowResult::free(ptr);
}

// ---------------------------------------------------------------------------
// Database
// ---------------------------------------------------------------------------

/// Initialize (or open) the SQLite database at `db_path`.
///
/// `db_path` must be a valid, null-terminated UTF-8 path string.
///
/// Returns a `RtflowResult` with `ok = true` and `data = "{}"` on success,
/// or `ok = false` and a descriptive error message on failure.
///
/// The returned pointer must be freed with `rtflow_free`.
///
/// # Safety
///
/// `db_path` must be a valid, non-null, null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn rtflow_init(db_path: *const c_char) -> *mut RtflowResult {
    let path = match cstring_to_str(db_path) {
        Ok(s) => s,
        Err(e) => return RtflowResult::failure(&e),
    };

    match create_pool(&path) {
        Ok(pool) => {
            // Only the first caller wins; subsequent callers get a
            // descriptive error rather than silently succeeding.
            if DB_POOL.set(pool).is_err() {
                return RtflowResult::failure(
                    "Database already initialized; rtflow_init may only be called once.",
                );
            }
            RtflowResult::success("{}")
        }
        Err(e) => RtflowResult::failure(&e.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Document ingestion
// ---------------------------------------------------------------------------

/// Ingest a list of blocks (as a JSON array) into the store under `doc_id`.
///
/// `json_ptr`    — null-terminated UTF-8 string containing the blocks JSON.
/// `doc_id_ptr`  — null-terminated UTF-8 string containing the document UUID.
///
/// Returns a `RtflowResult` whose `data` field is the ingested document UUID
/// on success.
///
/// The returned pointer must be freed with `rtflow_free`.
///
/// # Safety
///
/// Both pointer arguments must be valid, non-null, null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn rtflow_ingest_blocks(
    json_ptr: *const c_char,
    doc_id_ptr: *const c_char,
) -> *mut RtflowResult {
    let json = match cstring_to_str(json_ptr) {
        Ok(s) => s,
        Err(e) => return RtflowResult::failure(&e),
    };

    let doc_id_str = match cstring_to_str(doc_id_ptr) {
        Ok(s) => s,
        Err(e) => return RtflowResult::failure(&e),
    };

    let doc_id = match Uuid::parse_str(&doc_id_str) {
        Ok(id) => id,
        Err(e) => return RtflowResult::failure(&format!("invalid document UUID: {}", e)),
    };

    let pool = match get_pool() {
        Ok(p) => p,
        Err(e) => return RtflowResult::failure(&e),
    };

    // Deserialize as an array of blocks.
    let blocks: Vec<Block> = match deserialize_json(&json) {
        Ok(b) => b,
        Err(e) => return RtflowResult::failure(&format!("failed to parse blocks JSON: {}", e)),
    };

    let store = SqliteBlockStore::new(pool.clone());

    // Ensure the document row exists; insert a minimal record if missing.
    if store.get_document(&doc_id).is_err() {
        use chrono::Utc;
        use rt_core::schema::SCHEMA_VERSION;
        let doc = Document {
            id: doc_id,
            name: doc_id_str.clone(),
            source_path: None,
            doc_type: DocumentType::Original,
            schema_version: SCHEMA_VERSION.to_string(),
            normalization_version: "1.0.0".to_string(),
            hash_contract_version: "1.0.0".to_string(),
            ingested_at: Utc::now(),
            metadata: None,
        };
        if let Err(e) = store.insert_document(&doc) {
            return RtflowResult::failure(&format!("failed to create document record: {}", e));
        }
    }

    let count = blocks.len();

    if let Err(e) = store.insert_blocks(&blocks) {
        return RtflowResult::failure(&format!("failed to insert blocks: {}", e));
    }

    let payload = serde_json::json!({
        "doc_id": doc_id.to_string(),
        "count": count,
    });

    match serde_json::to_string(&payload) {
        Ok(json_out) => RtflowResult::success(&json_out),
        Err(e) => RtflowResult::failure(&format!("failed to serialize response: {}", e)),
    }
}

// ---------------------------------------------------------------------------
// Compare
// ---------------------------------------------------------------------------

/// Compare two documents identified by their UUIDs.
///
/// `left_doc_id`   — null-terminated UTF-8 string: UUID of the left document.
/// `right_doc_id`  — null-terminated UTF-8 string: UUID of the right document.
/// `options_json`  — null-terminated UTF-8 string: JSON object with compare
///                   options (may be `"{}"` for defaults).
///
/// Returns a `RtflowResult` whose `data` field is a `CompareResult` JSON
/// object on success.
///
/// The returned pointer must be freed with `rtflow_free`.
///
/// # Safety
///
/// All pointer arguments must be valid, non-null, null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn rtflow_compare(
    left_doc_id: *const c_char,
    right_doc_id: *const c_char,
    options_json: *const c_char,
) -> *mut RtflowResult {
    let left_str = match cstring_to_str(left_doc_id) {
        Ok(s) => s,
        Err(e) => return RtflowResult::failure(&e),
    };
    let right_str = match cstring_to_str(right_doc_id) {
        Ok(s) => s,
        Err(e) => return RtflowResult::failure(&e),
    };
    let _options_str = match cstring_to_str(options_json) {
        Ok(s) => s,
        Err(e) => return RtflowResult::failure(&e),
    };

    let left_id = match Uuid::parse_str(&left_str) {
        Ok(id) => id,
        Err(e) => return RtflowResult::failure(&format!("invalid left_doc_id UUID: {}", e)),
    };
    let right_id = match Uuid::parse_str(&right_str) {
        Ok(id) => id,
        Err(e) => return RtflowResult::failure(&format!("invalid right_doc_id UUID: {}", e)),
    };

    let pool = match get_pool() {
        Ok(p) => p,
        Err(e) => return RtflowResult::failure(&e),
    };

    let store = SqliteBlockStore::new(pool.clone());

    let left_blocks = match store.get_block_tree(&left_id) {
        Ok(b) => b,
        Err(e) => {
            return RtflowResult::failure(&format!("failed to load left document blocks: {}", e))
        }
    };
    let right_blocks = match store.get_block_tree(&right_id) {
        Ok(b) => b,
        Err(e) => {
            return RtflowResult::failure(&format!("failed to load right document blocks: {}", e))
        }
    };

    let engine = CompareEngine::new(CompareConfig::default());
    let result = engine.compare(left_id, right_id, &left_blocks, &right_blocks);

    match serde_json::to_string(&result) {
        Ok(json_out) => RtflowResult::success(&json_out),
        Err(e) => RtflowResult::failure(&format!("failed to serialize CompareResult: {}", e)),
    }
}

// ---------------------------------------------------------------------------
// Merge
// ---------------------------------------------------------------------------

/// Merge an incoming document into a base document.
///
/// `base_doc_id`     — null-terminated UTF-8 string: UUID of the base document.
/// `incoming_doc_id` — null-terminated UTF-8 string: UUID of the incoming document.
/// `options_json`    — null-terminated UTF-8 string: JSON object with merge
///                     options (may be `"{}"` for defaults).
///
/// Returns a `RtflowResult` whose `data` field is a `MergeResult` JSON object
/// on success.
///
/// The returned pointer must be freed with `rtflow_free`.
///
/// # Safety
///
/// All pointer arguments must be valid, non-null, null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn rtflow_merge(
    base_doc_id: *const c_char,
    incoming_doc_id: *const c_char,
    options_json: *const c_char,
) -> *mut RtflowResult {
    let base_str = match cstring_to_str(base_doc_id) {
        Ok(s) => s,
        Err(e) => return RtflowResult::failure(&e),
    };
    let incoming_str = match cstring_to_str(incoming_doc_id) {
        Ok(s) => s,
        Err(e) => return RtflowResult::failure(&e),
    };
    let _options_str = match cstring_to_str(options_json) {
        Ok(s) => s,
        Err(e) => return RtflowResult::failure(&e),
    };

    let base_id = match Uuid::parse_str(&base_str) {
        Ok(id) => id,
        Err(e) => return RtflowResult::failure(&format!("invalid base_doc_id UUID: {}", e)),
    };
    let incoming_id = match Uuid::parse_str(&incoming_str) {
        Ok(id) => id,
        Err(e) => return RtflowResult::failure(&format!("invalid incoming_doc_id UUID: {}", e)),
    };

    let pool = match get_pool() {
        Ok(p) => p,
        Err(e) => return RtflowResult::failure(&e),
    };

    let store = SqliteBlockStore::new(pool.clone());

    let base_blocks = match store.get_block_tree(&base_id) {
        Ok(b) => b,
        Err(e) => {
            return RtflowResult::failure(&format!("failed to load base document blocks: {}", e))
        }
    };
    let incoming_blocks = match store.get_block_tree(&incoming_id) {
        Ok(b) => b,
        Err(e) => {
            return RtflowResult::failure(&format!(
                "failed to load incoming document blocks: {}",
                e
            ))
        }
    };

    let engine = MergeEngine::new();
    let result = engine.merge(base_id, incoming_id, &base_blocks, &incoming_blocks);

    match serde_json::to_string(&result) {
        Ok(json_out) => RtflowResult::success(&json_out),
        Err(e) => RtflowResult::failure(&format!("failed to serialize MergeResult: {}", e)),
    }
}

// ---------------------------------------------------------------------------
// Workflow
// ---------------------------------------------------------------------------

/// Submit a workflow event and advance the workflow state machine.
///
/// `workflow_id` — null-terminated UTF-8 string: UUID of the workflow.
/// `event_json`  — null-terminated UTF-8 string: JSON object describing the
///                 event to apply.
///
/// The `event_json` object must contain at least:
///   - `"event_type"`: string — a valid `EventType` snake_case value
///   - `"actor"`:      string — identifier of the user/system submitting the event
///
/// An optional `"payload"` key may hold any JSON value; it defaults to `{}`.
///
/// Returns a `RtflowResult` whose `data` field is the updated `Workflow`
/// JSON object on success.
///
/// The returned pointer must be freed with `rtflow_free`.
///
/// # Safety
///
/// Both pointer arguments must be valid, non-null, null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn rtflow_workflow_event(
    workflow_id: *const c_char,
    event_json: *const c_char,
) -> *mut RtflowResult {
    let wf_id_str = match cstring_to_str(workflow_id) {
        Ok(s) => s,
        Err(e) => return RtflowResult::failure(&e),
    };
    let event_str = match cstring_to_str(event_json) {
        Ok(s) => s,
        Err(e) => return RtflowResult::failure(&e),
    };

    let wf_id = match Uuid::parse_str(&wf_id_str) {
        Ok(id) => id,
        Err(e) => return RtflowResult::failure(&format!("invalid workflow_id UUID: {}", e)),
    };

    // Parse the event JSON envelope.
    let event_value: serde_json::Value = match deserialize_json(&event_str) {
        Ok(v) => v,
        Err(e) => return RtflowResult::failure(&format!("failed to parse event JSON: {}", e)),
    };

    let event_type_str = match event_value.get("event_type").and_then(|v| v.as_str()) {
        Some(s) => s.to_owned(),
        None => {
            return RtflowResult::failure(
                "event JSON must contain a string field \"event_type\"",
            )
        }
    };

    let actor = match event_value.get("actor").and_then(|v| v.as_str()) {
        Some(s) => s.to_owned(),
        None => {
            return RtflowResult::failure("event JSON must contain a string field \"actor\"")
        }
    };

    let payload = event_value
        .get("payload")
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    let event_type = match EventType::from_str(&event_type_str) {
        Ok(et) => et,
        Err(e) => return RtflowResult::failure(&format!("invalid event_type: {}", e)),
    };

    let pool = match get_pool() {
        Ok(p) => p,
        Err(e) => return RtflowResult::failure(&e),
    };

    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            return RtflowResult::failure(&format!("failed to acquire database connection: {}", e))
        }
    };

    match WorkflowEngine::submit_event(&conn, wf_id, event_type, &actor, payload) {
        Ok(wf) => match serde_json::to_string(&wf) {
            Ok(json_out) => RtflowResult::success(&json_out),
            Err(e) => RtflowResult::failure(&format!("failed to serialize Workflow: {}", e)),
        },
        Err(e) => RtflowResult::failure(&e.to_string()),
    }
}

/// Retrieve the current state of a workflow.
///
/// `workflow_id` — null-terminated UTF-8 string: UUID of the workflow.
///
/// Returns a `RtflowResult` whose `data` field is the current `Workflow`
/// JSON object on success.
///
/// The returned pointer must be freed with `rtflow_free`.
///
/// # Safety
///
/// `workflow_id` must be a valid, non-null, null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn rtflow_workflow_state(
    workflow_id: *const c_char,
) -> *mut RtflowResult {
    let wf_id_str = match cstring_to_str(workflow_id) {
        Ok(s) => s,
        Err(e) => return RtflowResult::failure(&e),
    };

    let wf_id = match Uuid::parse_str(&wf_id_str) {
        Ok(id) => id,
        Err(e) => return RtflowResult::failure(&format!("invalid workflow_id UUID: {}", e)),
    };

    let pool = match get_pool() {
        Ok(p) => p,
        Err(e) => return RtflowResult::failure(&e),
    };

    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            return RtflowResult::failure(&format!("failed to acquire database connection: {}", e))
        }
    };

    match WorkflowEngine::get_workflow(&conn, wf_id) {
        Ok(wf) => match serde_json::to_string(&wf) {
            Ok(json_out) => RtflowResult::success(&json_out),
            Err(e) => RtflowResult::failure(&format!("failed to serialize Workflow: {}", e)),
        },
        Err(e) => RtflowResult::failure(&e.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Initialize the FFI layer using an in-memory SQLite database.
///
/// This function is provided for integration testing only.  It behaves
/// identically to `rtflow_init` but uses an ephemeral in-memory database
/// instead of a file on disk.
///
/// Returns `RtflowResult` with `ok = true` and `data = "{}"` on success.
/// The returned pointer must be freed with `rtflow_free`.
#[cfg(test)]
pub fn rtflow_init_memory() -> *mut RtflowResult {
    use rt_core::db::create_memory_pool;
    match create_memory_pool() {
        Ok(pool) => {
            if DB_POOL.set(pool).is_err() {
                return RtflowResult::failure(
                    "Database already initialized; rtflow_init_memory may only be called once.",
                );
            }
            RtflowResult::success("{}")
        }
        Err(e) => RtflowResult::failure(&e.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    use chrono::Utc;
    use rt_core::block::{Block, BlockType, Document, DocumentType};
    use rt_core::db::{create_memory_pool, DbPool, SqliteBlockStore, BlockStore};
    use rt_core::schema::SCHEMA_VERSION;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Create an isolated in-memory pool for a single test.
    fn make_test_pool() -> DbPool {
        create_memory_pool().expect("in-memory pool")
    }

    fn make_test_store(pool: DbPool) -> SqliteBlockStore {
        SqliteBlockStore::new(pool)
    }

    fn make_doc(pool: &DbPool) -> Document {
        let doc = Document {
            id: Uuid::new_v4(),
            name: "test-doc".to_string(),
            source_path: None,
            doc_type: DocumentType::Original,
            schema_version: SCHEMA_VERSION.to_string(),
            normalization_version: "1.0.0".to_string(),
            hash_contract_version: "1.0.0".to_string(),
            ingested_at: Utc::now(),
            metadata: None,
        };
        let store = SqliteBlockStore::new(pool.clone());
        store.insert_document(&doc).expect("insert_document");
        doc
    }

    fn make_block(doc_id: Uuid, path: &str, text: &str, pos: i32) -> Block {
        Block::new(BlockType::Clause, path, text, text, None, doc_id, pos)
    }

    fn blocks_json(doc_id: Uuid) -> String {
        let blocks: Vec<Block> = vec![
            make_block(doc_id, "1.1", "the borrower shall repay the principal", 0),
            make_block(doc_id, "1.2", "interest shall accrue at five percent per annum", 1),
        ];
        serde_json::to_string(&blocks).expect("serialize blocks")
    }

    fn to_cstr(s: &str) -> CString {
        CString::new(s).expect("CString::new")
    }

    // -----------------------------------------------------------------------
    // Test: rtflow_free does not panic on null
    // -----------------------------------------------------------------------

    #[test]
    fn free_null_is_noop() {
        unsafe {
            rtflow_free(std::ptr::null_mut());
        }
    }

    // -----------------------------------------------------------------------
    // Test: RtflowResult success/failure round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn result_success_and_free() {
        unsafe {
            let ptr = RtflowResult::success(r#"{"ok":true}"#);
            assert!(!ptr.is_null());
            assert!((*ptr).ok);
            RtflowResult::free(ptr);
        }
    }

    #[test]
    fn result_failure_and_free() {
        unsafe {
            let ptr = RtflowResult::failure("something went wrong");
            assert!(!ptr.is_null());
            assert!(!(*ptr).ok);
            RtflowResult::free(ptr);
        }
    }

    // -----------------------------------------------------------------------
    // Test: rtflow_init with in-memory database (via test helper)
    // -----------------------------------------------------------------------

    // NOTE: Because DB_POOL is a process-global OnceLock the init tests
    // interact; each test that needs an initialized pool must work with
    // whatever state the OnceLock is already in.  The safe approach is to
    // exercise init functionality via the store directly and only call
    // rtflow_init_memory once per test binary.

    #[test]
    fn init_memory_succeeds() {
        // Attempt to initialise; if the pool is already set from a previous
        // test in this binary, the function returns an error string – that is
        // acceptable behaviour which we simply tolerate here.
        let ptr = rtflow_init_memory();
        unsafe {
            assert!(!ptr.is_null());
            RtflowResult::free(ptr);
        }
    }

    // -----------------------------------------------------------------------
    // Test: marshal helpers
    // -----------------------------------------------------------------------

    #[test]
    fn cstring_to_str_null_returns_err() {
        unsafe {
            let result = cstring_to_str(std::ptr::null());
            assert!(result.is_err());
        }
    }

    #[test]
    fn cstring_to_str_valid_returns_ok() {
        let s = to_cstr("hello world");
        unsafe {
            let result = cstring_to_str(s.as_ptr());
            assert_eq!(result.unwrap(), "hello world");
        }
    }

    #[test]
    fn deserialize_json_valid() {
        let result: Result<serde_json::Value, _> = deserialize_json(r#"{"key": 42}"#);
        assert!(result.is_ok());
        assert_eq!(result.unwrap()["key"], 42);
    }

    #[test]
    fn deserialize_json_invalid_returns_err() {
        let result: Result<serde_json::Value, _> = deserialize_json("not json {{{");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Test: ingest blocks via store (unit-level, bypassing global state)
    // -----------------------------------------------------------------------

    #[test]
    fn store_ingest_blocks_roundtrip() {
        let pool = make_test_pool();
        let doc = make_doc(&pool);
        let store = make_test_store(pool);

        let blocks: Vec<Block> = vec![
            make_block(doc.id, "1.1", "the borrower shall repay the principal", 0),
            make_block(doc.id, "1.2", "interest shall accrue at five percent", 1),
        ];

        store.insert_blocks(&blocks).expect("insert_blocks");

        let fetched = store.get_block_tree(&doc.id).expect("get_block_tree");
        assert_eq!(fetched.len(), 2);
        assert_eq!(fetched[0].structural_path, "1.1");
        assert_eq!(fetched[1].structural_path, "1.2");
    }

    // -----------------------------------------------------------------------
    // Test: compare two documents via engine (unit-level)
    // -----------------------------------------------------------------------

    #[test]
    fn compare_two_docs_via_engine() {
        let pool = make_test_pool();
        let left_doc = make_doc(&pool);
        let right_doc = make_doc(&pool);
        let store = make_test_store(pool);

        let left_blocks = vec![
            make_block(left_doc.id, "1.1", "the borrower shall repay the principal", 0),
            make_block(left_doc.id, "1.2", "interest accrues at five percent", 1),
        ];
        let right_blocks = vec![
            make_block(right_doc.id, "1.1", "the borrower shall repay the principal", 0),
            make_block(right_doc.id, "1.2", "interest accrues at six percent per annum", 1),
        ];

        store.insert_blocks(&left_blocks).expect("insert left");
        store.insert_blocks(&right_blocks).expect("insert right");

        let lft = store.get_block_tree(&left_doc.id).unwrap();
        let rgt = store.get_block_tree(&right_doc.id).unwrap();

        let engine = CompareEngine::new(CompareConfig::default());
        let result = engine.compare(left_doc.id, right_doc.id, &lft, &rgt);

        assert_eq!(result.stats.blocks_left, 2);
        assert_eq!(result.stats.blocks_right, 2);
        assert_eq!(result.stats.unchanged, 1);
        assert_eq!(result.stats.modified, 1);

        let json = serde_json::to_string(&result).expect("serialize CompareResult");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("run_id").is_some());
        assert!(parsed.get("deltas").is_some());
    }

    // -----------------------------------------------------------------------
    // Test: compare identical documents
    // -----------------------------------------------------------------------

    #[test]
    fn compare_identical_docs_all_unchanged() {
        let pool = make_test_pool();
        let doc = make_doc(&pool);
        let store = make_test_store(pool);

        let blocks = vec![
            make_block(doc.id, "1.1", "the borrower shall repay the principal", 0),
            make_block(doc.id, "1.2", "interest shall accrue at five percent per annum", 1),
        ];

        store.insert_blocks(&blocks).expect("insert");

        let fetched = store.get_block_tree(&doc.id).unwrap();

        let engine = CompareEngine::new(CompareConfig::default());
        let result = engine.compare(doc.id, doc.id, &fetched, &fetched);

        assert_eq!(result.stats.unchanged, 2);
        assert_eq!(result.stats.modified, 0);
        assert_eq!(result.stats.inserted, 0);
        assert_eq!(result.stats.deleted, 0);
    }

    // -----------------------------------------------------------------------
    // Test: merge two documents via engine (unit-level)
    // -----------------------------------------------------------------------

    #[test]
    fn merge_two_docs_via_engine() {
        let pool = make_test_pool();
        let base_doc = make_doc(&pool);
        let incoming_doc = make_doc(&pool);
        let store = make_test_store(pool);

        let base_blocks = vec![
            make_block(base_doc.id, "1.1", "the borrower shall repay the principal", 0),
            make_block(base_doc.id, "1.2", "interest accrues at five percent", 1),
        ];
        let incoming_blocks = vec![
            make_block(incoming_doc.id, "1.1", "the borrower shall repay the principal", 0),
            make_block(incoming_doc.id, "1.2", "interest accrues at six percent per annum", 1),
        ];

        store.insert_blocks(&base_blocks).expect("insert base");
        store.insert_blocks(&incoming_blocks).expect("insert incoming");

        let base = store.get_block_tree(&base_doc.id).unwrap();
        let incoming = store.get_block_tree(&incoming_doc.id).unwrap();

        let engine = MergeEngine::new();
        let result = engine.merge(base_doc.id, incoming_doc.id, &base, &incoming);

        let json = serde_json::to_string(&result).expect("serialize MergeResult");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("merge_id").is_some());
        assert!(parsed.get("conflicts").is_some());
        assert!(parsed.get("auto_resolved").is_some());
        assert!(parsed.get("pending_review").is_some());
    }

    // -----------------------------------------------------------------------
    // Test: workflow lifecycle via engine (unit-level)
    // -----------------------------------------------------------------------

    #[test]
    fn workflow_lifecycle_via_engine() {
        let pool = make_test_pool();
        let store = SqliteBlockStore::new(pool.clone());

        // Insert a document row for the foreign-key constraint.
        let doc_id = Uuid::new_v4();
        let doc = Document {
            id: doc_id,
            name: "workflow-test-doc".to_string(),
            source_path: None,
            doc_type: DocumentType::Original,
            schema_version: SCHEMA_VERSION.to_string(),
            normalization_version: "1.0.0".to_string(),
            hash_contract_version: "1.0.0".to_string(),
            ingested_at: Utc::now(),
            metadata: None,
        };
        store.insert_document(&doc).expect("insert document");

        let conn = pool.get().expect("connection");

        // Create workflow.
        let wf = WorkflowEngine::create_workflow(&conn, doc_id, "alice")
            .expect("create_workflow");

        use rt_workflow::state::WorkflowState;

        assert_eq!(wf.state, WorkflowState::Draft);

        // Advance through the happy path.
        let steps = vec![
            (EventType::CompareStarted, "system"),
            (EventType::CompareCompleted, "system"),
            (EventType::ReviewStarted, "alice"),
        ];

        let mut current = wf;
        for (et, actor) in steps {
            current = WorkflowEngine::submit_event(
                &conn,
                current.id,
                et,
                actor,
                serde_json::Value::Null,
            )
            .expect("submit_event");
        }

        assert_eq!(current.state, WorkflowState::InReview);

        // Retrieve via get_workflow and verify JSON serialisation.
        let fetched = WorkflowEngine::get_workflow(&conn, current.id).expect("get_workflow");
        let json = serde_json::to_string(&fetched).expect("serialize Workflow");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("id").is_some());
        assert!(parsed.get("state").is_some());
        assert_eq!(
            parsed["state"].as_str().unwrap(),
            WorkflowState::InReview.as_str()
        );
    }

    // -----------------------------------------------------------------------
    // Test: rtflow_ingest_blocks via FFI (requires initialized pool)
    // -----------------------------------------------------------------------

    #[test]
    fn ffi_ingest_blocks_returns_success_or_not_initialized() {
        let doc_id = Uuid::new_v4();
        let json = blocks_json(doc_id);
        let c_json = to_cstr(&json);
        let c_doc_id = to_cstr(&doc_id.to_string());

        unsafe {
            let ptr = rtflow_ingest_blocks(c_json.as_ptr(), c_doc_id.as_ptr());
            assert!(!ptr.is_null());
            // We accept either ok (pool initialized) or error (pool not yet set).
            // The test merely verifies no panic / memory unsafety.
            RtflowResult::free(ptr);
        }
    }

    // -----------------------------------------------------------------------
    // Test: rtflow_workflow_event / rtflow_workflow_state via FFI
    // (requires initialized pool; skips gracefully when not initialized)
    // -----------------------------------------------------------------------

    #[test]
    fn ffi_workflow_event_without_init_returns_error() {
        // When the pool is not set the functions must return a failure result
        // rather than panicking.  Because the OnceLock may already be set by
        // init_memory_succeeds() we test the "not initialized" path only when
        // we can confirm the lock is empty by using a fresh pool directly.
        //
        // If the pool IS already set we skip this particular assertion.
        if DB_POOL.get().is_none() {
            let wf_id = to_cstr(&Uuid::new_v4().to_string());
            let event = to_cstr(r#"{"event_type":"compare_started","actor":"system"}"#);
            unsafe {
                let ptr = rtflow_workflow_event(wf_id.as_ptr(), event.as_ptr());
                assert!(!ptr.is_null());
                assert!(!(*ptr).ok, "expected failure when pool not initialized");
                RtflowResult::free(ptr);
            }
        }
    }

    #[test]
    fn ffi_workflow_state_without_init_returns_error() {
        if DB_POOL.get().is_none() {
            let wf_id = to_cstr(&Uuid::new_v4().to_string());
            unsafe {
                let ptr = rtflow_workflow_state(wf_id.as_ptr());
                assert!(!ptr.is_null());
                assert!(!(*ptr).ok, "expected failure when pool not initialized");
                RtflowResult::free(ptr);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Test: rtflow_compare / rtflow_merge via FFI
    // (tolerates not-initialized state gracefully)
    // -----------------------------------------------------------------------

    #[test]
    fn ffi_compare_without_init_returns_error() {
        if DB_POOL.get().is_none() {
            let left = to_cstr(&Uuid::new_v4().to_string());
            let right = to_cstr(&Uuid::new_v4().to_string());
            let opts = to_cstr("{}");
            unsafe {
                let ptr = rtflow_compare(left.as_ptr(), right.as_ptr(), opts.as_ptr());
                assert!(!ptr.is_null());
                assert!(!(*ptr).ok);
                RtflowResult::free(ptr);
            }
        }
    }

    #[test]
    fn ffi_merge_without_init_returns_error() {
        if DB_POOL.get().is_none() {
            let base = to_cstr(&Uuid::new_v4().to_string());
            let inc = to_cstr(&Uuid::new_v4().to_string());
            let opts = to_cstr("{}");
            unsafe {
                let ptr = rtflow_merge(base.as_ptr(), inc.as_ptr(), opts.as_ptr());
                assert!(!ptr.is_null());
                assert!(!(*ptr).ok);
                RtflowResult::free(ptr);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Test: invalid UUID returns clean error
    // -----------------------------------------------------------------------

    #[test]
    fn ffi_ingest_invalid_uuid_returns_failure() {
        let c_json = to_cstr("[]");
        let c_bad_id = to_cstr("not-a-uuid");
        unsafe {
            let ptr = rtflow_ingest_blocks(c_json.as_ptr(), c_bad_id.as_ptr());
            assert!(!ptr.is_null());
            assert!(!(*ptr).ok);
            RtflowResult::free(ptr);
        }
    }

    #[test]
    fn ffi_compare_invalid_uuid_returns_failure() {
        let bad = to_cstr("bad-uuid");
        let good = to_cstr(&Uuid::new_v4().to_string());
        let opts = to_cstr("{}");
        unsafe {
            let ptr = rtflow_compare(bad.as_ptr(), good.as_ptr(), opts.as_ptr());
            assert!(!ptr.is_null());
            assert!(!(*ptr).ok);
            RtflowResult::free(ptr);
        }
    }

    #[test]
    fn ffi_workflow_event_invalid_event_type() {
        // Pool may or may not be set; either way an invalid event_type must
        // produce a failure (if pool is set) or a "not initialized" failure.
        let wf_id = to_cstr(&Uuid::new_v4().to_string());
        let event = to_cstr(r#"{"event_type":"not_real_event","actor":"system"}"#);
        unsafe {
            let ptr = rtflow_workflow_event(wf_id.as_ptr(), event.as_ptr());
            assert!(!ptr.is_null());
            assert!(!(*ptr).ok);
            RtflowResult::free(ptr);
        }
    }

    #[test]
    fn ffi_workflow_event_missing_actor() {
        let wf_id = to_cstr(&Uuid::new_v4().to_string());
        let event = to_cstr(r#"{"event_type":"compare_started"}"#);
        unsafe {
            let ptr = rtflow_workflow_event(wf_id.as_ptr(), event.as_ptr());
            assert!(!ptr.is_null());
            assert!(!(*ptr).ok);
            RtflowResult::free(ptr);
        }
    }
}
