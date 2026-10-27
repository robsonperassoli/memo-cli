use std::fs;
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

fn stdout(cmd: &mut Command) -> (bool, String, String) {
    let output = cmd.output().unwrap();
    (
        output.status.success(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

fn json_ok(cmd: &mut Command) -> Value {
    let (ok, out, err) = stdout(cmd);
    assert!(ok, "{err}");
    serde_json::from_str(out.trim()).unwrap()
}

fn assert_shape(value: &Value) {
    for key in [
        "id",
        "scope",
        "kind",
        "title",
        "content",
        "tags",
        "source_agent",
        "created_at",
        "updated_at",
        "expires_at",
    ] {
        assert!(value.get(key).is_some(), "missing {key} in {value}");
    }
    assert!(value["tags"].is_array());
    assert!(value["created_at"].as_str().unwrap().ends_with('Z'));
    assert!(value["updated_at"].as_str().unwrap().ends_with('Z'));
}

#[test]
fn add_writes_markdown_and_json() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_path(&dir);
    let value = json_ok(memo(&db).args([
        "--json",
        "add",
        "# Hello\n`code`",
        "--title",
        "Title",
        "--kind",
        "fact",
        "--tags",
        "b, a",
        "--agent",
        "ada",
        "--expires",
        "2030-01-02",
    ]));
    assert_shape(&value);
    assert_eq!(value["deduplicated"], false);
    assert_eq!(value["scope"], "global");
    assert_eq!(value["kind"], "fact");
    assert_eq!(value["title"], "Title");
    assert_eq!(value["content"], "# Hello\n`code`");
    assert_eq!(value["tags"], serde_json::json!(["a", "b"]));
    assert_eq!(value["source_agent"], "ada");
    assert_eq!(value["expires_at"], "2030-01-02T00:00:00.000Z");

    let (ok, out, err) = stdout(memo(&db).args(["add", "plain note"]));
    assert!(ok, "{err}");
    assert!(out.starts_with("added "));
}

#[test]
fn add_reads_stdin_and_file() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_path(&dir);
    let from_dash = json_ok(
        memo(&db)
            .args(["--json", "add", "-"])
            .write_stdin("# piped\n"),
    );
    assert_eq!(from_dash["content"], "# piped\n");
    assert_eq!(from_dash["deduplicated"], false);

    let from_pipe = json_ok(
        memo(&db)
            .args(["--json", "add"])
            .write_stdin("implicit pipe"),
    );
    assert_eq!(from_pipe["content"], "implicit pipe");

    let path = dir.path().join("note.md");
    fs::write(&path, "from file\n").unwrap();
    let from_file = json_ok(
        memo(&db)
            .args(["--json", "add", "--file"])
            .arg(&path)
            .arg("--scope")
            .arg("docs"),
    );
    assert_eq!(from_file["content"], "from file\n");
    assert_eq!(from_file["scope"], "docs");

    memo(&db)
        .args(["add", "hello", "--file"])
        .arg(&path)
        .assert()
        .failure();
}

#[test]
fn add_deduplicates_in_the_same_scope() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_path(&dir);
    let first = json_ok(memo(&db).args(["--json", "add", "Hello   world"]));
    let second = json_ok(memo(&db).args(["--json", "add", "Hello world"]));
    assert_eq!(first["id"], second["id"]);
    assert_eq!(second["deduplicated"], true);
    assert_eq!(second["content"], "Hello   world");

    let other = json_ok(memo(&db).args(["--json", "add", "Hello world", "--scope", "project"]));
    assert_ne!(other["id"], first["id"]);
    assert_eq!(other["deduplicated"], false);

    let (ok, out, err) = stdout(memo(&db).args(["add", "Hello world"]));
    assert!(ok, "{err}");
    assert_eq!(out.trim(), format!("deduplicated {}", first["id"]));
}

#[test]
fn get_prints_memory_and_errors_when_missing() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_path(&dir);
    let added = json_ok(memo(&db).args([
        "--json",
        "add",
        "body text",
        "--title",
        "Shown",
        "--kind",
        "note",
    ]));
    let id = added["id"].as_i64().unwrap().to_string();

    let fetched = json_ok(memo(&db).args(["--json", "get", &id]));
    assert_shape(&fetched);
    assert_eq!(fetched["content"], "body text");
    assert_eq!(fetched["title"], "Shown");
    assert!(fetched.get("deduplicated").is_none());

    let (ok, out, err) = stdout(memo(&db).args(["get", &id]));
    assert!(ok, "{err}");
    assert!(out.contains("title: Shown"));
    assert!(out.contains("body text"));
    assert!(out.contains("kind: note"));

    let missing = memo(&db).args(["get", "999"]).assert().failure();
    let stderr = String::from_utf8_lossy(&missing.get_output().stderr);
    assert!(stderr.contains("memory 999 not found"));

    let expired =
        json_ok(memo(&db).args(["--json", "add", "already gone", "--expires", "2000-01-01"]));
    let expired_id = expired["id"].as_i64().unwrap().to_string();
    memo(&db)
        .args(["get", &expired_id])
        .assert()
        .failure()
        .code(1);
}

