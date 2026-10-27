//! Shared agent memory stored in SQLite.
//!
//! The `memo` binary parses flags. Persistence, migrations, and search live in
//! [`storage`].

mod error;
pub mod storage;

pub use error::{Error, Result};
pub use storage::{
    resolve_db_path, InsertResult, Memory, MemoryKind, MemoryUpdate, NewMemory, Patch, ReadOptions,
    Store, SCHEMA_VERSION,
};
