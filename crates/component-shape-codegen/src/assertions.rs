use super::*;

/// Emit a concrete field/shape compatibility assertion.
///
/// The trait inputs are token streams so consumers can use their own runtime
/// contracts while sharing the assertion shape and generated identifier policy.
pub fn shape_type_assertion_tokens(
    assertion_prefix: &str,
    field_name: &str,
    shape_path: &impl quote::ToTokens,
    field_type: &impl quote::ToTokens,
    span: Span,
    declared_shape_trait_bounds: impl IntoIterator<Item = TokenStream>,
    compatibility_trait_path: impl quote::ToTokens,
) -> TokenStream {
    shape_type_assertion_tokens_with_suffixes(
        assertion_prefix,
        field_name,
        shape_path,
        field_type,
        span,
        declared_shape_trait_bounds,
        compatibility_trait_path,
        "declared_shape",
        "shape_compatibility",
    )
}

/// Emit a concrete field/shape compatibility assertion with consumer-specific
/// assertion name suffixes.
#[allow(clippy::too_many_arguments)]
pub fn shape_type_assertion_tokens_with_suffixes(
    assertion_prefix: &str,
    field_name: &str,
    shape_path: &impl quote::ToTokens,
    field_type: &impl quote::ToTokens,
    span: Span,
    declared_shape_trait_bounds: impl IntoIterator<Item = TokenStream>,
    compatibility_trait_path: impl quote::ToTokens,
    declared_assertion_suffix: &str,
    compatibility_assertion_suffix: &str,
) -> TokenStream {
    let field_fragment = field_assertion_ident_fragment(field_name);
    let declared_assertion_ident = format_ident!(
        "__{assertion_prefix}_assert_{field_fragment}_{declared_assertion_suffix}",
        span = span
    );
    let compatibility_assertion_ident = format_ident!(
        "__{assertion_prefix}_assert_{field_fragment}_{compatibility_assertion_suffix}",
        span = span
    );
    let declared_shape_trait_bounds: Vec<TokenStream> = declared_shape_trait_bounds
        .into_iter()
        .map(|tokens| tokens_with_span(&tokens, span))
        .collect();
    let compatibility_trait_path = tokens_with_span(&compatibility_trait_path, span);

    quote_spanned! {span=>
        const _: () = {
            #[allow(non_snake_case)]
            const fn #declared_assertion_ident<Shape>()
            where
                #(
                    Shape: #declared_shape_trait_bounds,
                )*
            {
            }
            #declared_assertion_ident::<#shape_path>();

            #[allow(non_snake_case)]
            const fn #compatibility_assertion_ident<Shape, Field>()
            where
                Shape: #compatibility_trait_path<Field>,
            {
            }
            #compatibility_assertion_ident::<#shape_path, #field_type>();
        };
    }
}

/// Emit block-scoped field/shape compatibility assertions.
///
/// Use this when the asserted field type may reference generics from the
/// surrounding item. Nested const items cannot capture those generics.
#[allow(clippy::too_many_arguments)]
pub fn shape_type_assertion_block_tokens_with_suffixes(
    assertion_prefix: &str,
    field_name: &str,
    shape_path: &impl quote::ToTokens,
    field_type: &impl quote::ToTokens,
    span: Span,
    declared_shape_trait_bounds: impl IntoIterator<Item = TokenStream>,
    compatibility_trait_path: impl quote::ToTokens,
    declared_assertion_suffix: &str,
    compatibility_assertion_suffix: &str,
) -> TokenStream {
    let field_fragment = field_assertion_ident_fragment(field_name);
    let declared_assertion_ident = format_ident!(
        "__{assertion_prefix}_assert_{field_fragment}_{declared_assertion_suffix}",
        span = span
    );
    let compatibility_assertion_ident = format_ident!(
        "__{assertion_prefix}_assert_{field_fragment}_{compatibility_assertion_suffix}",
        span = span
    );
    let declared_shape_trait_bounds: Vec<TokenStream> = declared_shape_trait_bounds
        .into_iter()
        .map(|tokens| tokens_with_span(&tokens, span))
        .collect();
    let compatibility_trait_path = tokens_with_span(&compatibility_trait_path, span);

    quote_spanned! {span=>
        {
            #[allow(non_snake_case)]
            fn #declared_assertion_ident<Shape>()
            where
                #(
                    Shape: #declared_shape_trait_bounds,
                )*
            {
            }
            #declared_assertion_ident::<#shape_path>();

            #[allow(non_snake_case)]
            fn #compatibility_assertion_ident<Shape, Field>()
            where
                Shape: #compatibility_trait_path<Field>,
            {
            }
            #compatibility_assertion_ident::<#shape_path, #field_type>();
        }
    }
}
