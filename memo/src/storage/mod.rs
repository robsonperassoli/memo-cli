//! SQLite store for shared agent memory.
//!
//! Writers take an immediate transaction. There is no per-agent ownership:
//! any caller may update or delete any row. Expired rows stay in the database
//! and are omitted from reads unless [`ReadOptions::include_expired`] is set.

mod markdown;
mod migrate;
mod path;
mod query;

pub use query::compile_fts_query;

use std::fs;
use std::path::Path;
use std::time::Duration;

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};

pub use migrate::SCHEMA_VERSION;
pub use path::resolve_db_path;

use crate::error::{Error, Result};
use markdown::indexed_text;

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
/// `SQLITE_CONSTRAINT_UNIQUE`.
const SQLITE_CONSTRAINT_UNIQUE: i32 = 2067;

/// `fact`, `decision`, or `note`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MemoryKind {
    Fact,
    Decision,
    #[default]
    Note,
}

impl MemoryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fact => "fact",
            Self::Decision => "decision",
            Self::Note => "note",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "fact" => Ok(Self::Fact),
            "decision" => Ok(Self::Decision),
            "note" => Ok(Self::Note),
            other => Err(Error::InvalidKind(other.to_owned())),
        }
    }
}

impl std::fmt::Display for MemoryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A field on update: leave it, replace it, or set it to NULL.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Patch<T> {
    #[default]
    Unchanged,
    Set(T),
    Clear,
}

/// Values for a new memory. `content` is stored unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMemory {
    pub title: Option<String>,
    pub content: String,
    pub kind: MemoryKind,
    pub scope: String,
    pub source_agent: Option<String>,
    pub tags: Vec<String>,
    /// RFC 3339 timestamp. Stored as canonical UTC.
    pub expires_at: Option<String>,
}

impl Default for NewMemory {
    fn default() -> Self {
        Self {
            title: None,
            content: String::new(),
            kind: MemoryKind::Note,
            scope: "global".to_owned(),
            source_agent: None,
            tags: Vec::new(),
            expires_at: None,
        }
    }
}

/// Changes applied by [`Store::update`]. `None` and [`Patch::Unchanged`] leave
/// the current value in place.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MemoryUpdate {
    pub title: Patch<String>,
    pub content: Option<String>,
    pub kind: Option<MemoryKind>,
    pub scope: Option<String>,
    pub source_agent: Patch<String>,
    pub expires_at: Patch<String>,
    pub tags: Option<Vec<String>>,
}

/// Filters for [`Store::list`].
///
/// [`Store::search`] uses `kind`, `tag`, `include_expired`, and `limit`.
/// Search scope is [`ScopeQuery`]; `scope` on this struct is ignored there.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReadOptions {
    pub scope: Option<String>,
    pub kind: Option<MemoryKind>,
    pub tag: Option<String>,
    pub include_expired: bool,
    /// Maximum rows. `None` means no limit.
    pub limit: Option<u32>,
}

/// Which scopes [`Store::search`] reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeQuery {
    /// Every scope.
    All,
    /// `scope` plus `global`.
    WithGlobal(String),
    /// Exactly this scope.
    Only(String),
}

/// One full-text hit. `rank` is the FTS5 `bm25` score; lower is a stronger match.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub memory: Memory,
    pub rank: f64,
    pub snippet: String,
}

/// Unexpired memories in one scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeSummary {
    pub scope: String,
    pub count: i64,
    pub last_updated: Option<String>,
}

/// One stored memory, including its tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Memory {
    pub id: i64,
    pub title: Option<String>,
    pub content: String,
    pub kind: MemoryKind,
    pub scope: String,
    pub source_agent: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: Option<String>,
    pub content_hash: String,
    pub tags: Vec<String>,
}

/// Result of [`Store::insert`]. Duplicate content returns `created: false`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsertResult {
    pub memory: Memory,
    pub created: bool,
}

struct PreparedMemory {
    title: Option<String>,
    content: String,
    kind: MemoryKind,
    scope: String,
    source_agent: Option<String>,
    tags: Vec<String>,
    expires_at: Option<String>,
    content_hash: String,
}

impl PreparedMemory {
    fn prepare(new: &NewMemory) -> Result<Self> {
        Ok(Self {
            title: new.title.as_deref().and_then(blank_to_none),
            content: new.content.clone(),
            kind: new.kind,
            scope: normalize_scope(&new.scope)?,
            source_agent: new.source_agent.as_deref().and_then(blank_to_none),
            tags: normalize_tags(&new.tags),
            expires_at: canonicalize_optional(new.expires_at.as_deref())?,
            content_hash: content_hash(&new.content),
        })
    }
}

