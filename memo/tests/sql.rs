use std::path::Path;

use assert_cmd::Command;
use serde_json::Value;

fn memo(db: &Path) -> Command {
    let mut cmd = Command::cargo_bin("memo").unwrap();
    cmd.arg("--db").arg(db);
    cmd
}

fn db_path(dir: &tempfile::TempDir) -> std::path::PathBuf {
    dir.path().join("memo.db")
}

fn output(cmd: &mut Command) -> (i32, String, String) {
    let output = cmd.output().unwrap();
    (
        output.status.code().unwrap_or(101),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

fn json_ok(cmd: &mut Command) -> Value {
    let (code, out, err) = output(cmd);
    assert_eq!(code, 0, "{err}{out}");
    serde_json::from_str(out.trim()).unwrap()
}

#[test]
fn select_reads_the_memories_view_and_fts() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_path(&dir);
    let added = json_ok(memo(&db).arg("--json").args([
        "add",
        "hello quark",
        "--title",
        "Note",
        "--tags",
        "b, a",
    ]));
    let id = added["id"].clone();

    let rows = json_ok(memo(&db).args([
        "--json",
        "sql",
        "SELECT id, title, content, tags FROM memories",
    ]));
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"], id);
    assert_eq!(rows[0]["title"], "Note");
    assert_eq!(rows[0]["content"], "hello quark");
    assert_eq!(rows[0]["tags"], r#"["a","b"]"#);

    let found = json_ok(memo(&db).args([
        "--json",
        "sql",
        "SELECT memories.id AS id
         FROM memories
         JOIN memories_fts ON memories_fts.rowid = memories.id
         WHERE memories_fts MATCH 'quark'",
    ]));
    assert_eq!(found[0]["id"], id);

    let (code, out, err) = output(memo(&db).args(["sql", "SELECT id, tags FROM memories"]));
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("id"));
    assert!(out.contains("tags"));
    assert!(!err.contains("truncated"), "{err}");
}

#[test]
fn schema_lists_memories_and_fts() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_path(&dir);
    let objects = json_ok(memo(&db).args(["--json", "schema"]));
    let objects = objects.as_array().unwrap();
    assert_eq!(objects[0]["name"], "memories");
    assert_eq!(objects[0]["type"], "view");
    let columns: Vec<&str> = objects[0]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|column| column["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        columns,
        vec![
            "id",
            "title",
            "content",
            "kind",
            "scope",
            "tags",
            "source_agent",
            "created_at",
            "updated_at",
            "expires_at",
        ]
    );
    assert!(objects[0]["columns"][5]["description"]
        .as_str()
        .unwrap()
        .contains("JSON"));
    assert_eq!(objects[1]["name"], "memories_fts");
    assert_eq!(objects[1]["type"], "table");

    let (code, out, err) = output(memo(&db).arg("schema"));
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("memories (view)"));
    assert!(out.contains("memories_fts (table)"));
}

#[test]
fn writes_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_path(&dir);
    json_ok(memo(&db).args(["--json", "add", "keep this row"]));
    for statement in [
        "INSERT INTO memory_rows (content, kind, scope, created_at, updated_at, content_hash) VALUES ('x', 'note', 'global', '2020-01-01T00:00:00.000Z', '2020-01-01T00:00:00.000Z', 'hash')",
        "UPDATE memory_rows SET title = 'changed'",
        "DELETE FROM memory_rows",
        "DROP TABLE memory_rows",
        "ATTACH DATABASE ':memory:' AS extra",
    ] {
        let (code, out, err) = output(memo(&db).args(["sql", statement]));
        assert_eq!(code, 1, "{statement}\n{out}{err}");
        assert!(err.contains("SQL access is read-only"), "{statement}\n{err}");
        assert!(!err.to_ascii_lowercase().contains("panic"), "{err}");
    }
    let rows = json_ok(memo(&db).args(["--json", "sql", "SELECT content FROM memories"]));
    assert_eq!(rows[0]["content"], "keep this row");
}

#[test]
fn row_cap_reports_truncation() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_path(&dir);
    let query = "WITH RECURSIVE c(n) AS (
        SELECT 1 UNION ALL SELECT n + 1 FROM c WHERE n < 30
    ) SELECT n FROM c";
    let rows = json_ok(
        memo(&db)
            .arg("--json")
            .args(["sql", query, "--max-rows", "10"]),
    );
    assert_eq!(rows.as_array().unwrap().len(), 10);
    assert_eq!(rows[0]["n"], 1);
    assert_eq!(rows[9]["n"], 10);

    let (code, out, err) = output(memo(&db).args(["sql", query, "--max-rows", "10"]));
    assert_eq!(code, 0, "{err}{out}");
    assert!(err.contains("results truncated to 10 rows"), "{err}");

    let wide = "WITH RECURSIVE c(n) AS (
        SELECT 1 UNION ALL SELECT n + 1 FROM c WHERE n < 1001
    ) SELECT n FROM c";
    let (code, out, err) = output(memo(&db).args(["--json", "sql", wide]));
    assert_eq!(code, 0, "{err}");
    let rows: Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 1000);
    assert!(err.contains("results truncated to 1000 rows"), "{err}");
}

#[test]
fn an_expensive_query_hits_the_time_limit() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_path(&dir);
    let query = "WITH RECURSIVE c(n) AS (
        SELECT 1 UNION ALL SELECT n + 1 FROM c WHERE n < 100000000
    ) SELECT sum(n) FROM c";
    let started = std::time::Instant::now();
    let (code, out, err) = output(memo(&db).args(["sql", query]));
    let elapsed = started.elapsed();
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("time limit"), "{err}");
    assert!(
        elapsed.as_secs() < 30,
        "query was not interrupted in time: {elapsed:?}"
    );
    assert!(!err.to_ascii_lowercase().contains("panic"), "{err}");
}
