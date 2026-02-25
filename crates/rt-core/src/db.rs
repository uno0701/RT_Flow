use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use uuid::Uuid;

use crate::block::{
    Block, BlockType, Document, DocumentType, FormattingMeta, Run, RunFormatting,
    Token, TokenKind, TrackedChange,
};
use crate::error::{Result, RtError};
use crate::schema::run_migrations;

// ---------------------------------------------------------------------------
// Pool type alias
// ---------------------------------------------------------------------------

pub type DbPool = Pool<SqliteConnectionManager>;

// ---------------------------------------------------------------------------
// Pool constructors
// ---------------------------------------------------------------------------

/// Open a connection pool backed by a file-based SQLite database.
pub fn create_pool(db_path: &str) -> Result<DbPool> {
    let manager = SqliteConnectionManager::file(db_path)
        .with_init(|conn| {
            conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
            Ok(())
        });

    let pool = Pool::builder()
        .max_size(16)
        .build(manager)
        .map_err(|e| RtError::Internal(e.to_string()))?;

    let conn = pool.get().map_err(|e| RtError::Internal(e.to_string()))?;
    run_migrations(&conn)?;

    Ok(pool)
}

/// Open a connection pool backed by a shared in-memory SQLite database.
pub fn create_memory_pool() -> Result<DbPool> {
    let manager = SqliteConnectionManager::memory()
        .with_init(|conn| {
            conn.execute_batch("PRAGMA foreign_keys = ON;")?;
            Ok(())
        });

    let pool = Pool::builder()
        .max_size(4)
        .build(manager)
        .map_err(|e| RtError::Internal(e.to_string()))?;

    let conn = pool.get().map_err(|e| RtError::Internal(e.to_string()))?;
    run_migrations(&conn)?;

    Ok(pool)
}

// ---------------------------------------------------------------------------
// BlockStore trait
// ---------------------------------------------------------------------------

/// Persistence interface for blocks and their parent documents.
pub trait BlockStore: Send + Sync {
    fn insert_document(&self, doc: &Document) -> Result<()>;
    fn get_document(&self, id: &Uuid) -> Result<Document>;
    fn insert_block(&self, block: &Block) -> Result<()>;
    fn insert_blocks(&self, blocks: &[Block]) -> Result<()>;
    fn get_blocks_by_document(&self, doc_id: &Uuid) -> Result<Vec<Block>>;
    fn get_block(&self, id: &Uuid) -> Result<Block>;
    fn get_block_children(&self, parent_id: &Uuid) -> Result<Vec<Block>>;
    fn get_block_tree(&self, doc_id: &Uuid) -> Result<Vec<Block>>;
    fn update_block(&self, block: &Block) -> Result<()>;
    fn delete_block(&self, id: &Uuid) -> Result<()>;
    fn get_blocks_by_anchor(&self, anchor_signature: &str) -> Result<Vec<Block>>;
}

// ---------------------------------------------------------------------------
// SqliteBlockStore
// ---------------------------------------------------------------------------

pub struct SqliteBlockStore {
    pool: DbPool,
}

