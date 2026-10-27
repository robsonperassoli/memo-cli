use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDate};
use clap::{Parser, Subcommand, ValueEnum};
use memo::{
    compile_fts_query, query_readonly, Memory, MemoryKind, MemoryUpdate, NewMemory, Patch,
    ReadOptions, SchemaObject, ScopeQuery, SearchHit, SqlCell, SqlTable, Store, SQL_ROW_CAP,
};
use serde_json::{json, Value};

/// Shared agent memory stored in SQLite.
#[derive(Parser)]
#[command(name = "memo", version, about, propagate_version = true)]
struct Cli {
    /// SQLite database path. Overrides `MEMO_DB`.
    ///
    /// When omitted, memo uses `$XDG_DATA_HOME/memo/memo.db`, or
    /// `~/.local/share/memo/memo.db` if `XDG_DATA_HOME` is unset.
    #[arg(long, global = true, value_name = "PATH")]
    db: Option<PathBuf>,

    /// Print JSON instead of text.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Store a memory.
    Add {
        /// Memory body. Use `-` to read stdin.
        ///
        /// Markdown is stored unchanged. When this argument and `--file` are
        /// both omitted, a pipe is read as the body.
        #[arg(value_name = "CONTENT", conflicts_with = "file")]
        content: Option<String>,

        /// Read the memory body from a file.
        #[arg(long, value_name = "PATH")]
        file: Option<PathBuf>,

        /// Optional title.
        #[arg(long, value_name = "TEXT")]
        title: Option<String>,

        /// Memory kind. Defaults to `note`.
        #[arg(long, value_enum, value_name = "KIND")]
        kind: Option<KindArg>,

        /// Scope. Defaults to `global`.
        #[arg(long, value_name = "SCOPE")]
        scope: Option<String>,

        /// Comma-separated tags.
        #[arg(long, value_name = "TAGS")]
        tags: Option<String>,

        /// Name recorded as `source_agent`.
        #[arg(long, value_name = "NAME")]
        agent: Option<String>,

        /// Expiry as `YYYY-MM-DD` or an RFC 3339 timestamp.
        #[arg(long, value_name = "DATE")]
        expires: Option<String>,
    },

    /// Print one memory.
    Get {
        /// Memory id.
        id: i64,
    },

    /// Change a memory. `updated_at` is refreshed and the full-text index is updated.
    Update {
        /// Memory id.
        id: i64,

        /// Replacement body. Use `-` to read stdin.
        #[arg(long, value_name = "CONTENT", conflicts_with = "file")]
        content: Option<String>,

        /// Read the replacement body from a file.
        #[arg(long, value_name = "PATH")]
        file: Option<PathBuf>,

        /// Replacement title. An empty value clears it.
        #[arg(long, value_name = "TEXT")]
        title: Option<String>,

        /// Replacement kind.
        #[arg(long, value_enum, value_name = "KIND")]
        kind: Option<KindArg>,

        /// Replacement scope.
        #[arg(long, value_name = "SCOPE")]
        scope: Option<String>,

        /// Tag to add. May be repeated.
        #[arg(long = "add-tag", value_name = "TAG")]
        add_tag: Vec<String>,

        /// Tag to remove. May be repeated.
        #[arg(long = "remove-tag", value_name = "TAG")]
        remove_tag: Vec<String>,

        /// Replacement expiry (`YYYY-MM-DD` or RFC 3339).
        #[arg(long, value_name = "DATE")]
        expires: Option<String>,
    },

    /// Delete a memory.
    Forget {
        /// Memory id.
        id: i64,
    },

    /// List memories, newest first.
    List {
        /// Only memories with this tag.
        #[arg(long, value_name = "TAG")]
        tag: Option<String>,

        /// Only memories in this scope.
        #[arg(long, value_name = "SCOPE")]
        scope: Option<String>,

        /// Only memories of this kind.
        #[arg(long, value_enum, value_name = "KIND")]
        kind: Option<KindArg>,

        /// Maximum number of rows.
        #[arg(long, value_name = "N")]
        limit: Option<u32>,
    },

