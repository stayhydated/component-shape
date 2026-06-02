//! Shared code generation helpers for component shape consumers.

pub mod imports;

use heck::ToSnakeCase as _;
use proc_macro2::{Group, Span, TokenStream, TokenTree};
use quote::{format_ident, quote, quote_spanned};
use syn::{Expr, Path, Type, TypePath, parse::ParseStream, spanned::Spanned as _};

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

/// Derive a helper suffix from `field_name` and a shape/component suffix source.
pub fn component_suffix_from_suffix(field_name: &str, suffix: &str) -> Option<String> {
    let mut suffix = suffix.to_snake_case();
    let field_name = field_name.to_snake_case();

    if suffix == field_name {
        return None;
    }

    let field_prefix = format!("{field_name}_");
    if let Some(rest) = suffix.strip_prefix(&field_prefix) {
        suffix = rest.to_string();
    }

    (!suffix.is_empty()).then_some(suffix)
}

/// Extract a suffix source from a path's final segment.
pub fn path_suffix_source(path: &Path) -> Option<String> {
    let ident = path.segments.last()?.ident.to_string();
    let suffix_source = ident
        .strip_suffix("Shape")
        .or_else(|| ident.strip_suffix("State"))
        .unwrap_or(&ident);

    Some(suffix_source.to_string())
}

/// Derive the generated component helper suffix for a field and shape path.
pub fn component_suffix_for_shape(path: &Path, field_name: &str) -> String {
    path_suffix_source(path)
        .and_then(|source| component_suffix_from_suffix(field_name, &source))
        .unwrap_or_else(|| "shape".to_string())
}

/// Normalize shape paths before metadata comparisons and code generation.
pub fn normalize_shape_path(mut path: Path) -> Path {
    for segment in &mut path.segments {
        if let syn::PathArguments::AngleBracketed(args) = &mut segment.arguments {
            args.colon2_token = None;
        }
    }

    path
}

/// Extract a shape path from an expression position.
pub fn shape_path_from_expr(expr: &Expr, expected: &'static str) -> syn::Result<Path> {
    let Expr::Path(expr_path) = expr else {
        return Err(syn::Error::new(expr.span(), expected));
    };

    Ok(normalize_shape_path(expr_path.path.clone()))
}

/// Extract a shape path from a parsed type path.
pub fn shape_path_from_type_path(type_path: TypePath) -> syn::Result<Path> {
    if let Some(qself) = type_path.qself {
        return Err(syn::Error::new(
            qself.lt_token.span,
            "expected a shape path, not a qualified self type",
        ));
    }

    Ok(normalize_shape_path(type_path.path))
}

/// Parse exactly one shape path from a nested syntax stream.
pub fn parse_single_shape_path(
    input: ParseStream<'_>,
    expected: &'static str,
) -> syn::Result<Path> {
    let type_path = input.parse::<TypePath>()?;
    if !input.is_empty() {
        return Err(input.error(expected));
    }

    shape_path_from_type_path(type_path).map_err(|_| input.error(expected))
}