/// Open database for shared memories.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Open `path`, creating parent directories and applying migrations.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let mut conn = open_connection(path)?;
        migrate::migrate(&mut conn)?;
        Ok(Self { conn })
    }

    pub fn schema_version(&self) -> Result<u32> {
        let version: i64 = self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))?;
        u32::try_from(version).map_err(|_| Error::NewerSchema {
            found: version,
            supported: i64::from(SCHEMA_VERSION),
        })
    }

    /// Insert `new`, or return the existing row when the same normalized
    /// content already exists in the scope. The existing row is not modified.
    pub fn insert(&mut self, new: &NewMemory) -> Result<InsertResult> {
        let prepared = PreparedMemory::prepare(new)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) = find_by_hash(&tx, &prepared.scope, &prepared.content_hash)? {
            tx.commit()?;
            return Ok(InsertResult {
                memory: existing,
                created: false,
            });
        }
        match insert_row(&tx, &prepared) {
            Ok(memory) => {
                tx.commit()?;
                Ok(InsertResult {
                    memory,
                    created: true,
                })
            }
            Err(Error::Sqlite(err)) if is_unique_violation(&err) => {
                drop(tx);
                let memory = find_by_hash(&self.conn, &prepared.scope, &prepared.content_hash)?
                    .ok_or_else(|| Error::Duplicate {
                        scope: prepared.scope.clone(),
                    })?;
                Ok(InsertResult {
                    memory,
                    created: false,
                })
            }
            Err(err) => Err(err),
        }
    }

    /// Load one memory. Expired rows are omitted unless `include_expired` is set.
    pub fn get(&self, id: i64) -> Result<Option<Memory>> {
        load_memory(&self.conn, id, false)
    }

    pub fn get_including_expired(&self, id: i64) -> Result<Option<Memory>> {
        load_memory(&self.conn, id, true)
    }

    pub fn list(&self, options: &ReadOptions) -> Result<Vec<Memory>> {
        let sql = "
            SELECT m.id
            FROM memories m
            WHERE (?1 OR m.expires_at IS NULL OR m.expires_at > ?2)
              AND (?3 IS NULL OR m.scope = ?3)
              AND (?4 IS NULL OR m.kind = ?4)
              AND (
                    ?5 IS NULL
                    OR EXISTS (
                        SELECT 1
                        FROM memory_tags mt
                        JOIN tags t ON t.id = mt.tag_id
                        WHERE mt.memory_id = m.id AND t.name = ?5
                    )
                  )
            ORDER BY m.created_at DESC, m.id DESC
            LIMIT ?6
        ";
        self.collect_memories(sql, options, None)
    }

    /// Full-text search over indexed title, content, and tags.
    ///
    /// `query` is an FTS5 `MATCH` expression. An empty query returns no rows.
    /// Expired rows are omitted unless `options.include_expired` is set.
    /// Results are ordered by `bm25` (lower is a stronger match), then id.
    /// Invalid FTS5 syntax is [`Error::InvalidSearch`].
    pub fn search(
        &self,
        query: &str,
        scope: &ScopeQuery,
        options: &ReadOptions,
    ) -> Result<Vec<SearchHit>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let (mode, scope_value) = scope_bind(scope)?;
        let sql = "
            SELECT m.id,
                   bm25(memories_fts),
                   snippet(memories_fts, -1, '', '', '...', 12)
            FROM memories_fts
            JOIN memories m ON m.id = memories_fts.rowid
            WHERE memories_fts MATCH ?1
              AND (?2 OR m.expires_at IS NULL OR m.expires_at > ?3)
              AND (
                    ?4 = 0
                    OR (?4 = 1 AND (m.scope = ?5 OR m.scope = 'global'))
                    OR (?4 = 2 AND m.scope = ?5)
                  )
              AND (?6 IS NULL OR m.kind = ?6)
              AND (
                    ?7 IS NULL
                    OR EXISTS (
                        SELECT 1
                        FROM memory_tags mt
                        JOIN tags t ON t.id = mt.tag_id
                        WHERE mt.memory_id = m.id AND t.name = ?7
                    )
                  )
            ORDER BY bm25(memories_fts), m.id
            LIMIT ?8
        ";
        let now = now_utc();
        let kind = options.kind.map(|kind| kind.as_str().to_owned());
        let tag = options.tag.as_deref().and_then(blank_to_none);
        let limit = options.limit.map_or(-1, i64::from);
        let mut stmt = self.conn.prepare(sql)?;
        let mut query_rows = stmt
            .query(params![
                query,
                options.include_expired,
                now,
                mode,
                scope_value,
                kind,
                tag,
                limit
            ])
            .map_err(map_fts_error)?;
        let mut rows = Vec::new();
        loop {
            let Some(row) = query_rows.next().map_err(map_fts_error)? else {
                break;
            };
            rows.push((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            ));
        }
        drop(query_rows);
        drop(stmt);
        let mut hits = Vec::with_capacity(rows.len());
        for (id, rank, snippet) in rows {
            if let Some(memory) = load_memory(&self.conn, id, true)? {
                hits.push(SearchHit {
                    memory,
                    rank,
                    snippet,
                });
            }
        }
        Ok(hits)
    }

    /// Each scope that has an unexpired memory, plus `global` when it has none.
    ///
    /// `global` is first. Other scopes follow in name order. `count` skips
    /// expired rows. `last_updated` is the newest `updated_at` in that scope.
    pub fn scopes(&self) -> Result<Vec<ScopeSummary>> {
        let now = now_utc();
        let mut stmt = self.conn.prepare(
            "SELECT scope, COUNT(*), MAX(updated_at)
             FROM memories
             WHERE expires_at IS NULL OR expires_at > ?1
             GROUP BY scope",
        )?;
        let mut rows = stmt
            .query_map(params![now], |row| {
                Ok(ScopeSummary {
                    scope: row.get(0)?,
                    count: row.get(1)?,
                    last_updated: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);
        if !rows.iter().any(|row| row.scope == "global") {
            rows.push(ScopeSummary {
                scope: "global".to_owned(),
                count: 0,
                last_updated: None,
            });
        }
        rows.sort_by(|left, right| scope_list_order(&left.scope, &right.scope));
        Ok(rows)
    }

    /// Update any memory. `source_agent` is not an access check.
    ///
    /// Returns `Ok(None)` when `id` does not exist. A content collision with a
    /// different row in the target scope is [`Error::Duplicate`].
    pub fn update(&mut self, id: i64, update: &MemoryUpdate) -> Result<Option<Memory>> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let Some(current) = load_memory(&tx, id, true)? else {
            return Ok(None);
        };
        let title = match &update.title {
            Patch::Unchanged => current.title.clone(),
            Patch::Clear => None,
            Patch::Set(value) => blank_to_none(value),
        };
        let content = update
            .content
            .clone()
            .unwrap_or_else(|| current.content.clone());
        let kind = update.kind.unwrap_or(current.kind);
        let scope = match &update.scope {
            Some(scope) => normalize_scope(scope)?,
            None => current.scope.clone(),
        };
        let source_agent = match &update.source_agent {
            Patch::Unchanged => current.source_agent.clone(),
            Patch::Clear => None,
            Patch::Set(value) => blank_to_none(value),
        };
        let expires_at = match &update.expires_at {
            Patch::Unchanged => current.expires_at.clone(),
            Patch::Clear => None,
            Patch::Set(value) => Some(canonicalize_timestamp(value)?),
        };
        let tags = match &update.tags {
            Some(tags) => normalize_tags(tags),
            None => current.tags.clone(),
        };
        let hash = content_hash(&content);
        if hash != current.content_hash || scope != current.scope {
            let conflict: Option<i64> = tx
                .query_row(
                    "SELECT id FROM memories
                     WHERE scope = ?1 AND content_hash = ?2 AND id != ?3",
                    params![scope, hash, id],
                    |row| row.get(0),
                )
                .optional()?;
            if conflict.is_some() {
                return Err(Error::Duplicate { scope });
            }
        }
        let now = now_utc();
        tx.execute(
            "UPDATE memories
             SET title = ?1,
                 content = ?2,
                 kind = ?3,
                 scope = ?4,
                 source_agent = ?5,
                 updated_at = ?6,
                 expires_at = ?7,
                 content_hash = ?8
             WHERE id = ?9",
            params![
                title,
                content,
                kind.as_str(),
                scope,
                source_agent,
                now,
                expires_at,
                hash,
                id
            ],
        )
        .map_err(|err| {
            if is_unique_violation(&err) {
                Error::Duplicate { scope }
            } else {
                Error::from(err)
            }
        })?;
        if update.tags.is_some() {
            replace_tags(&tx, id, &tags)?;
        }
        sync_fts(&tx, id, title.as_deref(), &content, &tags)?;
        tx.commit()?;
        load_memory(&self.conn, id, true)
    }

    /// Delete any memory, expired or not. Returns whether a row was removed.
    pub fn delete(&mut self, id: i64) -> Result<bool> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let exists: Option<i64> = tx
            .query_row("SELECT id FROM memories WHERE id = ?1", [id], |row| {
                row.get(0)
            })
            .optional()?;
        if exists.is_none() {
            return Ok(false);
        }
        delete_fts(&tx, id)?;
        tx.execute("DELETE FROM memories WHERE id = ?1", [id])?;
        delete_orphan_tags(&tx)?;
        tx.commit()?;
        Ok(true)
    }

    fn collect_memories(
        &self,
        sql: &str,
        options: &ReadOptions,
        fts_query: Option<&str>,
    ) -> Result<Vec<Memory>> {
        let now = now_utc();
        let scope = options.scope.clone();
        let kind = options.kind.map(|kind| kind.as_str().to_owned());
        let tag = options.tag.as_deref().and_then(blank_to_none);
        let mut stmt = self.conn.prepare(sql)?;
        let ids = if let Some(query) = fts_query {
            stmt.query_map(
                params![query, options.include_expired, now, scope, kind, tag],
                |row| row.get(0),
            )?
            .collect::<std::result::Result<Vec<i64>, _>>()?
        } else {
            let limit = options.limit.map_or(-1, i64::from);
            stmt.query_map(
                params![options.include_expired, now, scope, kind, tag, limit],
                |row| row.get(0),
            )?
            .collect::<std::result::Result<Vec<i64>, _>>()?
        };
        drop(stmt);
        let mut memories = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(memory) = load_memory(&self.conn, id, true)? {
                memories.push(memory);
            }
        }
        Ok(memories)
    }
}