    /// Search title, content, and tags.
    Search {
        /// Keywords or a raw FTS5 query. A trailing `*` is a prefix.
        #[arg(value_name = "TERM")]
        terms: Vec<String>,

        /// This scope plus `global`.
        #[arg(long, value_name = "SCOPE", conflicts_with = "only_scope")]
        scope: Option<String>,

        /// Only this scope.
        #[arg(long = "only-scope", value_name = "SCOPE", conflicts_with = "scope")]
        only_scope: Option<String>,

        /// Combine plain terms with OR instead of AND.
        #[arg(long)]
        any: bool,

        /// Rust regular expression matched against title and content.
        #[arg(long, value_name = "PATTERN")]
        regex: Option<String>,

        /// Only memories of this kind.
        #[arg(long, value_enum, value_name = "KIND")]
        kind: Option<KindArg>,

        /// Only memories with this tag.
        #[arg(long, value_name = "TAG")]
        tag: Option<String>,

        /// Maximum number of rows.
        #[arg(long, value_name = "N")]
        limit: Option<u32>,
    },

    /// List scopes with unexpired memory counts.
    Scopes,

    /// Show the queryable views and tables.
    Schema,

    /// Run one read-only SQL statement.
    Sql {
        /// SQL statement. Writes are rejected.
        #[arg(value_name = "QUERY")]
        query: String,

        /// Maximum number of rows. Defaults to 1000.
        #[arg(long = "max-rows", value_name = "N")]
        max_rows: Option<u32>,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum KindArg {
    Fact,
    Decision,
    Note,
}

impl From<KindArg> for MemoryKind {
    fn from(value: KindArg) -> Self {
        match value {
            KindArg::Fact => Self::Fact,
            KindArg::Decision => Self::Decision,
            KindArg::Note => Self::Note,
        }
    }
}

pub fn run() -> Result<(), String> {
    let cli = Cli::parse();
    let mut store = open_store(cli.db.as_deref())?;
    match cli.command {
        Command::Add {
            content,
            file,
            title,
            kind,
            scope,
            tags,
            agent,
            expires,
        } => cmd_add(
            &mut store,
            cli.json,
            AddInput {
                content,
                file,
                title,
                kind,
                scope,
                tags,
                agent,
                expires,
            },
        ),
        Command::Get { id } => cmd_get(&store, cli.json, id),
        Command::Update {
            id,
            content,
            file,
            title,
            kind,
            scope,
            add_tag,
            remove_tag,
            expires,
        } => cmd_update(
            &mut store,
            cli.json,
            UpdateInput {
                id,
                content,
                file,
                title,
                kind,
                scope,
                add_tag,
                remove_tag,
                expires,
            },
        ),
        Command::Forget { id } => cmd_forget(&mut store, cli.json, id),
        Command::List {
            tag,
            scope,
            kind,
            limit,
        } => cmd_list(&store, cli.json, tag, scope, kind, limit),
        Command::Search {
            terms,
            scope,
            only_scope,
            any,
            regex,
            kind,
            tag,
            limit,
        } => cmd_search(
            &store,
            cli.json,
            SearchInput {
                terms,
                scope,
                only_scope,
                any,
                regex,
                kind,
                tag,
                limit,
            },
        ),
        Command::Scopes => cmd_scopes(&store, cli.json),
        Command::Schema => cmd_schema(&store, cli.json),
        Command::Sql { query, max_rows } => cmd_sql(cli.db.as_deref(), cli.json, query, max_rows),
    }
}

struct SearchInput {
    terms: Vec<String>,
    scope: Option<String>,
    only_scope: Option<String>,
    any: bool,
    regex: Option<String>,
    kind: Option<KindArg>,
    tag: Option<String>,
    limit: Option<u32>,
}

struct AddInput {
    content: Option<String>,
    file: Option<PathBuf>,
    title: Option<String>,
    kind: Option<KindArg>,
    scope: Option<String>,
    tags: Option<String>,
    agent: Option<String>,
    expires: Option<String>,
}

struct UpdateInput {
    id: i64,
    content: Option<String>,
    file: Option<PathBuf>,
    title: Option<String>,
    kind: Option<KindArg>,
    scope: Option<String>,
    add_tag: Vec<String>,
    remove_tag: Vec<String>,
    expires: Option<String>,
}

fn cmd_add(store: &mut Store, json_output: bool, input: AddInput) -> Result<(), String> {
    let body = read_body(input.content.as_deref(), input.file.as_deref(), true)?;
    let expires_at = input.expires.as_deref().map(parse_expires).transpose()?;
    let inserted = store
        .insert(&NewMemory {
            title: input.title,
            content: body,
            kind: input.kind.map(MemoryKind::from).unwrap_or_default(),
            scope: input.scope.unwrap_or_else(|| "global".to_owned()),
            source_agent: input.agent,
            tags: input.tags.as_deref().map(split_tags).unwrap_or_default(),
            expires_at,
        })
        .map_err(store_err)?;
    if json_output {
        let mut value = memory_json(&inserted.memory);
        value["deduplicated"] = Value::Bool(!inserted.created);
        print_json(&value)
    } else if inserted.created {
        println!("added {}", inserted.memory.id);
        Ok(())
    } else {
        println!("deduplicated {}", inserted.memory.id);
        Ok(())
    }
}

fn cmd_get(store: &Store, json_output: bool, id: i64) -> Result<(), String> {
    let memory = load_visible(store, id)?;
    emit_memory(json_output, &memory)
}

fn cmd_update(store: &mut Store, json_output: bool, input: UpdateInput) -> Result<(), String> {
    if input.content.is_none()
        && input.file.is_none()
        && input.title.is_none()
        && input.kind.is_none()
        && input.scope.is_none()
        && input.add_tag.is_empty()
        && input.remove_tag.is_empty()
        && input.expires.is_none()
    {
        return Err("memo: nothing to update".to_owned());
    }
    let current = load_any(store, input.id)?;
    let new_content = if input.content.is_some() || input.file.is_some() {
        Some(read_body(
            input.content.as_deref(),
            input.file.as_deref(),
            false,
        )?)
    } else {
        None
    };
    let tags = if input.add_tag.is_empty() && input.remove_tag.is_empty() {
        None
    } else {
        Some(edit_tags(&current.tags, &input.remove_tag, &input.add_tag))
    };
    let expires_at = match input.expires.as_deref() {
        Some(value) => Patch::Set(parse_expires(value)?),
        None => Patch::Unchanged,
    };
    let updated = store
        .update(
            input.id,
            &MemoryUpdate {
                title: match input.title {
                    Some(value) => Patch::Set(value),
                    None => Patch::Unchanged,
                },
                content: new_content,
                kind: input.kind.map(MemoryKind::from),
                scope: input.scope,
                expires_at,
                tags,
                ..MemoryUpdate::default()
            },
        )
        .map_err(store_err)?
        .ok_or_else(|| missing(input.id))?;
    if json_output {
        print_json(&memory_json(&updated))
    } else {
        println!("updated {}", updated.id);
        Ok(())
    }
}

fn cmd_forget(store: &mut Store, json_output: bool, id: i64) -> Result<(), String> {
    let memory = load_any(store, id)?;
    if !store.delete(id).map_err(store_err)? {
        return Err(missing(id));
    }
    if json_output {
        print_json(&memory_json(&memory))
    } else {
        println!("forgot {id}");
        Ok(())
    }
}

fn cmd_list(
    store: &Store,
    json_output: bool,
    tag: Option<String>,
    scope: Option<String>,
    kind: Option<KindArg>,
    limit: Option<u32>,
) -> Result<(), String> {
    let tag = tag
        .as_deref()
        .map(|value| non_empty("tag", value))
        .transpose()?;
    let scope = scope
        .as_deref()
        .map(|value| non_empty("scope", value))
        .transpose()?;
    let memories = store
        .list(&ReadOptions {
            scope,
            kind: kind.map(MemoryKind::from),
            tag,
            include_expired: false,
            limit,
        })
        .map_err(store_err)?;
    if json_output {
        let values: Vec<Value> = memories.iter().map(memory_json).collect();
        print_json(&Value::Array(values))
    } else {
        for memory in &memories {
            println!(
                "{}  {}  {}  {}",
                memory.id,
                memory.kind,
                memory.scope,
                preview(memory)
            );
        }
        Ok(())
    }
}

fn cmd_search(store: &Store, json_output: bool, input: SearchInput) -> Result<(), String> {
    if input.terms.is_empty() && input.regex.is_none() {
        return Err("memo: provide search terms or --regex".to_owned());
    }
    let scope = scope_query(input.scope, input.only_scope)?;
    let pattern = input.regex.as_deref().map(compile_regex).transpose()?;
    let tag = input
        .tag
        .as_deref()
        .map(|value| non_empty("tag", value))
        .transpose()?;
    let defer_limit = pattern.is_some();
    let options = ReadOptions {
        scope: None,
        kind: input.kind.map(MemoryKind::from),
        tag,
        include_expired: false,
        limit: if defer_limit { None } else { input.limit },
    };
    let mut hits = if input.terms.is_empty() {
        regex_only_hits(store, &scope, &options, pattern.as_ref().expect("pattern"))?
    } else {
        let query = compile_fts_query(&input.terms, input.any).map_err(store_err)?;
        let mut found = store.search(&query, &scope, &options).map_err(store_err)?;
        if let Some(pattern) = &pattern {
            found.retain(|hit| pattern.is_match(&haystack(&hit.memory)));
        }
        found
    };
    if defer_limit {
        if let Some(limit) = input.limit {
            let limit = usize::try_from(limit).unwrap_or(usize::MAX);
            hits.truncate(limit);
        }
    }
    emit_hits(json_output, &hits)
}

fn cmd_scopes(store: &Store, json_output: bool) -> Result<(), String> {
    let scopes = store.scopes().map_err(store_err)?;
    if json_output {
        let values: Vec<Value> = scopes
            .iter()
            .map(|row| {
                json!({
                    "scope": row.scope,
                    "count": row.count,
                    "last_updated": row.last_updated,
                })
            })
            .collect();
        return print_json(&Value::Array(values));
    }
    for row in &scopes {
        println!(
            "{}  {}  {}",
            row.scope,
            row.count,
            row.last_updated.as_deref().unwrap_or("-")
        );
    }
    Ok(())
}

fn scope_query(scope: Option<String>, only_scope: Option<String>) -> Result<ScopeQuery, String> {
    match (scope, only_scope) {
        (Some(value), None) => Ok(ScopeQuery::WithGlobal(non_empty("scope", &value)?)),
        (None, Some(value)) => Ok(ScopeQuery::Only(non_empty("scope", &value)?)),
        (None, None) => Ok(ScopeQuery::All),
        (Some(_), Some(_)) => Err("memo: --scope and --only-scope cannot be combined".to_owned()),
    }
}

fn compile_regex(pattern: &str) -> Result<regex::Regex, String> {
    regex::Regex::new(pattern).map_err(|err| format!("memo: invalid regular expression: {err}"))
}

fn regex_only_hits(
    store: &Store,
    scope: &ScopeQuery,
    options: &ReadOptions,
    pattern: &regex::Regex,
) -> Result<Vec<SearchHit>, String> {
    let list_scope = match scope {
        ScopeQuery::Only(value) => Some(value.clone()),
        ScopeQuery::All | ScopeQuery::WithGlobal(_) => None,
    };
    let memories = store
        .list(&ReadOptions {
            scope: list_scope,
            limit: None,
            ..options.clone()
        })
        .map_err(store_err)?;
    Ok(memories
        .into_iter()
        .filter(|memory| scope_allows(scope, &memory.scope))
        .filter_map(|memory| {
            let text = haystack(&memory);
            let found = pattern.find(&text)?;
            Some(SearchHit {
                memory,
                rank: 0.0,
                snippet: window_snippet(&text, found.start(), found.end()),
            })
        })
        .collect())
}

fn scope_allows(scope: &ScopeQuery, value: &str) -> bool {
    match scope {
        ScopeQuery::All => true,
        ScopeQuery::WithGlobal(wanted) => value == wanted || value == "global",
        ScopeQuery::Only(wanted) => value == wanted,
    }
}

fn haystack(memory: &Memory) -> String {
    match memory.title.as_deref() {
        Some(title) => format!("{title}\n{}", memory.content),
        None => memory.content.clone(),
    }
}

fn window_snippet(text: &str, start: usize, end: usize) -> String {
    let mut from = start.saturating_sub(48);
    while from > 0 && !text.is_char_boundary(from) {
        from -= 1;
    }
    let mut to = (end + 48).min(text.len());
    while to < text.len() && !text.is_char_boundary(to) {
        to += 1;
    }
    let mut snippet = String::new();
    if from > 0 {
        snippet.push_str("...");
    }
    snippet.push_str(&text[from..to]);
    if to < text.len() {
        snippet.push_str("...");
    }
    snippet
}

fn emit_hits(json_output: bool, hits: &[SearchHit]) -> Result<(), String> {
    if json_output {
        let values: Vec<Value> = hits.iter().map(search_json).collect();
        return print_json(&Value::Array(values));
    }
    for hit in hits {
        let text = if hit.snippet.trim().is_empty() {
            preview(&hit.memory)
        } else {
            one_line(&hit.snippet)
        };
        println!(
            "{}  {}  {}  {}",
            hit.memory.id, hit.memory.kind, hit.memory.scope, text
        );
    }
    Ok(())
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn search_json(hit: &SearchHit) -> Value {
    let mut value = memory_json(&hit.memory);
    value["rank"] = Value::from(hit.rank);
    value["snippet"] = Value::String(hit.snippet.clone());
    value
}

fn cmd_schema(store: &Store, json_output: bool) -> Result<(), String> {
    let objects = store.schema().map_err(store_err)?;
    if json_output {
        let values: Vec<Value> = objects.iter().map(schema_json).collect();
        return print_json(&Value::Array(values));
    }
    for (index, object) in objects.iter().enumerate() {
        if index > 0 {
            println!();
        }
        println!("{} ({})", object.name, object.kind);
        println!("{}", object.description);
        let width = object
            .columns
            .iter()
            .map(|column| column.name.chars().count())
            .max()
            .unwrap_or(0);
        for column in &object.columns {
            println!(
                "  {:width$}  {}",
                column.name,
                column.description,
                width = width
            );
        }
    }
    Ok(())
}

fn schema_json(object: &SchemaObject) -> Value {
    json!({
        "name": object.name,
        "type": object.kind,
        "description": object.description,
        "columns": object.columns.iter().map(|column| json!({
            "name": column.name,
            "type": column.type_name,
            "description": column.description,
        })).collect::<Vec<_>>(),
    })
}

fn cmd_sql(
    db: Option<&Path>,
    json_output: bool,
    query: String,
    max_rows: Option<u32>,
) -> Result<(), String> {
    let path = memo::resolve_db_path(db).map_err(store_err)?;
    let max_rows = max_rows.unwrap_or(SQL_ROW_CAP);
    let table = query_readonly(&path, &query, max_rows).map_err(store_err)?;
    if json_output {
        print_json(&sql_json(&table))?;
    } else {
        print_sql_table(&table);
    }
    if table.truncated {
        eprintln!("memo: results truncated to {max_rows} rows");
    }
    Ok(())
}

fn sql_json(table: &SqlTable) -> Value {
    Value::Array(
        table
            .rows
            .iter()
            .map(|row| {
                let mut object = serde_json::Map::new();
                for (column, cell) in table.columns.iter().zip(row) {
                    object.insert(column.clone(), sql_cell_json(cell));
                }
                Value::Object(object)
            })
            .collect(),
    )
}

fn sql_cell_json(cell: &SqlCell) -> Value {
    match cell {
        SqlCell::Null => Value::Null,
        SqlCell::Integer(value) => Value::from(*value),
        SqlCell::Real(value) => serde_json::Number::from_f64(*value)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        SqlCell::Text(value) => Value::String(value.clone()),
        SqlCell::Blob(value) => Value::Array(value.iter().copied().map(Value::from).collect()),
    }
}

fn print_sql_table(table: &SqlTable) {
    let rendered: Vec<Vec<String>> = table
        .rows
        .iter()
        .map(|row| row.iter().map(display_cell).collect())
        .collect();
    let mut widths: Vec<usize> = table
        .columns
        .iter()
        .map(|column| column.chars().count())
        .collect();
    for row in &rendered {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.chars().count());
        }
    }
    print_aligned(&table.columns, &widths);
    for row in &rendered {
        print_aligned(row, &widths);
    }
}

fn print_aligned(cells: &[impl AsRef<str>], widths: &[usize]) {
    let line = cells
        .iter()
        .enumerate()
        .map(|(index, cell)| {
            let width = widths.get(index).copied().unwrap_or(0);
            format!("{:width$}", cell.as_ref(), width = width)
        })
        .collect::<Vec<_>>()
        .join("  ");
    println!("{line}");
}

fn display_cell(cell: &SqlCell) -> String {
    match cell {
        SqlCell::Null => String::new(),
        SqlCell::Integer(value) => value.to_string(),
        SqlCell::Real(value) => value.to_string(),
        SqlCell::Text(value) => value.split_whitespace().collect::<Vec<_>>().join(" "),
        SqlCell::Blob(value) => format!("<{} bytes>", value.len()),
    }
}

fn open_store(db: Option<&Path>) -> Result<Store, String> {
    let path = memo::resolve_db_path(db).map_err(store_err)?;
    Store::open(path).map_err(store_err)
}

fn load_visible(store: &Store, id: i64) -> Result<Memory, String> {
    store.get(id).map_err(store_err)?.ok_or_else(|| missing(id))
}

fn load_any(store: &Store, id: i64) -> Result<Memory, String> {
    store
        .get_including_expired(id)
        .map_err(store_err)?
        .ok_or_else(|| missing(id))
}

fn read_body(
    content: Option<&str>,
    file: Option<&Path>,
    implicit_stdin: bool,
) -> Result<String, String> {
    if let Some(path) = file {
        return fs::read_to_string(path)
            .map_err(|err| format!("memo: cannot read {}: {err}", path.display()));
    }
    match content {
        Some("-") => read_stdin(),
        Some(text) => Ok(text.to_owned()),
        None if implicit_stdin && !io::stdin().is_terminal() => read_stdin(),
        None => Err("memo: content is required (argument, stdin, or --file)".to_owned()),
    }
}

fn read_stdin() -> Result<String, String> {
    let mut buf = String::new();
    io::stdin()
        .read_to_string(&mut buf)
        .map_err(|err| format!("memo: cannot read stdin: {err}"))?;
    Ok(buf)
}

fn parse_expires(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if DateTime::parse_from_rfc3339(trimmed).is_ok() {
        return Ok(trimmed.to_owned());
    }
    if NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").is_ok() {
        return Ok(format!("{trimmed}T00:00:00Z"));
    }
    Err(format!("memo: invalid timestamp: {value}"))
}

