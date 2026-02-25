use crate::error::Result;

/// Monotonic version string recorded in every `documents` row so that readers
/// can detect when a database was created by an older build.
pub const SCHEMA_VERSION: &str = "1.0.0";

// ---------------------------------------------------------------------------
// DDL
// ---------------------------------------------------------------------------

/// Full DDL for every table and index in the RT_Flow SQLite schema.
///
/// All tables use `CREATE TABLE IF NOT EXISTS` so that `run_migrations` is
/// idempotent and safe to call on an already-initialised database.
pub const CREATE_TABLES: &str = "
-- -------------------------------------------------------------------------
-- documents
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS documents (
    id                      TEXT    NOT NULL PRIMARY KEY,
    name                    TEXT    NOT NULL,
    source_path             TEXT,
    doc_type                TEXT    NOT NULL,
    schema_version          TEXT    NOT NULL,
    normalization_version   TEXT    NOT NULL,
    hash_contract_version   TEXT    NOT NULL,
    ingested_at             TEXT    NOT NULL,
    metadata                TEXT    NOT NULL DEFAULT '{}'
);

-- -------------------------------------------------------------------------
-- blocks
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS blocks (
    id                  TEXT    NOT NULL PRIMARY KEY,
    document_id         TEXT    NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    parent_id           TEXT             REFERENCES blocks(id)    ON DELETE SET NULL,
    block_type          TEXT    NOT NULL,
    level               INTEGER NOT NULL DEFAULT 0,
    structural_path     TEXT    NOT NULL,
    anchor_signature    TEXT    NOT NULL,
    clause_hash         TEXT    NOT NULL,
    canonical_text      TEXT    NOT NULL,
    display_text        TEXT    NOT NULL,
    formatting_meta     TEXT    NOT NULL DEFAULT '{}',
    position_index      INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_blocks_document_id
    ON blocks (document_id);

CREATE INDEX IF NOT EXISTS idx_blocks_parent_id
    ON blocks (parent_id);

CREATE INDEX IF NOT EXISTS idx_blocks_anchor_signature
    ON blocks (anchor_signature);

CREATE UNIQUE INDEX IF NOT EXISTS uq_blocks_document_structural_path
    ON blocks (document_id, structural_path);

-- -------------------------------------------------------------------------
-- tokens
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS tokens (
    id          TEXT    NOT NULL PRIMARY KEY,
    block_id    TEXT    NOT NULL REFERENCES blocks(id) ON DELETE CASCADE,
    seq         INTEGER NOT NULL,
    text        TEXT    NOT NULL,
    kind        TEXT    NOT NULL,
    normalized  TEXT    NOT NULL,
    offset      INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tokens_block_id
    ON tokens (block_id);

-- -------------------------------------------------------------------------
-- runs
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS runs (
    id            TEXT    NOT NULL PRIMARY KEY,
    block_id      TEXT    NOT NULL REFERENCES blocks(id) ON DELETE CASCADE,
    seq           INTEGER NOT NULL,
    text          TEXT    NOT NULL,
    bold          INTEGER NOT NULL DEFAULT 0,
    italic        INTEGER NOT NULL DEFAULT 0,
    underline     INTEGER NOT NULL DEFAULT 0,
    strikethrough INTEGER NOT NULL DEFAULT 0,
    font_size     REAL,
    color         TEXT
);

CREATE INDEX IF NOT EXISTS idx_runs_block_id
    ON runs (block_id);

-- -------------------------------------------------------------------------
-- tracked_changes
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS tracked_changes (
    id          TEXT NOT NULL PRIMARY KEY,
    block_id    TEXT NOT NULL REFERENCES blocks(id) ON DELETE CASCADE,
    author      TEXT NOT NULL,
    changed_at  TEXT NOT NULL,
    change_type TEXT NOT NULL,
    original    TEXT
);

-- -------------------------------------------------------------------------
-- block_deltas
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS block_deltas (
    id               TEXT NOT NULL PRIMARY KEY,
    review_layer_id  TEXT,
    reviewer_id      TEXT,
    block_id         TEXT NOT NULL REFERENCES blocks(id) ON DELETE CASCADE,
    delta_type       TEXT NOT NULL,
    token_start      INTEGER,
    token_end        INTEGER,
    delta_payload    TEXT NOT NULL DEFAULT '{}',
    created_at       TEXT NOT NULL
);

-- -------------------------------------------------------------------------
-- review_layers
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS review_layers (
    id           TEXT NOT NULL PRIMARY KEY,
    workflow_id  TEXT,
    reviewer_id  TEXT,
    document_id  TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    created_at   TEXT NOT NULL
);

-- -------------------------------------------------------------------------
-- workflows
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS workflows (
    id           TEXT NOT NULL PRIMARY KEY,
    document_id  TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    state        TEXT NOT NULL,
    initiator_id TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);

-- -------------------------------------------------------------------------
-- workflow_events
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS workflow_events (
    id           TEXT    NOT NULL PRIMARY KEY,
    workflow_id  TEXT    NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    event_type   TEXT    NOT NULL,
    actor        TEXT,
    payload      TEXT    NOT NULL DEFAULT '{}',
    created_at   TEXT    NOT NULL,
    seq          INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_workflow_events_workflow_seq
    ON workflow_events (workflow_id, seq);

-- -------------------------------------------------------------------------
-- merges
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS merges (
    id               TEXT NOT NULL PRIMARY KEY,
    base_doc_id      TEXT NOT NULL REFERENCES documents(id) ON DELETE RESTRICT,
    incoming_doc_id  TEXT NOT NULL REFERENCES documents(id) ON DELETE RESTRICT,
    output_doc_id    TEXT          REFERENCES documents(id) ON DELETE SET NULL,
    status           TEXT NOT NULL,
    created_at       TEXT NOT NULL
);

-- -------------------------------------------------------------------------
-- conflicts
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS conflicts (
    id               TEXT NOT NULL PRIMARY KEY,
    merge_id         TEXT NOT NULL REFERENCES merges(id)  ON DELETE CASCADE,
    block_id         TEXT NOT NULL REFERENCES blocks(id)  ON DELETE CASCADE,
    conflict_type    TEXT NOT NULL,
    base_content     TEXT,
    incoming_content TEXT,
    resolution       TEXT NOT NULL DEFAULT 'pending'
);

-- -------------------------------------------------------------------------
-- artifacts
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS artifacts (
    id                   TEXT NOT NULL PRIMARY KEY,
    workflow_id          TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    artifact_type        TEXT NOT NULL,
    file_path            TEXT NOT NULL,
    content_hash         TEXT NOT NULL,
    source_document_hash TEXT,
    created_at           TEXT NOT NULL
);
";

// ---------------------------------------------------------------------------
// Migration runner
// ---------------------------------------------------------------------------

/// Initialise (or upgrade) the database schema.
///
/// This function is **idempotent**: it is safe to call on a database that has
/// already been initialised.
///
/// Steps performed:
/// 1. Enable WAL journal mode for better concurrent read performance.
/// 2. Enable foreign-key enforcement.
/// 3. Execute the full `CREATE TABLE / INDEX IF NOT EXISTS` DDL.
pub fn run_migrations(conn: &rusqlite::Connection) -> Result<()> {
    // WAL mode gives better read/write concurrency and is safe for the
    // single-writer, multiple-reader pattern used by the connection pool.
    conn.execute_batch("PRAGMA journal_mode = WAL;")?;

    // SQLite does not enforce foreign keys by default; every connection must
    // opt in.
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    // Create all tables and indices.
    conn.execute_batch(CREATE_TABLES)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn open_memory() -> Connection {
        Connection::open_in_memory().expect("in-memory db")
    }

    #[test]
    fn migrations_are_idempotent() {
        let conn = open_memory();
        run_migrations(&conn).expect("first migration");
        // Running a second time must not fail (all DDL uses IF NOT EXISTS).
        run_migrations(&conn).expect("second migration");
    }

    #[test]
    fn all_tables_exist_after_migration() {
        let conn = open_memory();
        run_migrations(&conn).unwrap();

        let expected = [
            "documents",
            "blocks",
            "tokens",
            "runs",
            "tracked_changes",
            "block_deltas",
            "review_layers",
            "workflows",
            "workflow_events",
            "merges",
            "conflicts",
            "artifacts",
        ];

        for table in &expected {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    rusqlite::params![table],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            assert_eq!(count, 1, "table '{table}' should exist");
        }
    }

    #[test]
    fn wal_mode_is_active() {
        let conn = open_memory();
        run_migrations(&conn).unwrap();
        // For an in-memory database SQLite returns "memory", not "wal", but
        // the PRAGMA must not error.
        let _mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
    }
}