/// Substitute a field type for every `_` occurrence inside a type.
pub fn substitute_infer_in_type(ty: &Type, replacement: &Type) -> Type {
    match ty {
        Type::Infer(_) => replacement.clone(),
        Type::Path(type_path) => {
            let mut type_path = type_path.clone();
            type_path.path = substitute_infer_in_path(&type_path.path, replacement);
            Type::Path(type_path)
        },
        Type::Array(array) => {
            let mut array = array.clone();
            array.elem = Box::new(substitute_infer_in_type(&array.elem, replacement));
            Type::Array(array)
        },
        Type::Slice(slice) => {
            let mut slice = slice.clone();
            slice.elem = Box::new(substitute_infer_in_type(&slice.elem, replacement));
            Type::Slice(slice)
        },
        Type::Ptr(ptr) => {
            let mut ptr = ptr.clone();
            ptr.elem = Box::new(substitute_infer_in_type(&ptr.elem, replacement));
            Type::Ptr(ptr)
        },
        Type::BareFn(bare_fn) => {
            let mut bare_fn = bare_fn.clone();
            for input in &mut bare_fn.inputs {
                input.ty = substitute_infer_in_type(&input.ty, replacement);
            }
            substitute_infer_in_return_type(&mut bare_fn.output, replacement);
            Type::BareFn(bare_fn)
        },
        Type::TraitObject(trait_object) => {
            let mut trait_object = trait_object.clone();
            substitute_infer_in_bounds(&mut trait_object.bounds, replacement);
            Type::TraitObject(trait_object)
        },
        Type::ImplTrait(impl_trait) => {
            let mut impl_trait = impl_trait.clone();
            substitute_infer_in_bounds(&mut impl_trait.bounds, replacement);
            Type::ImplTrait(impl_trait)
        },
        Type::Tuple(tuple) => {
            let mut tuple = tuple.clone();
            tuple.elems = tuple
                .elems
                .iter()
                .map(|ty| substitute_infer_in_type(ty, replacement))
                .collect();
            Type::Tuple(tuple)
        },
        Type::Paren(paren) => {
            let mut paren = paren.clone();
            paren.elem = Box::new(substitute_infer_in_type(&paren.elem, replacement));
            Type::Paren(paren)
        },
        Type::Group(group) => {
            let mut group = group.clone();
            group.elem = Box::new(substitute_infer_in_type(&group.elem, replacement));
            Type::Group(group)
        },
        Type::Reference(reference) => {
            let mut reference = reference.clone();
            *reference.elem = substitute_infer_in_type(&reference.elem, replacement);
            Type::Reference(reference)
        },
        _ => ty.clone(),
    }
}

fn substitute_infer_in_return_type(return_type: &mut syn::ReturnType, replacement: &Type) {
    if let syn::ReturnType::Type(_, ty) = return_type {
        **ty = substitute_infer_in_type(ty, replacement);
    }
}

fn substitute_infer_in_bounds(
    bounds: &mut syn::punctuated::Punctuated<syn::TypeParamBound, syn::Token![+]>,
    replacement: &Type,
) {
    for bound in bounds {
        if let syn::TypeParamBound::Trait(trait_bound) = bound {
            trait_bound.path = substitute_infer_in_path(&trait_bound.path, replacement);
        }
    }
}

/// Substitute a field type for every `_` occurrence inside path arguments.
pub fn substitute_infer_in_path(path: &Path, replacement: &Type) -> Path {
    let mut path = path.clone();

    for segment in &mut path.segments {
        substitute_infer_in_path_arguments(&mut segment.arguments, replacement);
    }

    path
}

fn substitute_infer_in_path_arguments(arguments: &mut syn::PathArguments, replacement: &Type) {
    match arguments {
        syn::PathArguments::AngleBracketed(args) => {
            substitute_infer_in_angle_bracketed_arguments(args, replacement);
        },
        syn::PathArguments::Parenthesized(args) => {
            args.inputs = args
                .inputs
                .iter()
                .map(|ty| substitute_infer_in_type(ty, replacement))
                .collect();
            substitute_infer_in_return_type(&mut args.output, replacement);
        },
        syn::PathArguments::None => {},
    }
}

fn substitute_infer_in_angle_bracketed_arguments(
    args: &mut syn::AngleBracketedGenericArguments,
    replacement: &Type,
) {
    for arg in &mut args.args {
        match arg {
            syn::GenericArgument::Type(ty) => {
                *ty = substitute_infer_in_type(ty, replacement);
            },
            syn::GenericArgument::AssocType(assoc_type) => {
                if let Some(generics) = &mut assoc_type.generics {
                    substitute_infer_in_angle_bracketed_arguments(generics, replacement);
                }
                assoc_type.ty = substitute_infer_in_type(&assoc_type.ty, replacement);
            },
            syn::GenericArgument::Constraint(constraint) => {
                if let Some(generics) = &mut constraint.generics {
                    substitute_infer_in_angle_bracketed_arguments(generics, replacement);
                }
                substitute_infer_in_bounds(&mut constraint.bounds, replacement);
            },
            _ => {},
        }
    }
}