impl SqliteBlockStore {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| RtError::Internal(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Helper: row -> Block (without tokens / runs)
// ---------------------------------------------------------------------------

fn row_to_block(row: &rusqlite::Row<'_>) -> rusqlite::Result<Block> {
    let id_str: String = row.get(0)?;
    let document_id_str: String = row.get(1)?;
    let parent_id_str: Option<String> = row.get(2)?;
    let block_type_str: String = row.get(3)?;
    let level: i64 = row.get(4)?;
    let structural_path: String = row.get(5)?;
    let anchor_signature: String = row.get(6)?;
    let clause_hash: String = row.get(7)?;
    let canonical_text: String = row.get(8)?;
    let display_text: String = row.get(9)?;
    let formatting_meta_json: String = row.get(10)?;
    let position_index: i64 = row.get(11)?;

    let id = Uuid::parse_str(&id_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
    let document_id = Uuid::parse_str(&document_id_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e)))?;
    let parent_id = parent_id_str
        .map(|s| Uuid::parse_str(&s))
        .transpose()
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e)))?;

    let formatting_meta: FormattingMeta =
        serde_json::from_str(&formatting_meta_json).unwrap_or_default();

    Ok(Block {
        id,
        document_id,
        parent_id,
        block_type: BlockType::from(block_type_str.as_str()),
        level: level as i32,
        structural_path,
        anchor_signature,
        clause_hash,
        canonical_text,
        display_text,
        formatting_meta,
        position_index: position_index as i32,
        tokens: Vec::new(),
        runs: Vec::new(),
        children: Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// Helper: row -> Token
// ---------------------------------------------------------------------------

fn row_to_token(row: &rusqlite::Row<'_>) -> rusqlite::Result<Token> {
    // Columns: seq, text, kind, normalized, offset
    let _seq: i64 = row.get(0)?;
    let text: String = row.get(1)?;
    let kind_str: String = row.get(2)?;
    let normalized: String = row.get(3)?;
    let offset: i64 = row.get(4)?;

    Ok(Token {
        text,
        kind: TokenKind::from(kind_str.as_str()),
        normalized,
        offset: offset as usize,
    })
}

// ---------------------------------------------------------------------------
// Helper: row -> Run
// ---------------------------------------------------------------------------

fn row_to_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<Run> {
    // Columns: seq, text, bold, italic, underline, strikethrough, font_size, color
    let _seq: i64 = row.get(0)?;
    let text: String = row.get(1)?;
    let bold: i32 = row.get(2)?;
    let italic: i32 = row.get(3)?;
    let underline: i32 = row.get(4)?;
    let strikethrough: i32 = row.get(5)?;
    let font_size: Option<f64> = row.get(6)?;
    let color: Option<String> = row.get(7)?;

    Ok(Run {
        text,
        formatting: RunFormatting {
            bold: bold != 0,
            italic: italic != 0,
            underline: underline != 0,
            strikethrough: strikethrough != 0,
            font_size: font_size.map(|v| v as f32),
            color,
        },
    })
}

// ---------------------------------------------------------------------------
// Helpers: populate tokens + runs onto a flat block list
// ---------------------------------------------------------------------------

fn populate_tokens_and_runs(
    conn: &rusqlite::Connection,
    blocks: &mut Vec<Block>,
) -> Result<()> {
    for block in blocks.iter_mut() {
        let mut stmt = conn.prepare_cached(
            "SELECT seq, text, kind, normalized, offset
               FROM tokens
              WHERE block_id = ?1
              ORDER BY seq ASC",
        )?;
        let tokens: Vec<Token> = stmt
            .query_map(params![block.id.to_string()], row_to_token)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        block.tokens = tokens;

        let mut stmt = conn.prepare_cached(
            "SELECT seq, text, bold, italic, underline, strikethrough, font_size, color
               FROM runs
              WHERE block_id = ?1
              ORDER BY seq ASC",
        )?;
        let runs: Vec<Run> = stmt
            .query_map(params![block.id.to_string()], row_to_run)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        block.runs = runs;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers: insert a single block's sub-rows
// ---------------------------------------------------------------------------

fn insert_block_row(conn: &rusqlite::Connection, block: &Block) -> Result<()> {
    let formatting_meta_json = serde_json::to_string(&block.formatting_meta)?;

    conn.execute(
        "INSERT INTO blocks
            (id, document_id, parent_id, block_type, level, structural_path,
             anchor_signature, clause_hash, canonical_text, display_text,
             formatting_meta, position_index)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            block.id.to_string(),
            block.document_id.to_string(),
            block.parent_id.map(|u| u.to_string()),
            block.block_type.as_str(),
            block.level as i64,
            block.structural_path,
            block.anchor_signature,
            block.clause_hash,
            block.canonical_text,
            block.display_text,
            formatting_meta_json,
            block.position_index as i64,
        ],
    )?;

    for (seq, token) in block.tokens.iter().enumerate() {
        conn.execute(
            "INSERT INTO tokens (id, block_id, seq, text, kind, normalized, offset)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                Uuid::new_v4().to_string(),
                block.id.to_string(),
                seq as i64,
                token.text,
                token.kind.as_str(),
                token.normalized,
                token.offset as i64,
            ],
        )?;
    }

    for (seq, run) in block.runs.iter().enumerate() {
        conn.execute(
            "INSERT INTO runs
                (id, block_id, seq, text, bold, italic, underline, strikethrough,
                 font_size, color)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                Uuid::new_v4().to_string(),
                block.id.to_string(),
                seq as i64,
                run.text,
                run.formatting.bold as i32,
                run.formatting.italic as i32,
                run.formatting.underline as i32,
                run.formatting.strikethrough as i32,
                run.formatting.font_size.map(|v| v as f64),
                run.formatting.color,
            ],
        )?;
    }

    if let Some(tc) = &block.formatting_meta.tracked_change {
        insert_tracked_change(conn, tc, &block.id)?;
    }

    Ok(())
}

fn insert_tracked_change(
    conn: &rusqlite::Connection,
    tc: &TrackedChange,
    block_id: &Uuid,
) -> Result<()> {
    conn.execute(
        "INSERT INTO tracked_changes
            (id, block_id, author, changed_at, change_type, original)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            Uuid::new_v4().to_string(),
            block_id.to_string(),
            tc.author,
            tc.date.to_rfc3339(),
            tc.change_type.as_str(),
            tc.original,
        ],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Helper: build block tree from flat list
// ---------------------------------------------------------------------------

fn build_tree(flat: Vec<Block>) -> Vec<Block> {
    use std::collections::HashMap;

    let mut map: HashMap<Uuid, Block> = flat.into_iter().map(|b| (b.id, b)).collect();

    let child_ids: Vec<(Uuid, Uuid)> = map
        .values()
        .filter_map(|b| b.parent_id.map(|pid| (pid, b.id)))
        .collect();

    let mut pending: Vec<(Uuid, Uuid)> = Vec::new();
    for (pid, cid) in child_ids {
        if map.contains_key(&pid) {
            pending.push((pid, cid));
        }
    }

    let mut children_map: HashMap<Uuid, Vec<Block>> = HashMap::new();
    for (pid, cid) in &pending {
        if let Some(child) = map.remove(cid) {
            children_map.entry(*pid).or_default().push(child);
        }
    }

    for children in children_map.values_mut() {
        children.sort_by_key(|b| b.position_index);
    }

    fn attach(block: &mut Block, children_map: &mut HashMap<Uuid, Vec<Block>>) {
        if let Some(mut kids) = children_map.remove(&block.id) {
            for kid in &mut kids {
                attach(kid, children_map);
            }
            block.children = kids;
        }
    }

    let mut roots: Vec<Block> = map.into_values().collect();
    for root in &mut roots {
        attach(root, &mut children_map);
    }

    roots.sort_by_key(|b| b.position_index);
    roots
}

// ---------------------------------------------------------------------------
// BlockStore implementation
// ---------------------------------------------------------------------------

impl BlockStore for SqliteBlockStore {
    fn insert_document(&self, doc: &Document) -> Result<()> {
        let conn = self.conn()?;
        let metadata_json = serde_json::to_string(&doc.metadata)?;

        conn.execute(
            "INSERT INTO documents
                (id, name, source_path, doc_type, schema_version,
                 normalization_version, hash_contract_version, ingested_at, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                doc.id.to_string(),
                doc.name,
                doc.source_path,
                doc.doc_type.as_str(),
                doc.schema_version,
                doc.normalization_version,
                doc.hash_contract_version,
                doc.ingested_at.to_rfc3339(),
                metadata_json,
            ],
        )?;
        Ok(())
    }

    fn get_document(&self, id: &Uuid) -> Result<Document> {
        let conn = self.conn()?;

        let result = conn.query_row(
            "SELECT id, name, source_path, doc_type, schema_version,
                    normalization_version, hash_contract_version, ingested_at, metadata
               FROM documents
              WHERE id = ?1",
            params![id.to_string()],
            |row| {
                let id_str: String = row.get(0)?;
                let name: String = row.get(1)?;
                let source_path: Option<String> = row.get(2)?;
                let doc_type_str: String = row.get(3)?;
                let schema_version: String = row.get(4)?;
                let normalization_version: String = row.get(5)?;
                let hash_contract_version: String = row.get(6)?;
                let ingested_at_str: String = row.get(7)?;
                let metadata_json: String = row.get(8)?;
                Ok((
                    id_str,
                    name,
                    source_path,
                    doc_type_str,
                    schema_version,
                    normalization_version,
                    hash_contract_version,
                    ingested_at_str,
                    metadata_json,
                ))
            },
        );

        match result {
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                Err(RtError::NotFound(format!("document {id}")))
            }
            Err(e) => Err(RtError::Database(e)),
            Ok((
                id_str,
                name,
                source_path,
                doc_type_str,
                schema_version,
                normalization_version,
                hash_contract_version,
                ingested_at_str,
                metadata_json,
            )) => {
                let doc_id = Uuid::parse_str(&id_str)
                    .map_err(|e| RtError::InvalidInput(e.to_string()))?;
                let ingested_at = chrono::DateTime::parse_from_rfc3339(&ingested_at_str)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .map_err(|e| RtError::InvalidInput(e.to_string()))?;
                let metadata: Option<serde_json::Value> =
                    serde_json::from_str(&metadata_json).ok();

                Ok(Document {
                    id: doc_id,
                    name,
                    source_path,
                    doc_type: DocumentType::from(doc_type_str.as_str()),
                    schema_version,
                    normalization_version,
                    hash_contract_version,
                    ingested_at,
                    metadata,
                })
            }
        }
    }

    fn insert_block(&self, block: &Block) -> Result<()> {
        let conn = self.conn()?;
        insert_block_row(&conn, block)
    }

    fn insert_blocks(&self, blocks: &[Block]) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;

        for block in blocks {
            insert_block_row(&tx, block)?;
        }

        tx.commit()?;
        Ok(())
    }

    fn get_blocks_by_document(&self, doc_id: &Uuid) -> Result<Vec<Block>> {
        let conn = self.conn()?;

        let mut stmt = conn.prepare(
            "SELECT id, document_id, parent_id, block_type, level, structural_path,
                    anchor_signature, clause_hash, canonical_text, display_text,
                    formatting_meta, position_index
               FROM blocks
              WHERE document_id = ?1
              ORDER BY position_index ASC",
        )?;

        let mut blocks: Vec<Block> = stmt
            .query_map(params![doc_id.to_string()], row_to_block)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        populate_tokens_and_runs(&conn, &mut blocks)?;
        Ok(blocks)
    }

    fn get_block(&self, id: &Uuid) -> Result<Block> {
        let conn = self.conn()?;

        let result = conn.query_row(
            "SELECT id, document_id, parent_id, block_type, level, structural_path,
                    anchor_signature, clause_hash, canonical_text, display_text,
                    formatting_meta, position_index
               FROM blocks
              WHERE id = ?1",
            params![id.to_string()],
            row_to_block,
        );

        let mut block = match result {
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(RtError::NotFound(format!("block {id}")));
            }
            Err(e) => return Err(RtError::Database(e)),
            Ok(b) => b,
        };

        let mut blocks = vec![block];
        populate_tokens_and_runs(&conn, &mut blocks)?;
        block = blocks.remove(0);
        Ok(block)
    }

    fn get_block_children(&self, parent_id: &Uuid) -> Result<Vec<Block>> {
        let conn = self.conn()?;

        let mut stmt = conn.prepare(
            "SELECT id, document_id, parent_id, block_type, level, structural_path,
                    anchor_signature, clause_hash, canonical_text, display_text,
                    formatting_meta, position_index
               FROM blocks
              WHERE parent_id = ?1
              ORDER BY position_index ASC",
        )?;

        let mut blocks: Vec<Block> = stmt
            .query_map(params![parent_id.to_string()], row_to_block)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        populate_tokens_and_runs(&conn, &mut blocks)?;
        Ok(blocks)
    }

    fn get_block_tree(&self, doc_id: &Uuid) -> Result<Vec<Block>> {
        let flat = self.get_blocks_by_document(doc_id)?;
        Ok(build_tree(flat))
    }

    fn update_block(&self, block: &Block) -> Result<()> {
        let conn = self.conn()?;
        let formatting_meta_json = serde_json::to_string(&block.formatting_meta)?;

        let affected = conn.execute(
            "UPDATE blocks
                SET document_id      = ?2,
                    parent_id        = ?3,
                    block_type       = ?4,
                    level            = ?5,
                    structural_path  = ?6,
                    anchor_signature = ?7,
                    clause_hash      = ?8,
                    canonical_text   = ?9,
                    display_text     = ?10,
                    formatting_meta  = ?11,
                    position_index   = ?12
              WHERE id = ?1",
            params![
                block.id.to_string(),
                block.document_id.to_string(),
                block.parent_id.map(|u| u.to_string()),
                block.block_type.as_str(),
                block.level as i64,
                block.structural_path,
                block.anchor_signature,
                block.clause_hash,
                block.canonical_text,
                block.display_text,
                formatting_meta_json,
                block.position_index as i64,
            ],
        )?;

        if affected == 0 {
            return Err(RtError::NotFound(format!("block {}", block.id)));
        }
        Ok(())
    }

    fn delete_block(&self, id: &Uuid) -> Result<()> {
        let conn = self.conn()?;

        let affected =
            conn.execute("DELETE FROM blocks WHERE id = ?1", params![id.to_string()])?;

        if affected == 0 {
            return Err(RtError::NotFound(format!("block {id}")));
        }
        Ok(())
    }

    fn get_blocks_by_anchor(&self, anchor_signature: &str) -> Result<Vec<Block>> {
        let conn = self.conn()?;

        let mut stmt = conn.prepare(
            "SELECT id, document_id, parent_id, block_type, level, structural_path,
                    anchor_signature, clause_hash, canonical_text, display_text,
                    formatting_meta, position_index
               FROM blocks
              WHERE anchor_signature = ?1
              ORDER BY position_index ASC",
        )?;

        let mut blocks: Vec<Block> = stmt
            .query_map(params![anchor_signature], row_to_block)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        populate_tokens_and_runs(&conn, &mut blocks)?;
        Ok(blocks)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{BlockType, DocumentType, FormattingMeta, Run, RunFormatting, Token, TokenKind};
    use crate::schema::SCHEMA_VERSION;
    use chrono::Utc;

    fn make_store() -> SqliteBlockStore {
        let pool = create_memory_pool().expect("memory pool");
        SqliteBlockStore::new(pool)
    }

    fn make_doc() -> Document {
        Document {
            id: Uuid::new_v4(),
            name: "Test Document".into(),
            source_path: Some("/tmp/test.docx".into()),
            doc_type: DocumentType::Original,
            schema_version: SCHEMA_VERSION.into(),
            normalization_version: "1.0.0".into(),
            hash_contract_version: "1.0.0".into(),
            ingested_at: Utc::now(),
            metadata: Some(serde_json::json!({"author": "tester"})),
        }
    }

    fn make_block(doc_id: Uuid, position_index: i32) -> Block {
        Block {
            id: Uuid::new_v4(),
            document_id: doc_id,
            parent_id: None,
            block_type: BlockType::Paragraph,
            level: 0,
            structural_path: format!("{position_index}"),
            anchor_signature: format!("anchor-{position_index}"),
            clause_hash: "abc123".into(),
            canonical_text: "hello world".into(),
            display_text: "Hello World".into(),
            formatting_meta: FormattingMeta::default(),
            position_index,
            tokens: vec![Token {
                text: "hello".into(),
                kind: TokenKind::Word,
                normalized: "hello".into(),
                offset: 0,
            }],
            runs: vec![Run {
                text: "Hello World".into(),
                formatting: RunFormatting {
                    font_size: Some(12.0),
                    ..RunFormatting::default()
                },
            }],
            children: Vec::new(),
        }
    }

    #[test]
    fn insert_and_get_document() {
        let store = make_store();
        let doc = make_doc();
        store.insert_document(&doc).expect("insert");
        let fetched = store.get_document(&doc.id).expect("get");
        assert_eq!(fetched.id, doc.id);
        assert_eq!(fetched.name, doc.name);
    }

    #[test]
    fn get_document_not_found() {
        let store = make_store();
        let result = store.get_document(&Uuid::new_v4());
        assert!(matches!(result, Err(RtError::NotFound(_))));
    }

    #[test]
    fn insert_and_get_block() {
        let store = make_store();
        let doc = make_doc();
        store.insert_document(&doc).unwrap();

        let block = make_block(doc.id, 0);
        store.insert_block(&block).unwrap();

        let fetched = store.get_block(&block.id).unwrap();
        assert_eq!(fetched.id, block.id);
        assert_eq!(fetched.canonical_text, block.canonical_text);
        assert_eq!(fetched.tokens.len(), 1);
        assert_eq!(fetched.runs.len(), 1);
    }

    #[test]
    fn insert_blocks_transaction() {
        let store = make_store();
        let doc = make_doc();
        store.insert_document(&doc).unwrap();

        let blocks: Vec<Block> = (0..5).map(|i| make_block(doc.id, i)).collect();
        store.insert_blocks(&blocks).unwrap();

        let fetched = store.get_blocks_by_document(&doc.id).unwrap();
        assert_eq!(fetched.len(), 5);
    }

    #[test]
    fn get_blocks_by_document_ordered() {
        let store = make_store();
        let doc = make_doc();
        store.insert_document(&doc).unwrap();

        for i in [3i32, 1, 4, 0, 2] {
            let mut b = make_block(doc.id, i);
            b.structural_path = i.to_string();
            store.insert_block(&b).unwrap();
        }

        let fetched = store.get_blocks_by_document(&doc.id).unwrap();
        let indices: Vec<i32> = fetched.iter().map(|b| b.position_index).collect();
        assert_eq!(indices, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn get_block_children() {
        let store = make_store();
        let doc = make_doc();
        store.insert_document(&doc).unwrap();

        let mut parent = make_block(doc.id, 0);
        parent.structural_path = "0".into();
        store.insert_block(&parent).unwrap();

        for i in 0..3i32 {
            let mut child = make_block(doc.id, i);
            child.parent_id = Some(parent.id);
            child.structural_path = format!("0.{i}");
            child.anchor_signature = format!("child-anchor-{i}");
            store.insert_block(&child).unwrap();
        }

        let children = store.get_block_children(&parent.id).unwrap();
        assert_eq!(children.len(), 3);
    }

    #[test]
    fn get_block_tree_builds_hierarchy() {
        let store = make_store();
        let doc = make_doc();
        store.insert_document(&doc).unwrap();

        let mut root = make_block(doc.id, 0);
        root.structural_path = "0".into();
        store.insert_block(&root).unwrap();

        let mut child = make_block(doc.id, 0);
        child.parent_id = Some(root.id);
        child.structural_path = "0.0".into();
        child.anchor_signature = "child-anchor".into();
        store.insert_block(&child).unwrap();

        let tree = store.get_block_tree(&doc.id).unwrap();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].children.len(), 1);
    }

    #[test]
    fn update_block() {
        let store = make_store();
        let doc = make_doc();
        store.insert_document(&doc).unwrap();

        let mut block = make_block(doc.id, 0);
        store.insert_block(&block).unwrap();

        block.canonical_text = "updated text".into();
        store.update_block(&block).unwrap();

        let fetched = store.get_block(&block.id).unwrap();
        assert_eq!(fetched.canonical_text, "updated text");
    }

    #[test]
    fn delete_block() {
        let store = make_store();
        let doc = make_doc();
        store.insert_document(&doc).unwrap();

        let block = make_block(doc.id, 0);
        store.insert_block(&block).unwrap();
        store.delete_block(&block.id).unwrap();

        let result = store.get_block(&block.id);
        assert!(matches!(result, Err(RtError::NotFound(_))));
    }

    #[test]
    fn get_blocks_by_anchor() {
        let store = make_store();
        let doc = make_doc();
        store.insert_document(&doc).unwrap();

        let block = make_block(doc.id, 0);
        let sig = block.anchor_signature.clone();
        store.insert_block(&block).unwrap();

        let found = store.get_blocks_by_anchor(&sig).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, block.id);
    }
}
