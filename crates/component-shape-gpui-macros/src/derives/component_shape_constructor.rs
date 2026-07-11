use proc_macro2::TokenStream;
use quote::quote;
use syn::Expr;

pub fn constructor_body_tokens(new_expr: &Expr) -> TokenStream {
    if accepts_window_cx_arguments(new_expr) {
        quote! { (#new_expr)(window, cx) }
    } else {
        quote! { #new_expr }
    }
}

fn accepts_window_cx_arguments(expr: &Expr) -> bool {
    match expr {
        Expr::Group(group) => accepts_window_cx_arguments(&group.expr),
        Expr::Paren(paren) => accepts_window_cx_arguments(&paren.expr),
        Expr::Closure(_) | Expr::Path(_) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use quote::ToTokens as _;
    use syn::parse_quote;

    use super::constructor_body_tokens;

    fn compact(expr: syn::Expr) -> String {
        constructor_body_tokens(&expr)
            .to_token_stream()
            .to_string()
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect()
    }

    #[test]
    fn constructor_paths_closures_and_wrappers_receive_gpui_context() {
        assert_eq!(compact(parse_quote!(State::new)), "(State::new)(window,cx)");
        assert_eq!(
            compact(parse_quote!((|window, cx| State::new(window, cx)))),
            "((|window,cx|State::new(window,cx)))(window,cx)"
        );
        let grouped = syn::Expr::Group(syn::ExprGroup {
            attrs: Vec::new(),
            group_token: Default::default(),
            expr: Box::new(parse_quote!(State::new)),
        });
        assert_eq!(compact(grouped), "(State::new)(window,cx)");
    }

    #[test]
    fn complete_constructor_expressions_are_emitted_unchanged() {
        assert_eq!(compact(parse_quote!(State::default())), "State::default()");
        assert_eq!(compact(parse_quote!(State { value: 1 })), "State{value:1}");
    }
}
