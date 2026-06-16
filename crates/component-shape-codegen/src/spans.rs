use super::*;

/// Replace every token span in `value` with `span`.
pub fn tokens_with_span<T: quote::ToTokens>(value: &T, span: Span) -> TokenStream {
    value
        .to_token_stream()
        .into_iter()
        .map(|token| token_tree_with_span(token, span))
        .collect()
}

fn token_tree_with_span(mut token: TokenTree, span: Span) -> TokenTree {
    if let TokenTree::Group(group) = &mut token {
        let mut new_group = Group::new(
            group.delimiter(),
            group
                .stream()
                .into_iter()
                .map(|token| token_tree_with_span(token, span))
                .collect(),
        );
        new_group.set_span(span);
        return TokenTree::Group(new_group);
    }

    match &mut token {
        TokenTree::Group(_) => unreachable!("groups are handled above"),
        TokenTree::Ident(ident) => ident.set_span(span),
        TokenTree::Punct(punct) => punct.set_span(span),
        TokenTree::Literal(literal) => literal.set_span(span),
    }

    token
}
