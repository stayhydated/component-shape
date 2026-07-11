/// Convert a user field name into an identifier-safe fragment for assertions.
pub fn field_assertion_ident_fragment(field_name: &str) -> String {
    let field_name = field_name.strip_prefix("r#").unwrap_or(field_name);
    let mut fragment = String::new();
    for ch in field_name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            fragment.push(ch);
        } else {
            fragment.push('_');
        }
    }

    if fragment.is_empty() {
        fragment.push_str("field");
    }
    if fragment
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_digit())
    {
        fragment.insert(0, '_');
    }

    fragment
}

/// Validate shape-owned `field_suffix` metadata.
pub fn validate_shape_field_suffix(value: &str) -> Result<(), String> {
    component_shape::validate_component_suffix(value).map_err(|err| err.to_string())
}
