/// Text stored in `memories_fts`.
///
/// Link syntax keeps the label and drops the destination. The symbols `#`,
/// `` ` ``, `*`, `_`, and `>` are removed. `memories.content` itself is not
/// modified.
pub(crate) fn indexed_text(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut index = 0;
    while index < chars.len() {
        if let Some((label, next)) = parse_link(&chars, index) {
            push_stripped(&mut out, &label);
            index = next;
            continue;
        }
        if !is_markdown_symbol(chars[index]) {
            out.push(chars[index]);
        }
        index += 1;
    }
    out
}

fn parse_link(chars: &[char], start: usize) -> Option<(String, usize)> {
    let mut index = start;
    if chars.get(index) == Some(&'!') {
        index += 1;
    }
    if chars.get(index) != Some(&'[') {
        return None;
    }
    let label_start = index + 1;
    let label_end = find_char(chars, label_start, ']')?;
    let label: String = chars[label_start..label_end].iter().collect();
    let after = label_end + 1;
    let end = match chars.get(after)? {
        '(' => find_char(chars, after + 1, ')')? + 1,
        '[' => find_char(chars, after + 1, ']')? + 1,
        _ => return None,
    };
    Some((label, end))
}

fn find_char(chars: &[char], start: usize, target: char) -> Option<usize> {
    chars[start..]
        .iter()
        .position(|ch| *ch == target)
        .map(|offset| start + offset)
}

fn push_stripped(out: &mut String, text: &str) {
    for ch in text.chars() {
        if !is_markdown_symbol(ch) {
            out.push(ch);
        }
    }
}

fn is_markdown_symbol(ch: char) -> bool {
    matches!(ch, '#' | '`' | '*' | '_' | '>')
}

#[cfg(test)]
mod tests {
    use super::indexed_text;

    #[test]
    fn strips_symbols_and_keeps_link_labels() {
        let indexed = indexed_text(
            "# Title `code` *bold* _em_ > quote [docs](https://example.com) ![pic](img.png)",
        );
        assert_eq!(indexed, " Title code bold em  quote docs pic");
        assert!(!indexed.contains("example.com"));
        assert!(!indexed.contains("img.png"));
    }

    #[test]
    fn reference_links_keep_the_label_only() {
        assert_eq!(indexed_text("see [docs][ref]"), "see docs");
    }

    #[test]
    fn brackets_that_are_not_links_stay() {
        assert_eq!(indexed_text("array[0]"), "array[0]");
    }
}
