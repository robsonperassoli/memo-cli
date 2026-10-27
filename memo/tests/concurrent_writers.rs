//! Two OS processes write one database at the same time.
//!
//! The test harness re-executes this binary. A child sees `MEMO_STORAGE_CHILD`
//! and only inserts; the parent waits for both children.

use std::process::Command;

use memo::{NewMemory, ReadOptions, Store};

#[test]
fn concurrent_writers_do_not_error() {
    if let Ok(id) = std::env::var("MEMO_STORAGE_CHILD") {
        let path = std::env::var("MEMO_DB").expect("MEMO_DB");
        let mut store = Store::open(path).expect("open");
        for n in 0..10 {
            store
                .insert(&NewMemory {
                    content: format!("writer {id} row {n}"),
                    source_agent: Some(format!("agent-{id}")),
                    ..NewMemory::default()
                })
                .expect("insert");
        }
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("memo.db");
    let exe = std::env::current_exe().unwrap();
    let mut children = Vec::new();
    for id in ["a", "b"] {
        children.push(
            Command::new(&exe)
                .arg("--exact")
                .arg("concurrent_writers_do_not_error")
                .arg("--test-threads=1")
                .env("MEMO_STORAGE_CHILD", id)
                .env("MEMO_DB", &db)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap(),
        );
    }

    for mut child in children {
        let status = child.wait().unwrap();
        let mut output = String::new();
        if let Some(mut stdout) = child.stdout.take() {
            use std::io::Read;
            stdout.read_to_string(&mut output).unwrap();
        }
        if let Some(mut stderr) = child.stderr.take() {
            use std::io::Read;
            stderr.read_to_string(&mut output).unwrap();
        }
        assert!(status.success(), "writer process failed: {output}");
    }

    let store = Store::open(&db).unwrap();
    let rows = store.list(&ReadOptions::default()).unwrap();
    assert_eq!(rows.len(), 20);
}
