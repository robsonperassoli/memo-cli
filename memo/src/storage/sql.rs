//! Read-only SQL against the stable `memories` view and `memories_fts`.

use std::path::Path;
use std::time::{Duration, Instant};

use rusqlite::{params, Connection, OpenFlags};

use super::Store;
use crate::error::{Error, Result};

/// Default cap for [`query_readonly`].
pub const SQL_ROW_CAP: u32 = 1000;

const SQL_TIMEOUT: Duration = Duration::from_secs(5);

/// One documented column of a queryable object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaColumn {
    pub name: String,
    pub type_name: String,
    pub description: String,
}

/// A view or table that `memo sql` is meant to query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaObject {
    pub name: String,
    pub kind: String,
    pub description: String,
    pub columns: Vec<SchemaColumn>,
}

/// A cell returned by [`query_readonly`].
#[derive(Debug, Clone, PartialEq)]
pub enum SqlCell {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

/// Rows from one read-only statement.
#[derive(Debug, Clone, PartialEq)]
pub struct SqlTable {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<SqlCell>>,
    /// True when another row existed past `max_rows`.
    pub truncated: bool,
}

struct DocumentedObject {
    name: &'static str,
    description: &'static str,
}

const DOCUMENTED: &[DocumentedObject] = &[
    DocumentedObject {
        name: "memories",
        description: "Stable memory rows. tags is a JSON array of tag names.",
    },
    DocumentedObject {
        name: "memories_fts",
        description: "Full-text index. MATCH this table and join rowid to memories.id.",
    },
];

pub(crate) fn read_schema(conn: &Connection) -> Result<Vec<SchemaObject>> {
    let mut objects = Vec::with_capacity(DOCUMENTED.len());
    for documented in DOCUMENTED {
        let kind: String = conn.query_row(
            "SELECT type FROM sqlite_master WHERE name = ?1 AND type IN ('table', 'view')",
            [documented.name],
            |row| row.get(0),
        )?;
        let mut stmt = conn.prepare("SELECT name, type FROM pragma_table_info(?1) ORDER BY cid")?;
        let columns = stmt
            .query_map(params![documented.name], |row| {
                let name: String = row.get(0)?;
                let type_name: String = row.get(1)?;
                Ok(SchemaColumn {
                    description: column_description(documented.name, &name).to_owned(),
                    name,
                    type_name,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);
        objects.push(SchemaObject {
            name: documented.name.to_owned(),
            kind,
            description: documented.description.to_owned(),
            columns,
        });
    }
    Ok(objects)
}

fn column_description(object: &str, column: &str) -> &'static str {
    match (object, column) {
        ("memories", "id") => "Integer primary key.",
        ("memories", "title") => "Optional title.",
        ("memories", "content") => "Markdown body, stored as written.",
        ("memories", "kind") => "fact, decision, or note.",
        ("memories", "scope") => "Scope name. The default scope is global.",
        ("memories", "tags") => "JSON array of tag names, stored as text.",
        ("memories", "source_agent") => "Recorded agent name, or null.",
        ("memories", "created_at") => "Creation time, ISO-8601 UTC.",
        ("memories", "updated_at") => "Last update time, ISO-8601 UTC.",
        ("memories", "expires_at") => "Expiry time, ISO-8601 UTC, or null.",
        ("memories_fts", "title") => "Indexed title.",
        ("memories_fts", "content") => "Indexed content. Markdown symbols are removed.",
        ("memories_fts", "tags") => "Indexed tag names.",
        _ => "Undocumented column.",
    }
}

/// Run one statement on a read-only connection.
///
/// The database is migrated first. The query connection is opened with
/// `SQLITE_OPEN_READ_ONLY` and `PRAGMA query_only`. Writes, including
/// `ATTACH`, are denied. At most `max_rows` rows are returned. The progress
/// handler cancels the query after 5 seconds.
pub fn query_readonly(path: &Path, sql: &str, max_rows: u32) -> Result<SqlTable> {
    if sql.trim().is_empty() {
        return Err(Error::EmptySql);
    }
    {
        let _migrated = Store::open(path)?;
    }
    let mut conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.busy_timeout(SQL_TIMEOUT)?;
    conn.pragma_update(None, "query_only", true)?;
    let started = Instant::now();
    conn.progress_handler(1_000, Some(move || started.elapsed() >= SQL_TIMEOUT))
        .map_err(Error::from)?;
    conn.authorizer(Some(readonly_authorizer))
        .map_err(Error::from)?;

    let mut stmt = conn.prepare(sql).map_err(map_sql_error)?;
    let columns: Vec<String> = stmt.column_names().map(str::to_owned).collect();
    let mut query = stmt.query([]).map_err(map_sql_error)?;
    let limit = usize::try_from(max_rows).unwrap_or(usize::MAX);
    let mut rows = Vec::new();
    let mut truncated = false;
    loop {
        let next = query.next().map_err(map_sql_error)?;
        let Some(row) = next else {
            break;
        };
        if rows.len() >= limit {
            truncated = true;
            break;
        }
        let mut cells = Vec::with_capacity(columns.len());
        for index in 0..columns.len() {
            cells.push(read_cell(row, index)?);
        }
        rows.push(cells);
    }
    Ok(SqlTable {
        columns,
        rows,
        truncated,
    })
}

fn read_cell(row: &rusqlite::Row<'_>, index: usize) -> Result<SqlCell> {
    let value = row.get_ref(index).map_err(map_sql_error)?;
    Ok(match value {
        rusqlite::types::ValueRef::Null => SqlCell::Null,
        rusqlite::types::ValueRef::Integer(value) => SqlCell::Integer(value),
        rusqlite::types::ValueRef::Real(value) => SqlCell::Real(value),
        rusqlite::types::ValueRef::Text(value) => {
            SqlCell::Text(String::from_utf8_lossy(value).into_owned())
        }
        rusqlite::types::ValueRef::Blob(value) => SqlCell::Blob(value.to_vec()),
    })
}

fn readonly_authorizer(ctx: rusqlite::hooks::AuthContext<'_>) -> rusqlite::hooks::Authorization {
    use rusqlite::hooks::{AuthAction, Authorization};
    match ctx.action {
        AuthAction::Select
        | AuthAction::Read { .. }
        | AuthAction::Function { .. }
        | AuthAction::Recursive => Authorization::Allow,
        AuthAction::Pragma {
            pragma_value: None, ..
        } => Authorization::Allow,
        _ => Authorization::Deny,
    }
}

fn map_sql_error(err: rusqlite::Error) -> Error {
    if is_timeout(&err) {
        Error::SqlTimeout
    } else if is_read_only(&err) {
        Error::ReadOnlySql
    } else {
        Error::Sqlite(err)
    }
}

fn is_timeout(err: &rusqlite::Error) -> bool {
    err.sqlite_error_code() == Some(rusqlite::ErrorCode::OperationInterrupted)
        || err.to_string().to_ascii_lowercase().contains("interrupted")
}

fn is_read_only(err: &rusqlite::Error) -> bool {
    matches!(
        err.sqlite_error_code(),
        Some(rusqlite::ErrorCode::ReadOnly | rusqlite::ErrorCode::AuthorizationForStatementDenied)
    ) || {
        let message = err.to_string().to_ascii_lowercase();
        message.contains("not authorized")
            || message.contains("readonly")
            || message.contains("read-only")
    }
}