#[test]
fn update_changes_fields_from_args_stdin_and_file() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_path(&dir);
    let added = json_ok(memo(&db).args([
        "--json",
        "add",
        "alpha content",
        "--title",
        "Old",
        "--tags",
        "a,b",
    ]));
    let id = added["id"].as_i64().unwrap().to_string();
    std::thread::sleep(std::time::Duration::from_millis(20));

    let updated = json_ok(memo(&db).args([
        "--json",
        "update",
        &id,
        "--title",
        "New",
        "--kind",
        "decision",
        "--scope",
        "proj",
        "--add-tag",
        "c",
        "--remove-tag",
        "a",
        "--expires",
        "2031-05-06T01:02:03Z",
        "--content",
        "beta content",
    ]));
    assert_shape(&updated);
    assert_eq!(updated["content"], "beta content");
    assert_eq!(updated["title"], "New");
    assert_eq!(updated["kind"], "decision");
    assert_eq!(updated["scope"], "proj");
    assert_eq!(updated["tags"], serde_json::json!(["b", "c"]));
    assert_eq!(updated["expires_at"], "2031-05-06T01:02:03.000Z");
    assert_ne!(updated["updated_at"], added["updated_at"]);
    assert_eq!(updated["created_at"], added["created_at"]);

    let from_stdin = json_ok(
        memo(&db)
            .args(["--json", "update", &id, "--content", "-"])
            .write_stdin("from stdin"),
    );
    assert_eq!(from_stdin["content"], "from stdin");

    let path = dir.path().join("next.md");
    fs::write(&path, "from file\n").unwrap();
    let from_file = json_ok(
        memo(&db)
            .args(["--json", "update", &id, "--file"])
            .arg(&path),
    );
    assert_eq!(from_file["content"], "from file\n");

    let (ok, out, err) = stdout(memo(&db).args(["update", &id, "--title", "Again"]));
    assert!(ok, "{err}");
    assert_eq!(out.trim(), format!("updated {id}"));

    memo(&db)
        .args(["update", "404", "--title", "nope"])
        .assert()
        .failure()
        .code(1);
    memo(&db).args(["update", &id]).assert().failure().code(1);
}

#[test]
fn forget_deletes_memory() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_path(&dir);
    let added = json_ok(memo(&db).args(["--json", "add", "to forget"]));
    let id = added["id"].as_i64().unwrap().to_string();

    let forgotten = json_ok(memo(&db).args(["--json", "forget", &id]));
    assert_shape(&forgotten);
    assert_eq!(forgotten["id"], added["id"]);
    assert_eq!(forgotten["content"], "to forget");

    memo(&db).args(["get", &id]).assert().failure().code(1);

    let added_again = json_ok(memo(&db).args(["--json", "add", "second forget"]));
    let id_again = added_again["id"].as_i64().unwrap().to_string();
    let (ok, out, err) = stdout(memo(&db).args(["forget", &id_again]));
    assert!(ok, "{err}");
    assert_eq!(out.trim(), format!("forgot {id_again}"));

    let missing = memo(&db).args(["forget", &id_again]).assert().failure();
    let stderr = String::from_utf8_lossy(&missing.get_output().stderr);
    assert!(stderr.contains("not found"));
}

#[test]
fn list_is_newest_first_and_filters() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_path(&dir);
    let older = json_ok(memo(&db).args([
        "--json",
        "add",
        "secret-body-should-not-show",
        "--title",
        "Older title",
        "--kind",
        "note",
        "--scope",
        "global",
        "--tags",
        "keep",
    ]));
    let long = format!("{} tail", "word ".repeat(40));
    let newer = json_ok(memo(&db).args([
        "--json", "add", &long, "--kind", "fact", "--scope", "lab", "--tags", "new",
    ]));

    let (ok, out, err) = stdout(memo(&db).arg("list"));
    assert!(ok, "{err}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains(&newer["id"].to_string()));
    assert!(lines[0].contains("fact"));
    assert!(lines[0].contains("lab"));
    assert!(lines[0].contains("..."));
    assert!(!lines[0].contains("tail"));
    assert!(lines[1].contains("Older title"));
    assert!(!lines[1].contains("secret-body-should-not-show"));

    let limited = json_ok(memo(&db).args(["--json", "list", "--limit", "1"]));
    let rows = limited.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_shape(&rows[0]);
    assert_eq!(rows[0]["id"], newer["id"]);
    assert_eq!(rows[0]["content"], long);

    let tagged = json_ok(memo(&db).args(["--json", "list", "--tag", "keep"]));
    assert_eq!(tagged.as_array().unwrap().len(), 1);
    assert_eq!(tagged[0]["id"], older["id"]);

    let scoped = json_ok(memo(&db).args(["--json", "list", "--scope", "lab", "--kind", "fact"]));
    assert_eq!(scoped.as_array().unwrap().len(), 1);
    assert_eq!(scoped[0]["id"], newer["id"]);
}
