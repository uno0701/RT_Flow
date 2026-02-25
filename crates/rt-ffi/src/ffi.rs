use std::os::raw::c_char;

use crate::result::RtflowResult;

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
    let _ = db_path;
    RtflowResult::failure("Not implemented")
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
    let _ = (json_ptr, doc_id_ptr);
    RtflowResult::failure("Not implemented")
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
    let _ = (left_doc_id, right_doc_id, options_json);
    RtflowResult::failure("Not implemented")
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
    let _ = (base_doc_id, incoming_doc_id, options_json);
    RtflowResult::failure("Not implemented")
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
/// Returns a `RtflowResult` whose `data` field is the updated `WorkflowState`
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
    let _ = (workflow_id, event_json);
    RtflowResult::failure("Not implemented")
}

/// Retrieve the current state of a workflow.
///
/// `workflow_id` — null-terminated UTF-8 string: UUID of the workflow.
///
/// Returns a `RtflowResult` whose `data` field is the current `WorkflowState`
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
    let _ = workflow_id;
    RtflowResult::failure("Not implemented")
}
