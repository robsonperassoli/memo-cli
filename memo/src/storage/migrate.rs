use rusqlite::{Connection, TransactionBehavior};

use crate::error::{Error, Result};

/// Current `PRAGMA user_version`. Append a migration rather than editing an
/// existing one.
pub const SCHEMA_VERSION: u32 = 2;

const MIGRATIONS: &[(u32, &str)] = &[(1, MIGRATION_V1), (2, MIGRATION_V2)];

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

/// `memories` becomes the stable view. Rows live in `memory_rows`.
const MIGRATION_V2: &str = "
ALTER TABLE memories RENAME TO memory_rows;

CREATE VIEW memories AS
SELECT
    memory_rows.id AS id,
    memory_rows.title AS title,
    memory_rows.content AS content,
    memory_rows.kind AS kind,
    memory_rows.scope AS scope,
    COALESCE(
        (
            SELECT json_group_array(tags.name ORDER BY tags.name)
            FROM memory_tags
            JOIN tags ON tags.id = memory_tags.tag_id
            WHERE memory_tags.memory_id = memory_rows.id
        ),
        '[]'
    ) AS tags,
    memory_rows.source_agent AS source_agent,
    memory_rows.created_at AS created_at,
    memory_rows.updated_at AS updated_at,
    memory_rows.expires_at AS expires_at
FROM memory_rows;
";

#[cfg(test)]
pub(crate) fn create_v1_database(path: &std::path::Path) -> Result<()> {
    let conn = Connection::open(path)?;
    conn.execute_batch(MIGRATION_V1)?;
    conn.pragma_update(None, "user_version", 1i64)?;
    conn.execute(
        "INSERT INTO memories (
            content, kind, scope, created_at, updated_at, content_hash
         ) VALUES (
            'migrated body', 'note', 'global',
            '2020-01-01T00:00:00.000Z', '2020-01-02T00:00:00.000Z', 'hash-one'
         )",
        [],
    )?;
    conn.execute("INSERT INTO tags (name) VALUES ('zeta')", [])?;
    conn.execute("INSERT INTO tags (name) VALUES ('alpha')", [])?;
    conn.execute(
        "INSERT INTO memory_tags (memory_id, tag_id) SELECT 1, id FROM tags",
        [],
    )?;
    Ok(())
}

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
