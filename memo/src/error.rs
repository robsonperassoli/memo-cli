use std::fmt;

/// Failures from path resolution or the SQLite store.
#[derive(Debug)]
pub enum Error {
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
    /// `HOME` is unset and neither `--db`, `MEMO_DB`, nor `XDG_DATA_HOME` was provided.
    HomeMissing,
    EmptyPath,
    EmptyScope,
    InvalidKind(String),
    InvalidTimestamp(String),
    /// Another memory in `scope` already has this normalized content.
    Duplicate {
        scope: String,
    },
    /// The on-disk schema is newer than this build.
    NewerSchema {
        found: i64,
        supported: i64,
    },
    /// SQLite refused to enable WAL.
    WalUnavailable(String),
    /// The FTS5 query or a search term is not usable.
    InvalidSearch(String),
    EmptySql,
    /// `memo sql` refused a statement that would change a database.
    ReadOnlySql,
    /// A read-only query ran longer than the progress-handler limit.
    SqlTimeout,
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(err) => write!(f, "database error: {err}"),
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::HomeMissing => write!(
                f,
                "could not locate the database directory: set HOME, XDG_DATA_HOME, or MEMO_DB"
            ),
            Self::EmptyPath => write!(f, "database path must not be empty"),
            Self::EmptyScope => write!(f, "scope must not be empty"),
            Self::InvalidKind(kind) => write!(f, "invalid memory kind: {kind}"),
            Self::InvalidTimestamp(value) => write!(f, "invalid timestamp: {value}"),
            Self::Duplicate { scope } => write!(
                f,
                "a memory with the same content already exists in scope {scope}"
            ),
            Self::NewerSchema { found, supported } => write!(
                f,
                "database schema version {found} is newer than supported version {supported}"
            ),
            Self::WalUnavailable(mode) => {
                write!(f, "could not enable WAL journal mode (got {mode})")
            }
            Self::InvalidSearch(message) => write!(f, "invalid search query: {message}"),
            Self::EmptySql => write!(f, "SQL query must not be empty"),
            Self::ReadOnlySql => write!(f, "SQL access is read-only"),
            Self::SqlTimeout => write!(f, "query exceeded the 5s time limit"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(err) => Some(err),
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sqlite(value)
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
