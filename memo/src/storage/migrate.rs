use rusqlite::{Connection, TransactionBehavior};

use crate::error::{Error, Result};

/// Current `PRAGMA user_version`. Append a migration rather than editing an
/// existing one.
pub const SCHEMA_VERSION: u32 = 1;

const MIGRATIONS: &[(u32, &str)] = &[(1, MIGRATION_V1)];

/// Initial schema.
///
/// `embeddings` is reserved for a future table keyed by `memories.id`. It is
/// intentionally not created here.
const MIGRATION_V1: &str = "
CREATE TABLE memories (
    id INTEGER PRIMARY KEY,
    title TEXT,
    content TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'note' CHECK (kind IN ('fact', 'decision', 'note')),
    scope TEXT NOT NULL DEFAULT 'global',
    source_agent TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    expires_at TEXT,
    content_hash TEXT NOT NULL,
    UNIQUE (scope, content_hash)
);

CREATE INDEX idx_memories_expires_at ON memories (expires_at);

CREATE TABLE tags (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE memory_tags (
    memory_id INTEGER NOT NULL REFERENCES memories (id) ON DELETE CASCADE,
    tag_id INTEGER NOT NULL REFERENCES tags (id) ON DELETE CASCADE,
    PRIMARY KEY (memory_id, tag_id)
);

CREATE INDEX idx_memory_tags_tag_id ON memory_tags (tag_id);

CREATE VIRTUAL TABLE memories_fts USING fts5 (
    title,
    content,
    tags
);
";

pub(crate) fn migrate(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let current: i64 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let supported = i64::from(SCHEMA_VERSION);
    if current > supported {
        return Err(Error::NewerSchema {
            found: current,
            supported,
        });
    }
    for (version, sql) in MIGRATIONS {
        if current < i64::from(*version) {
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", i64::from(*version))?;
        }
    }
    tx.commit()?;
    Ok(())
}