#[derive(Clone, Debug)]
pub struct ShapeOptions {
    pub shape: Path,
    span: Span,
}

impl ShapeOptions {
    pub fn from_shape(shape: Path) -> Self {
        let span = shape.span();
        Self::from_shape_with_span(shape, span)
    }

    pub fn from_shape_with_span(shape: Path, span: Span) -> Self {
        let shape = normalize_shape_path(shape);
        Self { shape, span }
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn resolved_shape(&self, field_type: &Type) -> Path {
        substitute_infer_in_path(&self.shape, field_type)
    }

    pub fn resolve(&self, field_name: String, field_type: Type) -> ResolvedComponentShape {
        let shape = self.resolved_shape(&field_type);
        let component_suffix = component_suffix_for_shape(&shape, &field_name);

        ResolvedComponentShape {
            shape,
            field_name,
            field_type,
            component_suffix,
            span: self.span,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedComponentShape {
    pub shape: Path,
    pub field_name: String,
    pub field_type: Type,
    component_suffix: String,
    span: Span,
}

impl ResolvedComponentShape {
    pub fn shape(&self) -> &Path {
        &self.shape
    }

    pub fn field_name(&self) -> &str {
        &self.field_name
    }

    pub fn field_type(&self) -> &Type {
        &self.field_type
    }

    pub fn component_suffix(&self) -> &str {
        &self.component_suffix
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

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
            const fn #declared_assertion_ident<Shape>()
            where
                #(
                    Shape: #declared_shape_trait_bounds,
                )*
            {
            }
            #declared_assertion_ident::<#shape_path>();

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
            fn #declared_assertion_ident<Shape>()
            where
                #(
                    Shape: #declared_shape_trait_bounds,
                )*
            {
            }
            #declared_assertion_ident::<#shape_path>();

            fn #compatibility_assertion_ident<Shape, Field>()
            where
                Shape: #compatibility_trait_path<Field>,
            {
            }
            #compatibility_assertion_ident::<#shape_path, #field_type>();
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens as _;
    use syn::parse::Parser as _;
    use syn::parse_quote;

    fn compact_type(ty: &Type) -> String {
        ty.to_token_stream()
            .to_string()
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect()
    }

    fn compact_path(path: &Path) -> String {
        path.to_token_stream()
            .to_string()
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect()
    }

    fn compact_tokens(tokens: TokenStream) -> String {
        tokens
            .to_string()
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect()
    }

    #[test]
    fn shape_options_resolve_identifier_shape_and_suffix() {
        let options = ShapeOptions::from_shape(syn::parse_quote!(crate::EmailInputShape<_>));
        let field_type: Type = syn::parse_quote!(String);

        let resolved = options.resolve("email".to_string(), field_type);

        assert_eq!(
            compact_path(resolved.shape()),
            "crate::EmailInputShape<String>"
        );
        assert_eq!(resolved.component_suffix(), "input");
    }

    #[test]
    fn substitutes_infer_in_arrays_slices_pointers_and_bare_fns() {
        let ty: Type = syn::parse_quote! {
            fn([_; 2], &[_], *const _, *mut _) -> Option<_>
        };
        let replacement: Type = syn::parse_quote!(String);

        let substituted = substitute_infer_in_type(&ty, &replacement);

        assert_eq!(
            compact_type(&substituted),
            "fn([String;2],&[String],*constString,*mutString)->Option<String>"
        );
    }

    #[test]
    fn substitutes_infer_in_trait_objects_and_generic_constraints() {
        let ty: Type = syn::parse_quote! {
            dyn crate::Shape<Assoc: Iterator<Item = _>> + FnOnce(_) -> _
        };
        let replacement: Type = syn::parse_quote!(String);

        let substituted = substitute_infer_in_type(&ty, &replacement);

        assert_eq!(
            compact_type(&substituted),
            "dyncrate::Shape<Assoc:Iterator<Item=String>>+FnOnce(String)->String"
        );
    }

    #[test]
    fn rust_metadata_helpers_emit_unchecked_wrapper_construction() {
        let rust_type =
            rust_type_metadata_tokens(quote!(component_shape::RustType), &parse_quote!(String));
        let rust_path = rust_path_metadata_tokens(
            quote!(component_shape::RustPath),
            &parse_quote!(crate::Input),
        );
        let rust_expr = rust_expr_metadata_tokens(
            quote!(component_shape::RustExpr),
            &parse_quote!(Some(Default::default())),
        );
        let optional_rust_expr = optional_rust_expr_metadata_tokens(
            quote!(component_shape::RustExpr),
            Some(&parse_quote!(42)),
        );
        let no_rust_expr =
            optional_rust_expr_metadata_tokens(quote!(component_shape::RustExpr), None);

        assert_eq!(
            compact_tokens(rust_type),
            "component_shape::RustType::from_macro_tokens_unchecked(\"String\")"
        );
        assert_eq!(
            compact_tokens(rust_path),
            "component_shape::RustPath::from_macro_tokens_unchecked(\"crate::Input\")"
        );
        assert_eq!(
            compact_tokens(rust_expr),
            "component_shape::RustExpr::from_macro_tokens_unchecked(\"Some(Default::default())\")"
        );
        assert_eq!(
            compact_tokens(optional_rust_expr),
            "Some(component_shape::RustExpr::from_macro_tokens_unchecked(\"42\"))"
        );
        assert_eq!(compact_tokens(no_rust_expr), "None");
    }

    #[test]
    fn shape_path_from_expr_accepts_path_expressions() {
        let expr: Expr = parse_quote!(crate::Input::<_>);

        let path =
            shape_path_from_expr(&expr, "expected shape path").expect("path expression parses");

        assert_eq!(compact_path(&path), "crate::Input<_>");
    }

    #[test]
    fn shape_path_from_expr_rejects_non_path_expressions() {
        let expr: Expr = parse_quote!(make_shape());

        let error = shape_path_from_expr(&expr, "expected shape path")
            .expect_err("call expression should fail");

        assert_eq!(error.to_string(), "expected shape path");
    }

    #[test]
    fn parse_single_shape_path_rejects_extra_tokens() {
        let parser = |input: syn::parse::ParseStream<'_>| {
            parse_single_shape_path(input, "expected exactly one shape path")
        };

        let error = parser
            .parse2(quote!(crate::Input, value(type = String)))
            .expect_err("extra tokens should fail");

        assert_eq!(error.to_string(), "expected exactly one shape path");
    }

    #[test]
    fn shape_type_assertion_tokens_include_field_specific_bounds() {
        let tokens = shape_type_assertion_tokens(
            "gpui_form",
            "email-address",
            &quote!(EmailInputShape),
            &quote!(String),
            Span::call_site(),
            [
                quote!(component_shape_gpui::DeclaredGpuiComponentShape),
                quote!(component_shape_gpui::GpuiComponentShape),
            ],
            quote!(component_shape_gpui::GpuiComponentShapeFor),
        );

        let tokens = compact_tokens(tokens);
        assert!(tokens.contains("constfn__gpui_form_assert_email_address_declared_shape<Shape>()"));
        assert!(tokens.contains("Shape:component_shape_gpui::DeclaredGpuiComponentShape"));
        assert!(tokens.contains("Shape:component_shape_gpui::GpuiComponentShape"));
        assert!(tokens.contains(
            "constfn__gpui_form_assert_email_address_shape_compatibility<Shape,Field>()"
        ));
        assert!(tokens.contains("Shape:component_shape_gpui::GpuiComponentShapeFor<Field>"));
        assert!(tokens.contains(
            "__gpui_form_assert_email_address_shape_compatibility::<EmailInputShape,String>()"
        ));
    }
}
