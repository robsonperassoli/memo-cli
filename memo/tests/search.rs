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

fn add_id(db: &Path, args: &[&str]) -> i64 {
    json_ok(memo(db).arg("--json").arg("add").args(args))["id"]
        .as_i64()
        .unwrap()
}

fn search(db: &Path, args: &[&str]) -> Vec<Value> {
    let value = json_ok(memo(db).arg("--json").arg("search").args(args));
    let rows = value.as_array().expect("search json is an array").clone();
    for row in &rows {
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
            "rank",
            "snippet",
        ] {
            assert!(row.get(key).is_some(), "missing {key} in {row}");
        }
        assert!(row["rank"].is_number());
        assert!(row["snippet"].is_string());
    }
    rows
}

fn ids(rows: &[Value]) -> Vec<i64> {
    let mut ids: Vec<i64> = rows.iter().map(|row| row["id"].as_i64().unwrap()).collect();
    ids.sort_unstable();
    ids
}

#[test]
fn plain_terms_are_anded_and_any_is_or() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_path(&dir);
    let both = add_id(&db, &["alphaquark betaquark"]);
    let alpha = add_id(&db, &["alphaquark only"]);
    assert_eq!(ids(&search(&db, &["alphaquark", "betaquark"])), vec![both]);
    let mut either = vec![alpha, both];
    either.sort_unstable();
    assert_eq!(
        ids(&search(&db, &["alphaquark", "betaquark", "--any"])),
        either
    );
}

#[test]
fn prefix_matches_and_the_bare_token_does_not() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_path(&dir);
    let deployment = add_id(&db, &["deployment plan for the service"]);
    add_id(&db, &["unrelated note"]);
    assert_eq!(ids(&search(&db, &["deploy*"])), vec![deployment]);
    assert!(search(&db, &["deploy"]).is_empty());
}

#[test]
fn raw_fts_supports_phrases_or_and_near() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_path(&dir);
    let pie = add_id(&db, &["red apple pie"]);
    let truck = add_id(&db, &["red truck"]);
    let close = add_id(&db, &["red tasty apple"]);
    let far = add_id(&db, &["red one two three apple"]);

    assert_eq!(ids(&search(&db, &["\"red apple\""])), vec![pie]);
    let mut either = vec![pie, truck, close, far];
    either.sort_unstable();
    assert_eq!(ids(&search(&db, &["apple OR truck"])), either);
    let mut near = vec![pie, close];
    near.sort_unstable();
    assert_eq!(ids(&search(&db, &["NEAR(red apple, 2)"])), near);

    let (code, _, err) = output(memo(&db).args(["search", "red NEAR/2 apple"]));
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("invalid search query"), "{err}");
}

#[test]
fn markdown_symbols_do_not_hide_indexed_words() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_path(&dir);
    let id = add_id(
        &db,
        &["`codequark` [labelquark](https://example.com/urlquark) #headingquark"],
    );
    assert_eq!(ids(&search(&db, &["codequark"])), vec![id]);
    assert_eq!(ids(&search(&db, &["labelquark"])), vec![id]);
    assert_eq!(ids(&search(&db, &["#headingquark"])), vec![id]);
    assert!(search(&db, &["urlquark"]).is_empty());
    assert!(search(&db, &["example.com"]).is_empty());
}

#[test]
fn scope_flags_include_or_exclude_global() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_path(&dir);
    let global = add_id(&db, &["--scope", "global", "scopequark global copy"]);
    let project = add_id(&db, &["--scope", "project", "scopequark project copy"]);
    let other = add_id(&db, &["--scope", "other", "scopequark other copy"]);

    assert_eq!(
        ids(&search(&db, &["scopequark", "--scope", "project"])),
        vec![global, project]
    );
    assert_eq!(
        ids(&search(&db, &["scopequark", "--only-scope", "project"])),
        vec![project]
    );
    assert_eq!(
        ids(&search(&db, &["scopequark"])),
        vec![global, project, other]
    );

    let (code, _, err) = output(memo(&db).arg("search").args([
        "scopequark",
        "--scope",
        "project",
        "--only-scope",
        "project",
    ]));
    assert_eq!(code, 2, "{err}");
}

