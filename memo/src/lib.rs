//! Shared agent memory stored in SQLite.
//!
//! The `memo` binary parses flags. Persistence, migrations, and search live in
//! [`storage`].

mod error;
pub mod storage;

pub use error::{Error, Result};
pub use storage::{
    compile_fts_query, resolve_db_path, InsertResult, Memory, MemoryKind, MemoryUpdate, NewMemory,
    Patch, ReadOptions, ScopeQuery, ScopeSummary, SearchHit, Store, SCHEMA_VERSION,
};