fn split_tags(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_owned)
        .collect()
}

fn edit_tags(current: &[String], remove: &[String], add: &[String]) -> Vec<String> {
    let remove: Vec<String> = remove.iter().map(|tag| tag.trim().to_owned()).collect();
    let mut tags: Vec<String> = current
        .iter()
        .filter(|tag| !remove.iter().any(|gone| gone == *tag))
        .cloned()
        .collect();
    for tag in add {
        let tag = tag.trim();
        if tag.is_empty() || tags.iter().any(|existing| existing == tag) {
            continue;
        }
        tags.push(tag.to_owned());
    }
    tags
}

fn non_empty(label: &str, value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(format!("memo: {label} must not be empty"))
    } else {
        Ok(trimmed.to_owned())
    }
}

fn preview(memory: &Memory) -> String {
    let source = memory.title.as_deref().unwrap_or(memory.content.as_str());
    let line = source.split_whitespace().collect::<Vec<_>>().join(" ");
    const MAX: usize = 72;
    if line.chars().count() <= MAX {
        line
    } else {
        let mut truncated: String = line.chars().take(MAX - 3).collect();
        truncated.push_str("...");
        truncated
    }
}

fn memory_json(memory: &Memory) -> Value {
    json!({
        "id": memory.id,
        "scope": memory.scope,
        "kind": memory.kind.as_str(),
        "title": memory.title,
        "content": memory.content,
        "tags": memory.tags,
        "source_agent": memory.source_agent,
        "created_at": memory.created_at,
        "updated_at": memory.updated_at,
        "expires_at": memory.expires_at,
    })
}