fn open_connection(path: &Path) -> Result<Connection> {
    let deadline = std::time::Instant::now() + BUSY_TIMEOUT;
    loop {
        match Connection::open(path) {
            Ok(conn) => {
                configure(&conn)?;
                return Ok(conn);
            }
            Err(err) if is_locked(&err) && std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(err) => return Err(err.into()),
        }
    }
}

fn configure(conn: &Connection) -> Result<()> {
    conn.busy_timeout(BUSY_TIMEOUT)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    let mode: String =
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(Error::WalUnavailable(mode));
    }
    Ok(())
}

fn insert_row(tx: &Transaction<'_>, prepared: &PreparedMemory) -> Result<Memory> {
    let now = now_utc();
    tx.execute(
        "INSERT INTO memories (
            title, content, kind, scope, source_agent, created_at, updated_at, expires_at, content_hash
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            prepared.title,
            prepared.content,
            prepared.kind.as_str(),
            prepared.scope,
            prepared.source_agent,
            now,
            now,
            prepared.expires_at,
            prepared.content_hash
        ],
    )?;
    let id = tx.last_insert_rowid();
    replace_tags(tx, id, &prepared.tags)?;
    sync_fts(
        tx,
        id,
        prepared.title.as_deref(),
        &prepared.content,
        &prepared.tags,
    )?;
    load_memory(tx, id, true)?.ok_or(Error::Sqlite(rusqlite::Error::QueryReturnedNoRows))
}

