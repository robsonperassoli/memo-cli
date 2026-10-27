//! Compile command-line search terms into an FTS5 `MATCH` expression.
//!
//! Plain terms are quoted and joined with `AND` or `OR`. A trailing `*` is a
//! prefix. Queries that already use FTS5 syntax are passed through unchanged.

use super::markdown::indexed_text;
use crate::error::{Error, Result};

/// Build an FTS5 query from the positional terms of `memo search`.
///
/// `any` joins plain terms with `OR`. It does not rewrite raw FTS5 syntax.
pub fn compile_fts_query<T: AsRef<str>>(terms: &[T], any: bool) -> Result<String> {
    if terms.is_empty() || terms.iter().all(|term| term.as_ref().trim().is_empty()) {
        return Err(Error::InvalidSearch(
            "provide search terms or --regex".to_owned(),
        ));
    }
    if query_is_raw(terms) {
        return Ok(terms
            .iter()
            .map(|term| term.as_ref())
            .collect::<Vec<_>>()
            .join(" "));
    }
    let mut parts = Vec::with_capacity(terms.len());
    for term in terms {
        parts.push(compile_plain_term(term.as_ref())?);
    }
    let joiner = if any { " OR " } else { " AND " };
    Ok(parts.join(joiner))
}

fn query_is_raw<T: AsRef<str>>(terms: &[T]) -> bool {
    let mut tokens = 0;
    let mut saw_operator = false;
    for term in terms {
        let term = term.as_ref();
        if is_structurally_raw(term) {
            return true;
        }
        for token in term.split_whitespace() {
            tokens += 1;
            if is_operator_token(token) {
                saw_operator = true;
            }
        }
    }
    saw_operator && tokens > 1
}

fn is_structurally_raw(term: &str) -> bool {
    term.contains('"')
        || term.contains('(')
        || term.contains(')')
        || term.split_whitespace().any(is_column_filter)
}

fn is_column_filter(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    lower.starts_with("title:") || lower.starts_with("content:") || lower.starts_with("tags:")
}

fn is_operator_token(token: &str) -> bool {
    let trimmed = token.trim_matches(|ch: char| matches!(ch, '(' | ')' | ','));
    let upper = trimmed.to_ascii_uppercase();
    if matches!(upper.as_str(), "OR" | "AND" | "NOT" | "NEAR") {
        return true;
    }
    match upper.strip_prefix("NEAR/") {
        Some(distance) => !distance.is_empty() && distance.chars().all(|ch| ch.is_ascii_digit()),
        None => false,
    }
}

fn compile_plain_term(term: &str) -> Result<String> {
    if term.trim().is_empty() {
        return Err(Error::InvalidSearch(
            "search term must not be empty".to_owned(),
        ));
    }
    let (body, prefix) = match term.strip_suffix('*') {
        Some(body) => (body, true),
        None => (term, false),
    };
    let text = indexed_text(body)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if text.is_empty() {
        return Err(Error::InvalidSearch(format!(
            "search term {term:?} is empty after removing markdown symbols"
        )));
    }
    let escaped = text.replace('"', "\"\"");
    if prefix {
        Ok(format!("\"{escaped}\"*"))
    } else {
        Ok(format!("\"{escaped}\""))
    }
}

#[cfg(test)]
mod tests {
    use super::compile_fts_query;
    use crate::error::Error;

    #[test]
    fn plain_terms_are_anded_unless_any_is_set() {
        let terms = ["alpha", "beta"];
        assert_eq!(
            compile_fts_query(&terms, false).unwrap(),
            r#""alpha" AND "beta""#
        );
        assert_eq!(
            compile_fts_query(&terms, true).unwrap(),
            r#""alpha" OR "beta""#
        );
    }

    #[test]
    fn prefix_and_phrase_terms() {
        assert_eq!(
            compile_fts_query(&["deploy*"], false).unwrap(),
            r#""deploy"*"#
        );
        assert_eq!(
            compile_fts_query(&["red apple"], false).unwrap(),
            r#""red apple""#
        );
    }

    #[test]
    fn raw_fts_passes_through_without_rewriting_any() {
        assert_eq!(
            compile_fts_query(&["foo OR bar"], true).unwrap(),
            "foo OR bar"
        );
        assert_eq!(
            compile_fts_query(&["red", "NEAR/2", "apple"], false).unwrap(),
            "red NEAR/2 apple"
        );
        assert_eq!(
            compile_fts_query(&["\"red apple\""], false).unwrap(),
            "\"red apple\""
        );
        assert_eq!(
            compile_fts_query(&["title:deploy"], false).unwrap(),
            "title:deploy"
        );
    }

    #[test]
    fn markdown_symbols_are_removed_and_a_trailing_star_stays() {
        assert_eq!(
            compile_fts_query(&["#headingquark"], false).unwrap(),
            r#""headingquark""#
        );
        assert_eq!(compile_fts_query(&["`code`"], false).unwrap(), r#""code""#);
        assert_eq!(
            compile_fts_query(&["#deploy*"], false).unwrap(),
            r#""deploy"*"#
        );
    }

    #[test]
    fn a_bare_operator_or_nearness_is_a_plain_word() {
        assert_eq!(compile_fts_query(&["or"], false).unwrap(), r#""or""#);
        assert_eq!(
            compile_fts_query(&["nearness"], false).unwrap(),
            r#""nearness""#
        );
    }

    #[test]
    fn an_empty_query_is_rejected() {
        let err = compile_fts_query::<&str>(&[], false).unwrap_err();
        assert!(matches!(err, Error::InvalidSearch(_)));
    }
}
