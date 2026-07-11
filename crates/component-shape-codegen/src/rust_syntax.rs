use super::*;

fn rust_metadata_string<T: quote::ToTokens>(tokens: &T) -> String {
    quote::ToTokens::to_token_stream(tokens).to_string()
}

/// Emit construction for a Rust syntax metadata wrapper.
pub fn rust_syntax_metadata_tokens(
    wrapper_path: impl quote::ToTokens,
    syntax: &impl quote::ToTokens,
) -> TokenStream {
    let syntax = rust_metadata_string(syntax);
    quote! {
        #wrapper_path::from_macro_tokens_unchecked(#syntax)
    }
}

/// Emit construction for a `RustPath` metadata wrapper.
pub fn rust_path_metadata_tokens(rust_path_type: impl quote::ToTokens, path: &Path) -> TokenStream {
    rust_syntax_metadata_tokens(rust_path_type, path)
}

/// Emit construction for a `RustType` metadata wrapper.
pub fn rust_type_metadata_tokens(rust_type_type: impl quote::ToTokens, ty: &Type) -> TokenStream {
    rust_syntax_metadata_tokens(rust_type_type, ty)
}

/// Emit construction for a `RustExpr` metadata wrapper.
pub fn rust_expr_metadata_tokens(
    rust_expr_type: impl quote::ToTokens,
    expr: &syn::Expr,
) -> TokenStream {
    rust_syntax_metadata_tokens(rust_expr_type, expr)
}

/// Emit construction for an optional `RustExpr` metadata wrapper.
pub fn optional_rust_expr_metadata_tokens(
    rust_expr_type: impl quote::ToTokens,
    expr: Option<&syn::Expr>,
) -> TokenStream {
    match expr {
        Some(expr) => {
            let expr = rust_expr_metadata_tokens(rust_expr_type, expr);
            quote! { Some(#expr) }
        },
        None => quote! { None },
    }
}