fn find_by_hash(conn: &Connection, scope: &str, content_hash: &str) -> Result<Option<Memory>> {
    let id: Option<i64> = conn
        .query_row(
            "SELECT id FROM memories WHERE scope = ?1 AND content_hash = ?2",
            params![scope, content_hash],
            |row| row.get(0),
        )
        .optional()?;
    match id {
        Some(id) => load_memory(conn, id, true),
        None => Ok(None),
    }
}

fn load_memory(conn: &Connection, id: i64, include_expired: bool) -> Result<Option<Memory>> {
    let now = now_utc();
    let row = conn
        .query_row(
            "SELECT id, title, content, kind, scope, source_agent, created_at, updated_at,
                    expires_at, content_hash
             FROM memories
             WHERE id = ?1
               AND (?2 OR expires_at IS NULL OR expires_at > ?3)",
            params![id, include_expired, now],
            |row| {
                Ok(MemoryRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    content: row.get(2)?,
                    kind: row.get(3)?,
                    scope: row.get(4)?,
                    source_agent: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    expires_at: row.get(8)?,
                    content_hash: row.get(9)?,
                })
            },
        )
        .optional()?;
    let Some(row) = row else {
        return Ok(None);
    };
    let tags = load_tags(conn, id)?;
    Ok(Some(row.into_memory(tags)?))
}

struct MemoryRow {
    id: i64,
    title: Option<String>,
    content: String,
    kind: String,
    scope: String,
    source_agent: Option<String>,
    created_at: String,
    updated_at: String,
    expires_at: Option<String>,
    content_hash: String,
}

