pub fn tool_name(source_module_path: &str, subject_id: &str, fallback_prefix: &str) -> String {
    #[derive(Clone, Copy, Eq, PartialEq)]
    enum CharKind {
        Upper,
        Lower,
        Digit,
    }

    let input: Vec<char> = source_module_path
        .chars()
        .chain(['_'])
        .chain(subject_id.chars())
        .collect();
    let mut output = String::new();
    let mut last_was_separator = false;
    let mut last_kind = None;

    for (index, ch) in input.iter().copied().enumerate() {
        let kind = if ch.is_ascii_uppercase() {
            CharKind::Upper
        } else if ch.is_ascii_lowercase() {
            CharKind::Lower
        } else if ch.is_ascii_digit() {
            CharKind::Digit
        } else if !last_was_separator && !output.is_empty() {
            output.push('_');
            last_was_separator = true;
            last_kind = None;
            continue;
        } else {
            last_kind = None;
            continue;
        };

        if kind == CharKind::Upper && !last_was_separator && !output.is_empty() {
            let next_is_lower = input
                .get(index + 1)
                .is_some_and(|next| next.is_ascii_lowercase());
            if matches!(last_kind, Some(CharKind::Lower | CharKind::Digit))
                || (matches!(last_kind, Some(CharKind::Upper)) && next_is_lower)
            {
                output.push('_');
            }
        }

        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
            last_was_separator = false;
            last_kind = Some(kind);
        }
    }

    while output.ends_with('_') {
        output.pop();
    }

    if output.is_empty() || output.starts_with(|ch: char| ch.is_ascii_digit()) {
        output.insert_str(0, fallback_prefix);
    }

    output
}