fn emit_memory(json_output: bool, memory: &Memory) -> Result<(), String> {
    if json_output {
        return print_json(&memory_json(memory));
    }
    println!("id: {}", memory.id);
    println!("scope: {}", memory.scope);
    println!("kind: {}", memory.kind);
    println!("title: {}", display_optional(memory.title.as_deref()));
    println!(
        "tags: {}",
        if memory.tags.is_empty() {
            "-".to_owned()
        } else {
            memory.tags.join(", ")
        }
    );
    println!(
        "source_agent: {}",
        display_optional(memory.source_agent.as_deref())
    );
    println!("created_at: {}", memory.created_at);
    println!("updated_at: {}", memory.updated_at);
    println!(
        "expires_at: {}",
        display_optional(memory.expires_at.as_deref())
    );
    println!();
    if memory.content.ends_with('\n') {
        print!("{}", memory.content);
    } else {
        println!("{}", memory.content);
    }
    Ok(())
}

fn display_optional(value: Option<&str>) -> &str {
    value.unwrap_or("-")
}

fn print_json(value: &Value) -> Result<(), String> {
    let line = serde_json::to_string(value).map_err(|err| format!("memo: {err}"))?;
    println!("{line}");
    Ok(())
}

fn missing(id: i64) -> String {
    format!("memo: memory {id} not found")
}

fn store_err(err: impl std::fmt::Display) -> String {
    format!("memo: {err}")
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::Cli;

    #[test]
    fn version_help_db_json_and_commands_are_present() {
        let mut command = Cli::command();
        command.build();

        assert!(command.get_version().is_some());
        let longs: Vec<_> = command
            .get_arguments()
            .filter_map(|arg| arg.get_long())
            .collect();
        assert!(longs.contains(&"help"));
        assert!(longs.contains(&"db"));
        assert!(longs.contains(&"json"));

        let names: Vec<_> = command
            .get_subcommands()
            .map(|cmd| cmd.get_name())
            .collect();
        for name in [
            "add", "get", "update", "forget", "list", "search", "scopes", "schema", "sql",
        ] {
            assert!(names.contains(&name), "{names:?}");
        }
    }
}