#[test]
fn regex_filters_title_and_content_and_can_limit() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_path(&dir);
    let title = add_id(&db, &["--title", "Ticket E-42", "plain body"]);
    let body = add_id(&db, &["failure E-99 in the log"]);
    add_id(&db, &["failure E-nope in the log"]);

    assert_eq!(ids(&search(&db, &["--regex", r"E-\d+"])), vec![title, body]);
    let combined = search(&db, &["failure", "--regex", r"E-\d+"]);
    assert_eq!(ids(&combined), vec![body]);
    assert!(combined[0]["snippet"].as_str().unwrap().contains("failure"));

    let limited = search(&db, &["--regex", r"E-\d+", "--limit", "1"]);
    assert_eq!(limited.len(), 1);
    assert_eq!(limited[0]["rank"], 0.0);

    let (code, _, err) = output(memo(&db).args(["search", "--regex", "["]));
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("invalid regular expression"), "{err}");
    assert!(!err.to_ascii_lowercase().contains("panic"));
}

#[test]
fn results_are_ranked_and_expired_rows_are_omitted() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_path(&dir);
    let long = format!("rankingtoken {}", "paddingword ".repeat(40));
    add_id(&db, &[&long]);
    let short = add_id(&db, &["rankingtoken rankingtoken rankingtoken"]);
    add_id(&db, &["--expires", "2000-01-01", "rankingtoken expired"]);

    let hits = search(&db, &["rankingtoken"]);
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0]["id"], short);
    let rank0 = hits[0]["rank"].as_f64().unwrap();
    let rank1 = hits[1]["rank"].as_f64().unwrap();
    assert!(rank0 <= rank1, "{rank0} vs {rank1}");
    assert!(hits[0]["snippet"]
        .as_str()
        .unwrap()
        .contains("rankingtoken"));

    let top = search(&db, &["rankingtoken", "--limit", "1"]);
    assert_eq!(top.len(), 1);
    assert_eq!(top[0]["id"], short);

    let (code, out, err) = output(memo(&db).args(["search", "rankingtoken"]));
    assert_eq!(code, 0, "{err}");
    assert!(out.contains(&short.to_string()));
}

#[test]
fn scopes_list_counts_and_always_include_global() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_path(&dir);

    let empty = json_ok(memo(&db).args(["--json", "scopes"]));
    assert_eq!(
        empty,
        serde_json::json!([{ "scope": "global", "count": 0, "last_updated": null }])
    );
    let (code, out, err) = output(memo(&db).arg("scopes"));
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("global  0  -"), "{out}");

    add_id(&db, &["first global memory"]);
    add_id(&db, &["second global memory"]);
    add_id(&db, &["--scope", "project", "project memory"]);
    add_id(
        &db,
        &[
            "--scope",
            "archive",
            "--expires",
            "2000-01-01",
            "archived memory",
        ],
    );

    let rows = json_ok(memo(&db).args(["--json", "scopes"]))
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(rows[0]["scope"], "global");
    assert_eq!(rows[0]["count"], 2);
    assert!(rows[0]["last_updated"].as_str().unwrap().ends_with('Z'));
    assert_eq!(
        rows.iter()
            .map(|row| (
                row["scope"].as_str().unwrap(),
                row["count"].as_i64().unwrap()
            ))
            .collect::<Vec<_>>(),
        vec![("global", 2), ("project", 1)]
    );
}

#[test]
fn kind_and_tag_filters_apply_and_bad_fts_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_path(&dir);
    let note = add_id(
        &db,
        &["--kind", "note", "--tags", "rust", "filtertoken note"],
    );
    add_id(
        &db,
        &["--kind", "fact", "--tags", "rust", "filtertoken fact"],
    );
    add_id(
        &db,
        &["--kind", "note", "--tags", "other", "filtertoken other"],
    );
    assert_eq!(
        ids(&search(
            &db,
            &["filtertoken", "--kind", "note", "--tag", "rust"]
        )),
        vec![note]
    );

    let (code, _, err) = output(memo(&db).args(["search", "("]));
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("invalid search query"), "{err}");
    assert!(!err.to_ascii_lowercase().contains("panic"));

    let (code, _, err) = output(memo(&db).arg("search"));
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("provide search terms or --regex"), "{err}");
}
