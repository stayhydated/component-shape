use super::*;

/// Extract a rustdoc description from contiguous non-empty `///` lines.
pub fn doc_description(attrs: &[syn::Attribute]) -> Option<String> {
    let mut lines = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .filter_map(|attr| match &attr.meta {
            syn::Meta::NameValue(meta) => match &meta.value {
                Expr::Lit(expr) => match &expr.lit {
                    syn::Lit::Str(value) => Some(value.value().trim().to_string()),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();

    let first = lines.iter().position(|line| !line.is_empty())?;
    let last = lines.iter().rposition(|line| !line.is_empty())?;
    lines.drain(..first);
    lines.truncate(last - first + 1);

    Some(lines.join("\n"))
}