impl MemoryRow {
    fn into_memory(self, tags: Vec<String>) -> Result<Memory> {
        Ok(Memory {
            id: self.id,
            title: self.title,
            content: self.content,
            kind: MemoryKind::parse(&self.kind)?,
            scope: self.scope,
            source_agent: self.source_agent,
            created_at: self.created_at,
            updated_at: self.updated_at,
            expires_at: self.expires_at,
            content_hash: self.content_hash,
            tags,
        })
    }
}

fn load_tags(conn: &Connection, memory_id: i64) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.name
         FROM tags t
         JOIN memory_tags mt ON mt.tag_id = t.id
         WHERE mt.memory_id = ?1
         ORDER BY t.name",
    )?;
    let tags = stmt
        .query_map([memory_id], |row| row.get(0))?
        .collect::<std::result::Result<Vec<String>, _>>()?;
    Ok(tags)
}

fn replace_tags(conn: &Connection, memory_id: i64, tags: &[String]) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM memory_tags WHERE memory_id = ?1", [memory_id])?;
    for tag in tags {
        conn.execute("INSERT OR IGNORE INTO tags (name) VALUES (?1)", [tag])?;
        let tag_id: i64 = conn.query_row("SELECT id FROM tags WHERE name = ?1", [tag], |row| {
            row.get(0)
        })?;
        conn.execute(
            "INSERT INTO memory_tags (memory_id, tag_id) VALUES (?1, ?2)",
            params![memory_id, tag_id],
        )?;
    }
    delete_orphan_tags(conn)?;
    Ok(())
}

fn delete_orphan_tags(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM tags
         WHERE NOT EXISTS (
             SELECT 1 FROM memory_tags WHERE memory_tags.tag_id = tags.id
         )",
        [],
    )?;
    Ok(())
}

fn sync_fts(
    conn: &Connection,
    id: i64,
    title: Option<&str>,
    content: &str,
    tags: &[String],
) -> rusqlite::Result<()> {
    delete_fts(conn, id)?;
    let (title, content, tags) = fts_fields(title, content, tags);
    conn.execute(
        "INSERT INTO memories_fts (rowid, title, content, tags) VALUES (?1, ?2, ?3, ?4)",
        params![id, title, content, tags],
    )?;
    Ok(())
}

fn delete_fts(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM memories_fts WHERE rowid = ?1", [id])?;
    Ok(())
}

fn fts_fields(title: Option<&str>, content: &str, tags: &[String]) -> (String, String, String) {
    (
        indexed_text(title.unwrap_or("")),
        indexed_text(content),
        indexed_text(&tags.join(" ")),
    )
}

fn normalize_scope(scope: &str) -> Result<String> {
    let trimmed = scope.trim();
    if trimmed.is_empty() {
        return Err(Error::EmptyScope);
    }
    Ok(trimmed.to_owned())
}

fn scope_bind(scope: &ScopeQuery) -> Result<(i64, Option<String>)> {
    match scope {
        ScopeQuery::All => Ok((0, None)),
        ScopeQuery::WithGlobal(value) => Ok((1, Some(normalize_scope(value)?))),
        ScopeQuery::Only(value) => Ok((2, Some(normalize_scope(value)?))),
    }
}

fn scope_list_order(left: &str, right: &str) -> std::cmp::Ordering {
    match (left == "global", right == "global") {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => left.cmp(right),
    }
}

fn map_fts_error(err: rusqlite::Error) -> Error {
    let message = err.to_string();
    let lower = message.to_ascii_lowercase();
    if lower.contains("fts5") || lower.contains("syntax error") {
        Error::InvalidSearch(message)
    } else {
        Error::Sqlite(err)
    }
}

fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut names: Vec<String> = tags.iter().filter_map(|tag| blank_to_none(tag)).collect();
    names.sort();
    names.dedup();
    names
}

