//! Shared agent memory stored in SQLite.
//!
//! The `memo` binary parses flags. Persistence, migrations, and search live in
//! [`storage`].

mod error;
pub mod storage;

pub use error::{Error, Result};
pub use storage::{
    compile_fts_query, query_readonly, resolve_db_path, InsertResult, Memory, MemoryKind,
    MemoryUpdate, NewMemory, Patch, ReadOptions, SchemaColumn, SchemaObject, ScopeQuery,
    ScopeSummary, SearchHit, SqlCell, SqlTable, Store, SCHEMA_VERSION, SQL_ROW_CAP,
};