fn blank_to_none(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn canonicalize_optional(value: Option<&str>) -> Result<Option<String>> {
    value.map(canonicalize_timestamp).transpose()
}

fn canonicalize_timestamp(value: &str) -> Result<String> {
    let parsed = DateTime::parse_from_rfc3339(value.trim())
        .map_err(|_| Error::InvalidTimestamp(value.to_owned()))?;
    Ok(parsed
        .with_timezone(&Utc)
        .to_rfc3339_opts(SecondsFormat::Millis, true))
}

fn now_utc() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn content_hash(content: &str) -> String {
    let normalized = normalize_whitespace(content);
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    hex_encode(hasher.finalize().as_ref())
}

fn normalize_whitespace(content: &str) -> String {
    content.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn is_unique_violation(err: &rusqlite::Error) -> bool {
    err.sqlite_extended_error_code() == Some(SQLITE_CONSTRAINT_UNIQUE)
}

fn is_locked(err: &rusqlite::Error) -> bool {
    matches!(
        err.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_temp() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("memo.db")).unwrap();
        (dir, store)
    }

    fn new_content(content: &str) -> NewMemory {
        NewMemory {
            content: content.to_owned(),
            ..NewMemory::default()
        }
    }

    #[test]
    fn migrations_initialize_an_empty_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("memo").join("memo.db");
        assert!(!path.exists());

        let store = Store::open(&path).unwrap();
        assert!(path.is_file());
        assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);

        let journal: String = store
            .conn
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        assert_eq!(journal.to_ascii_lowercase(), "wal");

        let foreign_keys: i64 = store
            .conn
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        assert_eq!(foreign_keys, 1);

        let busy: i64 = store
            .conn
            .pragma_query_value(None, "busy_timeout", |row| row.get(0))
            .unwrap();
        assert_eq!(busy, 5000);

        let names = table_names(&store);
        for required in ["memories", "tags", "memory_tags", "memories_fts"] {
            assert!(names.iter().any(|name| name == required), "{names:?}");
        }
        assert!(!names.iter().any(|name| name == "embeddings"));

        let err = store
            .conn
            .execute(
                "INSERT INTO memory_tags (memory_id, tag_id) VALUES (1, 1)",
                [],
            )
            .unwrap_err();
        assert!(
            matches!(
                err.sqlite_error_code(),
                Some(rusqlite::ErrorCode::ConstraintViolation)
            ),
            "{err}"
        );
    }

    #[test]
    fn reopen_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memo.db");
        let memory_id;
        {
            let mut store = Store::open(&path).unwrap();
            let inserted = store.insert(&new_content("kept across reopen")).unwrap();
            assert!(inserted.created);
            memory_id = inserted.memory.id;
            assert_eq!(store.schema_version().unwrap(), 1);
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(store.schema_version().unwrap(), 1);
        let loaded = store.get(memory_id).unwrap().unwrap();
        assert_eq!(loaded.content, "kept across reopen");
        assert_eq!(
            table_names(&store)
                .iter()
                .filter(|name| *name == "memories")
                .count(),
            1
        );
    }

    #[test]
    fn duplicate_content_in_a_scope_returns_the_existing_memory() {
        let (_dir, mut store) = open_temp();
        let first = store
            .insert(&NewMemory {
                title: Some("original".to_owned()),
                content: "Hello   world\n".to_owned(),
                tags: vec!["one".to_owned()],
                source_agent: Some("alice".to_owned()),
                ..NewMemory::default()
            })
            .unwrap();
        let second = store
            .insert(&NewMemory {
                title: Some("ignored".to_owned()),
                content: "Hello world".to_owned(),
                tags: vec!["two".to_owned()],
                source_agent: Some("bob".to_owned()),
                ..NewMemory::default()
            })
            .unwrap();
        assert!(first.created);
        assert!(!second.created);
        assert_eq!(first.memory.id, second.memory.id);
        assert_eq!(second.memory.content, "Hello   world\n");
        assert_eq!(second.memory.title.as_deref(), Some("original"));
        assert_eq!(second.memory.tags, vec!["one".to_owned()]);
        assert_eq!(second.memory.source_agent.as_deref(), Some("alice"));
        assert_eq!(first.memory.content_hash, second.memory.content_hash);

        let other_scope = store
            .insert(&NewMemory {
                content: "Hello world".to_owned(),
                scope: "project".to_owned(),
                ..NewMemory::default()
            })
            .unwrap();
        assert!(other_scope.created);
        assert_ne!(other_scope.memory.id, first.memory.id);
        assert_eq!(store.list(&ReadOptions::default()).unwrap().len(), 2);
    }

    #[test]
    fn fts_stays_in_sync_across_update_and_delete() {
        let (_dir, mut store) = open_temp();
        let original = "See `codequark` and [labelquark](https://example.com/secretquark) #headingquark *boldquark*";
        let inserted = store
            .insert(&NewMemory {
                title: Some("# titlequark".to_owned()),
                content: original.to_owned(),
                kind: MemoryKind::Fact,
                tags: vec![" tagquark ".to_owned(), "tagquark".to_owned()],
                source_agent: Some("alice".to_owned()),
                ..NewMemory::default()
            })
            .unwrap();
        let id = inserted.memory.id;
        assert_eq!(inserted.memory.content, original);
        assert_eq!(inserted.memory.tags, vec!["tagquark".to_owned()]);
        assert_eq!(inserted.memory.kind, MemoryKind::Fact);

        assert_eq!(fts_rowid(&store, "codequark"), Some(id));
        assert_eq!(fts_rowid(&store, "labelquark"), Some(id));
        assert_eq!(fts_rowid(&store, "headingquark"), Some(id));
        assert_eq!(fts_rowid(&store, "boldquark"), Some(id));
        assert_eq!(fts_rowid(&store, "titlequark"), Some(id));
        assert_eq!(fts_rowid(&store, "tagquark"), Some(id));
        assert_eq!(fts_rowid(&store, "secretquark"), None);

        let updated = store
            .update(
                id,
                &MemoryUpdate {
                    title: Patch::Set("renamedquark".to_owned()),
                    content: Some("betaquark only".to_owned()),
                    tags: Some(vec!["pythonquark".to_owned()]),
                    source_agent: Patch::Set("bob".to_owned()),
                    kind: Some(MemoryKind::Decision),
                    ..MemoryUpdate::default()
                },
            )
            .unwrap()
            .unwrap();
        assert_eq!(updated.source_agent.as_deref(), Some("bob"));
        assert_eq!(updated.kind, MemoryKind::Decision);
        assert_eq!(updated.content, "betaquark only");
        assert_eq!(fts_rowid(&store, "betaquark"), Some(id));
        assert_eq!(fts_rowid(&store, "renamedquark"), Some(id));
        assert_eq!(fts_rowid(&store, "pythonquark"), Some(id));
        assert_eq!(fts_rowid(&store, "codequark"), None);
        assert_eq!(fts_rowid(&store, "titlequark"), None);
        assert_eq!(fts_rowid(&store, "tagquark"), None);
        assert_eq!(fts_rowid(&store, "secretquark"), None);

        assert!(store.delete(id).unwrap());
        assert!(!store.delete(id).unwrap());
        assert!(store.get_including_expired(id).unwrap().is_none());
        assert_eq!(fts_rowid(&store, "betaquark"), None);
        assert_eq!(count(&store, "SELECT COUNT(*) FROM tags"), 0);
        assert_eq!(count(&store, "SELECT COUNT(*) FROM memory_tags"), 0);
        assert_eq!(count(&store, "SELECT COUNT(*) FROM memories_fts"), 0);
    }

    #[test]
    fn expired_memories_are_hidden_from_reads() {
        let (_dir, mut store) = open_temp();
        let expired = store
            .insert(&NewMemory {
                content: "old fact".to_owned(),
                expires_at: Some("2000-01-01T00:00:00Z".to_owned()),
                ..NewMemory::default()
            })
            .unwrap();
        let live = store
            .insert(&NewMemory {
                content: "current fact".to_owned(),
                expires_at: Some("2999-01-01T00:00:00Z".to_owned()),
                ..NewMemory::default()
            })
            .unwrap();
        assert!(expired.memory.expires_at.unwrap().ends_with('Z'));
        assert!(store.get(expired.memory.id).unwrap().is_none());
        assert!(store.get(live.memory.id).unwrap().is_some());
        assert_eq!(store.list(&ReadOptions::default()).unwrap().len(), 1);
        assert_eq!(
            store
                .list(&ReadOptions {
                    include_expired: true,
                    ..ReadOptions::default()
                })
                .unwrap()
                .len(),
            2
        );
        assert!(
            store
                .search("fact", &ScopeQuery::All, &ReadOptions::default())
                .unwrap()
                .len()
                == 1
        );
        assert!(store.delete(expired.memory.id).unwrap());
    }

    #[test]
    fn update_conflict_does_not_overwrite_the_other_memory() {
        let (_dir, mut store) = open_temp();
        let first = store.insert(&new_content("alpha")).unwrap();
        let second = store.insert(&new_content("beta")).unwrap();
        let err = store
            .update(
                second.memory.id,
                &MemoryUpdate {
                    content: Some("alpha".to_owned()),
                    ..MemoryUpdate::default()
                },
            )
            .unwrap_err();
        assert!(matches!(err, Error::Duplicate { .. }));
        assert_eq!(
            store.get(second.memory.id).unwrap().unwrap().content,
            "beta"
        );
        assert_eq!(
            store.get(first.memory.id).unwrap().unwrap().content,
            "alpha"
        );
    }

    #[test]
    fn list_returns_newest_first_and_respects_limit() {
        let (_dir, mut store) = open_temp();
        let older = store.insert(&new_content("older-row")).unwrap();
        let newer = store.insert(&new_content("newer-row")).unwrap();
        let rows = store.list(&ReadOptions::default()).unwrap();
        assert_eq!(rows[0].id, newer.memory.id);
        assert_eq!(rows[1].id, older.memory.id);

        let limited = store
            .list(&ReadOptions {
                limit: Some(1),
                ..ReadOptions::default()
            })
            .unwrap();
        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0].id, newer.memory.id);
    }

    #[test]
    fn timestamps_are_iso8601_utc() {
        let (_dir, mut store) = open_temp();
        let inserted = store.insert(&new_content("timed")).unwrap();
        let created = DateTime::parse_from_rfc3339(&inserted.memory.created_at).unwrap();
        assert_eq!(created.offset().local_minus_utc(), 0);
        assert!(inserted.memory.created_at.ends_with('Z'));
        assert_eq!(inserted.memory.created_at, inserted.memory.updated_at);
    }

    #[test]
    fn search_ranks_a_dense_match_first_and_rejects_bad_syntax() {
        let (_dir, mut store) = open_temp();
        let long = format!("rankingtoken {}", "paddingword ".repeat(40));
        store.insert(&new_content(&long)).unwrap();
        let short = store
            .insert(&new_content("rankingtoken rankingtoken rankingtoken"))
            .unwrap();
        let hits = store
            .search("rankingtoken", &ScopeQuery::All, &ReadOptions::default())
            .unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].memory.id, short.memory.id);
        assert!(hits[0].rank <= hits[1].rank);
        assert!(hits[0].snippet.contains("rankingtoken"));

        let err = store
            .search("(", &ScopeQuery::All, &ReadOptions::default())
            .unwrap_err();
        assert!(matches!(err, Error::InvalidSearch(_)), "{err}");
    }

    #[test]
    fn search_scope_modes_and_scope_counts() {
        let (_dir, mut store) = open_temp();
        store
            .insert(&NewMemory {
                content: "scopequark global copy".to_owned(),
                scope: "global".to_owned(),
                ..NewMemory::default()
            })
            .unwrap();
        store
            .insert(&NewMemory {
                content: "scopequark project copy".to_owned(),
                scope: "project".to_owned(),
                ..NewMemory::default()
            })
            .unwrap();
        store
            .insert(&NewMemory {
                content: "scopequark other copy".to_owned(),
                scope: "other".to_owned(),
                ..NewMemory::default()
            })
            .unwrap();
        store
            .insert(&NewMemory {
                content: "scopequark expired copy".to_owned(),
                scope: "archive".to_owned(),
                expires_at: Some("2000-01-01T00:00:00Z".to_owned()),
                ..NewMemory::default()
            })
            .unwrap();

        let with_global = store
            .search(
                "scopequark",
                &ScopeQuery::WithGlobal("project".to_owned()),
                &ReadOptions::default(),
            )
            .unwrap();
        let mut scopes: Vec<_> = with_global
            .iter()
            .map(|hit| hit.memory.scope.as_str())
            .collect();
        scopes.sort_unstable();
        assert_eq!(scopes, vec!["global", "project"]);

        let only = store
            .search(
                "scopequark",
                &ScopeQuery::Only("project".to_owned()),
                &ReadOptions::default(),
            )
            .unwrap();
        assert_eq!(only.len(), 1);
        assert_eq!(only[0].memory.scope, "project");

        let summaries = store.scopes().unwrap();
        assert_eq!(summaries[0].scope, "global");
        assert_eq!(summaries[0].count, 1);
        assert!(summaries[0].last_updated.is_some());
        assert_eq!(
            summaries
                .iter()
                .map(|row| (row.scope.as_str(), row.count))
                .collect::<Vec<_>>(),
            vec![("global", 1), ("other", 1), ("project", 1)]
        );
    }

    fn table_names(store: &Store) -> Vec<String> {
        let mut stmt = store
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type IN ('table', 'view') ORDER BY name")
            .unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap()
    }

    fn fts_rowid(store: &Store, term: &str) -> Option<i64> {
        store
            .conn
            .query_row(
                "SELECT rowid FROM memories_fts WHERE memories_fts MATCH ?1",
                [term],
                |row| row.get(0),
            )
            .optional()
            .unwrap()
    }

    fn count(store: &Store, sql: &str) -> i64 {
        store.conn.query_row(sql, [], |row| row.get(0)).unwrap()
    }
}
