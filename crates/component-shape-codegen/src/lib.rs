//! Shared code generation helpers for component shape consumers.

pub mod imports;

use proc_macro2::{Group, Span, TokenStream, TokenTree};
use quote::{format_ident, quote, quote_spanned};
use syn::visit_mut::{self, VisitMut as _};
use syn::{
    Expr, GenericArgument, Ident, LitStr, Path, PathArguments, Type, TypePath, parse::ParseStream,
    spanned::Spanned as _,
};

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

/// Optional MCP tool metadata parsed by integration derives.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McpToolMetadataParts<'a> {
    pub name: Option<&'a str>,
    pub title: Option<&'a str>,
    pub description: Option<&'a str>,
    pub read_only: Option<bool>,
    pub destructive: Option<bool>,
    pub idempotent: Option<bool>,
    pub open_world: Option<bool>,
}

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

/// Build `McpToolMetadata` tokens from explicit derive options and rustdoc.
///
/// Explicit descriptions win over rustdoc. Names, titles, and descriptions are
/// validated with the shared component-shape MCP metadata rules so integration
/// derives fail before emitting invalid registration code.
pub fn mcp_tool_metadata_tokens(
    mcp_crate: &Path,
    attrs: &[syn::Attribute],
    metadata: McpToolMetadataParts<'_>,
    span: Span,
) -> syn::Result<TokenStream> {
    if metadata.read_only == Some(true) && metadata.destructive == Some(true) {
        return Err(syn::Error::new(
            span,
            "MCP tool annotation hints cannot be both read-only and destructive",
        ));
    }

    if let Some(name) = metadata.name {
        component_shape::validate_mcp_tool_name(name)
            .map_err(|error| syn::Error::new(span, error.to_string()))?;
    }
    if let Some(title) = metadata.title {
        component_shape::validate_mcp_tool_metadata_text("title", title)
            .map_err(|error| syn::Error::new(span, error.to_string()))?;
    }

    let description = metadata
        .description
        .map(str::to_string)
        .or_else(|| doc_description(attrs));
    if let Some(description) = description.as_deref() {
        component_shape::validate_mcp_tool_metadata_text("description", description)
            .map_err(|error| syn::Error::new(span, error.to_string()))?;
    }

    let mut tokens = quote! {
        #mcp_crate::McpToolMetadata::new()
    };

    if let Some(name) = metadata.name {
        let name = LitStr::new(name, span);
        tokens = quote! { #tokens.with_name(#name) };
    }
    if let Some(title) = metadata.title {
        let title = LitStr::new(title, span);
        tokens = quote! { #tokens.with_title(#title) };
    }
    if let Some(description) = description {
        let description = LitStr::new(&description, span);
        tokens = quote! { #tokens.with_description(#description) };
    }
    if let Some(read_only) = metadata.read_only {
        tokens = quote! { #tokens.with_read_only_hint(#read_only) };
    }
    if let Some(destructive) = metadata.destructive {
        tokens = quote! { #tokens.with_destructive_hint(#destructive) };
    }
    if let Some(idempotent) = metadata.idempotent {
        tokens = quote! { #tokens.with_idempotent_hint(#idempotent) };
    }
    if let Some(open_world) = metadata.open_world {
        tokens = quote! { #tokens.with_open_world_hint(#open_world) };
    }

    Ok(tokens)
}

/// Emit `McpInput` construction tokens for a known metadata shape.
pub fn mcp_input_shape_tokens(
    component_shape_crate: &Path,
    input_shape: component_shape::McpInputShape,
) -> TokenStream {
    match input_shape {
        component_shape::McpInputShape::Unsupported => {
            quote! { #component_shape_crate::McpInput::unsupported() }
        },
        component_shape::McpInputShape::Scalar(kind) => {
            if let Some(constructor) = scalar_mcp_input_constructor(kind) {
                quote! { #component_shape_crate::McpInput::#constructor() }
            } else {
                let kind = primitive_kind_tokens(component_shape_crate, kind);
                quote! { #component_shape_crate::McpInput::scalar(#kind) }
            }
        },
        component_shape::McpInputShape::List(kind) => {
            if let Some(constructor) = list_mcp_input_constructor(kind) {
                quote! { #component_shape_crate::McpInput::#constructor() }
            } else {
                let kind = primitive_kind_tokens(component_shape_crate, kind);
                quote! { #component_shape_crate::McpInput::list(#kind) }
            }
        },
        component_shape::McpInputShape::Set(kind) => {
            if let Some(constructor) = set_mcp_input_constructor(kind) {
                quote! { #component_shape_crate::McpInput::#constructor() }
            } else {
                let kind = primitive_kind_tokens(component_shape_crate, kind);
                quote! { #component_shape_crate::McpInput::set(#kind) }
            }
        },
        component_shape::McpInputShape::Range(kind) => {
            let constructor = range_mcp_input_constructor(kind);
            quote! { #component_shape_crate::McpInput::#constructor() }
        },
        component_shape::McpInputShape::Object => {
            quote! { #component_shape_crate::McpInput::object() }
        },
    }
}

fn primitive_kind_tokens(
    component_shape_crate: &Path,
    kind: component_shape::McpPrimitiveKind,
) -> TokenStream {
    let variant = match kind {
        component_shape::McpPrimitiveKind::Any => quote! { Any },
        component_shape::McpPrimitiveKind::Boolean => quote! { Boolean },
        component_shape::McpPrimitiveKind::Integer => quote! { Integer },
        component_shape::McpPrimitiveKind::Number => quote! { Number },
        component_shape::McpPrimitiveKind::Decimal => quote! { Decimal },
        component_shape::McpPrimitiveKind::String => quote! { String },
        component_shape::McpPrimitiveKind::Date => quote! { Date },
        component_shape::McpPrimitiveKind::DateTime => quote! { DateTime },
    };
    quote! { #component_shape_crate::McpPrimitiveKind::#variant }
}

fn scalar_mcp_input_constructor(kind: component_shape::McpPrimitiveKind) -> Option<Ident> {
    let constructor = match kind {
        component_shape::McpPrimitiveKind::Any => "any",
        component_shape::McpPrimitiveKind::Boolean => "boolean",
        component_shape::McpPrimitiveKind::Integer => "integer",
        component_shape::McpPrimitiveKind::Number => "number",
        component_shape::McpPrimitiveKind::Decimal => "decimal",
        component_shape::McpPrimitiveKind::String => "string",
        component_shape::McpPrimitiveKind::Date => "date",
        component_shape::McpPrimitiveKind::DateTime => "date_time",
    };
    Some(format_ident!("{constructor}"))
}

fn list_mcp_input_constructor(kind: component_shape::McpPrimitiveKind) -> Option<Ident> {
    let constructor = match kind {
        component_shape::McpPrimitiveKind::Boolean => "boolean_list",
        component_shape::McpPrimitiveKind::Integer => "integer_list",
        component_shape::McpPrimitiveKind::Number => "number_list",
        component_shape::McpPrimitiveKind::Decimal => "decimal_list",
        component_shape::McpPrimitiveKind::String => "string_list",
        component_shape::McpPrimitiveKind::Date => "date_list",
        component_shape::McpPrimitiveKind::DateTime => "date_time_list",
        component_shape::McpPrimitiveKind::Any => return None,
    };
    Some(format_ident!("{constructor}"))
}

fn set_mcp_input_constructor(kind: component_shape::McpPrimitiveKind) -> Option<Ident> {
    let constructor = match kind {
        component_shape::McpPrimitiveKind::Boolean => "boolean_set",
        component_shape::McpPrimitiveKind::Integer => "integer_set",
        component_shape::McpPrimitiveKind::Number => "number_set",
        component_shape::McpPrimitiveKind::Decimal => "decimal_set",
        component_shape::McpPrimitiveKind::String => "string_set",
        component_shape::McpPrimitiveKind::Date => "date_set",
        component_shape::McpPrimitiveKind::DateTime => "date_time_set",
        component_shape::McpPrimitiveKind::Any => return None,
    };
    Some(format_ident!("{constructor}"))
}

fn range_mcp_input_constructor(kind: component_shape::McpRangeBoundKind) -> Ident {
    let constructor = match kind {
        component_shape::McpRangeBoundKind::Integer => "integer_range",
        component_shape::McpRangeBoundKind::Number => "number_range",
        component_shape::McpRangeBoundKind::Decimal => "decimal_range",
        component_shape::McpRangeBoundKind::Date => "date_range",
        component_shape::McpRangeBoundKind::DateTime => "date_time_range",
    };
    format_ident!("{constructor}")
}

/// Return the `McpInput` constructor shorthand for a bare expression.
pub fn mcp_input_constructor_shorthand(expr: &Expr) -> Option<Ident> {
    match expr {
        Expr::Path(path) => mcp_input_path_constructor(path),
        Expr::Call(call) if call.args.is_empty() => {
            let Expr::Path(path) = call.func.as_ref() else {
                return None;
            };
            mcp_input_path_constructor(path)
        },
        _ => None,
    }
}

fn mcp_input_path_constructor(path: &syn::ExprPath) -> Option<Ident> {
    if !path.attrs.is_empty() || path.qself.is_some() || path.path.segments.len() != 1 {
        return None;
    }
    let segment = path.path.segments.first()?;
    if !matches!(segment.arguments, PathArguments::None) {
        return None;
    }
    let ident = segment.ident.to_string();
    if !is_mcp_input_constructor(&ident) {
        return None;
    }

    Some(segment.ident.clone())
}

/// Validate shorthand-only `mcp_input = ...` errors before macro expansion.
pub fn validate_mcp_input_expr(expr: &Expr) -> syn::Result<()> {
    match expr {
        Expr::Path(path) if is_single_segment_path(path) => {
            if mcp_input_path_constructor(path).is_some() {
                return Ok(());
            }
            let ident = &path
                .path
                .segments
                .first()
                .expect("path segment should exist")
                .ident;
            Err(syn::Error::new_spanned(
                ident,
                format!(
                    "unknown `mcp_input` shorthand `{ident}`; use a supported McpInput constructor name or an explicit `McpInput` expression",
                ),
            ))
        },
        Expr::Call(call) => {
            let Expr::Path(path) = call.func.as_ref() else {
                return Ok(());
            };
            if is_single_segment_path(path)
                && mcp_input_path_constructor(path).is_some()
                && !call.args.is_empty()
            {
                return Err(syn::Error::new_spanned(
                    call,
                    "`mcp_input` constructor shorthand does not accept arguments",
                ));
            }
            Ok(())
        },
        _ => Ok(()),
    }
}

fn is_single_segment_path(path: &syn::ExprPath) -> bool {
    path.attrs.is_empty()
        && path.qself.is_none()
        && path.path.segments.len() == 1
        && path
            .path
            .segments
            .first()
            .is_some_and(|segment| matches!(segment.arguments, PathArguments::None))
}

/// Whether `ident` names an `McpInput` constructor shorthand supported in macros.
pub fn is_mcp_input_constructor(ident: &str) -> bool {
    matches!(
        ident,
        "unsupported"
            | "any"
            | "boolean"
            | "integer"
            | "number"
            | "decimal"
            | "string"
            | "date"
            | "date_time"
            | "string_list"
            | "boolean_list"
            | "integer_list"
            | "number_list"
            | "decimal_list"
            | "date_list"
            | "date_time_list"
            | "string_set"
            | "boolean_set"
            | "integer_set"
            | "number_set"
            | "decimal_set"
            | "date_set"
            | "date_time_set"
            | "decimal_range"
            | "integer_range"
            | "number_range"
            | "date_range"
            | "date_time_range"
            | "object"
    )
}

/// Emit `McpInput` construction tokens from either shorthand or an explicit expression.
pub fn mcp_input_expr_tokens(component_shape_crate: &Path, expr: &Expr) -> TokenStream {
    if let Some(constructor) = mcp_input_constructor_shorthand(expr) {
        return quote! { #component_shape_crate::McpInput::#constructor() };
    }

    quote! { #expr }
}

/// Infer coarse MCP input metadata from a Rust value type.
pub fn inferred_mcp_input_shape_for_type(ty: &Type) -> Option<component_shape::McpInputShape> {
    match peel_type_wrappers(ty) {
        Type::Path(path) if path.qself.is_none() => inferred_mcp_input_shape_for_path(path),
        Type::Array(array) => {
            primitive_kind_for_type(&array.elem).map(component_shape::McpInputShape::List)
        },
        Type::Slice(slice) => {
            primitive_kind_for_type(&slice.elem).map(component_shape::McpInputShape::List)
        },
        Type::Tuple(tuple) if tuple.elems.len() == 2 => {
            let mut elems = tuple.elems.iter();
            let first = range_bound_kind(elems.next()?)?;
            let second = range_bound_kind(elems.next()?)?;
            if first != second {
                return None;
            }
            Some(component_shape::McpInputShape::Range(first))
        },
        _ => None,
    }
}

/// Infer a single shared MCP input shape when every type infers the same shape.
pub fn common_inferred_mcp_input_shape_for_types<'a>(
    values: impl IntoIterator<Item = &'a Type>,
) -> Option<component_shape::McpInputShape> {
    let mut inferred = None;
    for value in values {
        let value_input = inferred_mcp_input_shape_for_type(value)?;
        match inferred {
            Some(existing) if existing != value_input => return None,
            Some(_) => {},
            None => inferred = Some(value_input),
        }
    }

    inferred
}

fn inferred_mcp_input_shape_for_path(
    path: &syn::TypePath,
) -> Option<component_shape::McpInputShape> {
    let segment = path.path.segments.last()?;
    let ident = segment.ident.to_string();

    if ident == "Option" {
        return single_type_argument(&segment.arguments)
            .and_then(inferred_mcp_input_shape_for_type);
    }

    if is_transparent_value_wrapper_ident(&ident) {
        let item = single_type_argument(&segment.arguments)?;
        return inferred_mcp_input_shape_for_type(item);
    }

    if ident == "Cow" {
        let item = last_type_argument(&segment.arguments)?;
        return inferred_mcp_input_shape_for_type(item);
    }

    if is_list_wrapper_ident(&ident) {
        let item = single_type_argument(&segment.arguments)?;
        return primitive_kind_for_type(item).map(component_shape::McpInputShape::List);
    }

    if is_set_wrapper_ident(&ident) {
        let item = single_type_argument(&segment.arguments)?;
        return primitive_kind_for_type(item).map(component_shape::McpInputShape::Set);
    }

    if is_string_keyed_object_wrapper_ident(&ident)
        && first_type_argument(&segment.arguments).is_some_and(is_string_key_type)
    {
        return Some(component_shape::McpInputShape::Object);
    }

    if ident == "McpRange" {
        let item = single_type_argument(&segment.arguments)?;
        return primitive_kind_for_type(item)
            .and_then(range_bound_for_primitive)
            .map(component_shape::McpInputShape::Range);
    }

    primitive_kind_for_path_ident(&ident).map(component_shape::McpInputShape::Scalar)
}

fn is_transparent_value_wrapper_ident(ident: &str) -> bool {
    matches!(ident, "Box" | "Rc" | "Arc")
}

fn is_list_wrapper_ident(ident: &str) -> bool {
    matches!(ident, "Vec" | "VecDeque" | "LinkedList")
}

fn is_set_wrapper_ident(ident: &str) -> bool {
    matches!(ident, "BTreeSet" | "HashSet" | "IndexSet")
}

fn is_string_keyed_object_wrapper_ident(ident: &str) -> bool {
    matches!(ident, "BTreeMap" | "HashMap" | "IndexMap" | "Map")
}

fn is_string_key_type(ty: &Type) -> bool {
    match peel_type_wrappers(ty) {
        Type::Path(path) if path.qself.is_none() => {
            let segment = match path.path.segments.last() {
                Some(segment) => segment,
                None => return false,
            };
            let ident = segment.ident.to_string();
            if ident == "String" || ident == "str" {
                return true;
            }
            if is_transparent_value_wrapper_ident(&ident) {
                return single_type_argument(&segment.arguments).is_some_and(is_string_key_type);
            }
            if ident == "Cow" {
                return last_type_argument(&segment.arguments).is_some_and(is_string_key_type);
            }
            false
        },
        _ => false,
    }
}

fn primitive_kind_for_type(ty: &Type) -> Option<component_shape::McpPrimitiveKind> {
    match peel_type_wrappers(ty) {
        Type::Path(path) if path.qself.is_none() => {
            let segment = path.path.segments.last()?;
            if segment.ident == "Option" {
                return None;
            }
            let ident = segment.ident.to_string();
            if is_transparent_value_wrapper_ident(&ident) {
                let item = single_type_argument(&segment.arguments)?;
                return primitive_kind_for_type(item);
            }
            if ident == "Cow" {
                let item = last_type_argument(&segment.arguments)?;
                return primitive_kind_for_type(item);
            }
            primitive_kind_for_path_ident(&ident)
        },
        _ => None,
    }
}

fn primitive_kind_for_path_ident(ident: &str) -> Option<component_shape::McpPrimitiveKind> {
    match ident {
        "bool" => Some(component_shape::McpPrimitiveKind::Boolean),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128"
        | "usize" => Some(component_shape::McpPrimitiveKind::Integer),
        "f32" | "f64" => Some(component_shape::McpPrimitiveKind::Number),
        "Decimal" => Some(component_shape::McpPrimitiveKind::Decimal),
        "String" | "str" | "Path" | "PathBuf" | "OsString" | "char" => {
            Some(component_shape::McpPrimitiveKind::String)
        },
        "NaiveDate" | "Date" => Some(component_shape::McpPrimitiveKind::Date),
        "NaiveDateTime" | "DateTime" | "OffsetDateTime" | "Timestamp" | "Zoned" => {
            Some(component_shape::McpPrimitiveKind::DateTime)
        },
        "McpAny" => Some(component_shape::McpPrimitiveKind::Any),
        _ => None,
    }
}

fn range_bound_for_primitive(
    kind: component_shape::McpPrimitiveKind,
) -> Option<component_shape::McpRangeBoundKind> {
    match kind {
        component_shape::McpPrimitiveKind::Integer => {
            Some(component_shape::McpRangeBoundKind::Integer)
        },
        component_shape::McpPrimitiveKind::Number => {
            Some(component_shape::McpRangeBoundKind::Number)
        },
        component_shape::McpPrimitiveKind::Decimal => {
            Some(component_shape::McpRangeBoundKind::Decimal)
        },
        component_shape::McpPrimitiveKind::Date => Some(component_shape::McpRangeBoundKind::Date),
        component_shape::McpPrimitiveKind::DateTime => {
            Some(component_shape::McpRangeBoundKind::DateTime)
        },
        component_shape::McpPrimitiveKind::Any
        | component_shape::McpPrimitiveKind::Boolean
        | component_shape::McpPrimitiveKind::String => None,
    }
}

fn range_bound_kind(ty: &Type) -> Option<component_shape::McpRangeBoundKind> {
    let Type::Path(path) = peel_type_wrappers(ty) else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }

    let segment = path.path.segments.last()?;
    if segment.ident != "Option" {
        return None;
    }

    single_type_argument(&segment.arguments)
        .and_then(primitive_kind_for_type)
        .and_then(range_bound_for_primitive)
}

fn single_type_argument(arguments: &PathArguments) -> Option<&Type> {
    let PathArguments::AngleBracketed(arguments) = arguments else {
        return None;
    };
    let mut arguments = arguments.args.iter();
    let GenericArgument::Type(ty) = arguments.next()? else {
        return None;
    };
    if arguments.next().is_some() {
        return None;
    }
    Some(ty)
}

fn first_type_argument(arguments: &PathArguments) -> Option<&Type> {
    let PathArguments::AngleBracketed(arguments) = arguments else {
        return None;
    };
    arguments.args.iter().find_map(|argument| match argument {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}

fn last_type_argument(arguments: &PathArguments) -> Option<&Type> {
    let PathArguments::AngleBracketed(arguments) = arguments else {
        return None;
    };
    arguments
        .args
        .iter()
        .rev()
        .find_map(|argument| match argument {
            GenericArgument::Type(ty) => Some(ty),
            _ => None,
        })
}

fn peel_type_wrappers(mut ty: &Type) -> &Type {
    loop {
        match ty {
            Type::Group(group) => ty = &group.elem,
            Type::Paren(paren) => ty = &paren.elem,
            Type::Reference(reference) => ty = &reference.elem,
            _ => return ty,
        }
    }
}

/// Derive a helper suffix from `field_name` and a shape/component suffix source.
pub fn component_suffix_from_suffix(field_name: &str, suffix: &str) -> Option<String> {
    component_shape::component_suffix_from_suffix(field_name, suffix)
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

/// Extract a shape path from a path or configured constructor expression.
///
/// A plain path, such as `crate::Input::<_>`, is treated as the component shape
/// itself. An associated function call, such as
/// `crate::Select::<_>::searchable(true)`, is treated as a configured
/// constructor expression for the base shape `crate::Select<_>`.
pub fn shape_path_from_constructor_expr(expr: &Expr, expected: &'static str) -> syn::Result<Path> {
    component_shape_expression_parts(expr, expected).map(|parts| parts.shape)
}

struct ComponentShapeExpressionParts {
    shape: Path,
    configured: bool,
}

fn component_shape_expression_parts(
    expr: &Expr,
    expected: &'static str,
) -> syn::Result<ComponentShapeExpressionParts> {
    match expr {
        Expr::Path(expr_path) => Ok(ComponentShapeExpressionParts {
            shape: normalize_shape_path(expr_path.path.clone()),
            configured: false,
        }),
        Expr::Call(call) => {
            let Expr::Path(func) = &*call.func else {
                return Err(syn::Error::new(call.func.span(), expected));
            };

            Ok(ComponentShapeExpressionParts {
                shape: shape_path_from_associated_constructor(&func.path, expected)?,
                configured: true,
            })
        },
        Expr::Group(group) => component_shape_expression_parts(&group.expr, expected),
        Expr::Paren(paren) => component_shape_expression_parts(&paren.expr, expected),
        _ => Err(syn::Error::new(expr.span(), expected)),
    }
}

fn shape_path_from_associated_constructor(
    func_path: &Path,
    expected: &'static str,
) -> syn::Result<Path> {
    let mut shape = func_path.clone();
    let Some(associated_fn) = shape.segments.pop() else {
        return Err(syn::Error::new(func_path.span(), expected));
    };
    shape.segments.pop_punct();

    if shape.segments.is_empty() {
        return Err(syn::Error::new_spanned(
            associated_fn.into_value(),
            expected,
        ));
    }

    Ok(normalize_shape_path(shape))
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

/// Substitute a field type for every `_` occurrence inside an expression.
///
/// This is primarily useful for configured component shape expressions such as
/// `crate::Select::<_>::searchable(true)`, where the expression must retain
/// expression-position turbofish syntax while its base shape metadata uses a
/// type-position path.
pub fn substitute_infer_in_expr(expr: &Expr, replacement: &Type) -> Expr {
    let mut expr = expr.clone();
    InferSubstitutor { replacement }.visit_expr_mut(&mut expr);
    expr
}

struct InferSubstitutor<'a> {
    replacement: &'a Type,
}

impl visit_mut::VisitMut for InferSubstitutor<'_> {
    fn visit_type_mut(&mut self, node: &mut Type) {
        *node = substitute_infer_in_type(node, self.replacement);
    }

    fn visit_path_mut(&mut self, node: &mut Path) {
        *node = substitute_infer_in_path(node, self.replacement);
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
    constructor: ComponentShapeConstructor,
    span: Span,
}

/// Optional configured construction expression attached to a component shape.
#[derive(Clone, Debug)]
pub enum ComponentShapeConstructor {
    /// Construct the shape with the consumer's normal/default constructor.
    Default,
    /// Construct the shape with a user-supplied expression, such as
    /// `Select::<_>::searchable(true)`.
    Expr(Expr),
}

impl ComponentShapeConstructor {
    pub fn expr(&self) -> Option<&Expr> {
        match self {
            Self::Default => None,
            Self::Expr(expr) => Some(expr),
        }
    }

    fn resolved(&self, field_type: &Type) -> Self {
        match self {
            Self::Default => Self::Default,
            Self::Expr(expr) => Self::Expr(substitute_infer_in_expr(expr, field_type)),
        }
    }
}

impl ShapeOptions {
    pub fn from_shape(shape: Path) -> Self {
        let span = shape.span();
        Self::from_shape_with_span(shape, span)
    }

    pub fn from_shape_with_span(shape: Path, span: Span) -> Self {
        let shape = normalize_shape_path(shape);
        Self {
            shape,
            constructor: ComponentShapeConstructor::Default,
            span,
        }
    }

    /// Build shape options from either a plain shape path expression or a
    /// configured constructor expression.
    pub fn from_constructor_expr(expr: Expr, expected: &'static str) -> syn::Result<Self> {
        let span = expr.span();
        Self::from_constructor_expr_with_span(expr, span, expected)
    }

    /// Build shape options from either a plain shape path expression or a
    /// configured constructor expression, using `span` for later diagnostics.
    pub fn from_constructor_expr_with_span(
        expr: Expr,
        span: Span,
        expected: &'static str,
    ) -> syn::Result<Self> {
        let parts = component_shape_expression_parts(&expr, expected)?;
        let constructor = if parts.configured {
            ComponentShapeConstructor::Expr(expr)
        } else {
            ComponentShapeConstructor::Default
        };

        Ok(Self {
            shape: parts.shape,
            constructor,
            span,
        })
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn resolved_shape(&self, field_type: &Type) -> Path {
        substitute_infer_in_path(&self.shape, field_type)
    }

    pub fn resolve(&self, field_name: String, field_type: Type) -> ResolvedComponentShape {
        let shape = self.resolved_shape(&field_type);
        let constructor = self.constructor.resolved(&field_type);
        let component_suffix = component_suffix_for_shape(&shape, &field_name);

        ResolvedComponentShape {
            shape,
            constructor,
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
    constructor: ComponentShapeConstructor,
    pub field_name: String,
    pub field_type: Type,
    component_suffix: String,
    span: Span,
}

impl ResolvedComponentShape {
    pub fn shape(&self) -> &Path {
        &self.shape
    }

    pub fn constructor(&self) -> &ComponentShapeConstructor {
        &self.constructor
    }

    pub fn constructor_expr(&self) -> Option<&Expr> {
        self.constructor.expr()
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
    fn shape_options_resolve_plain_constructor_expr_as_default_shape() {
        let expr: Expr = parse_quote!(crate::Input::<_>);
        let options = ShapeOptions::from_constructor_expr(expr, "expected component shape")
            .expect("plain shape expression should parse");
        let field_type: Type = syn::parse_quote!(crate::types::AccountCode);

        let resolved = options.resolve("account".to_string(), field_type);

        assert_eq!(
            compact_path(resolved.shape()),
            "crate::Input<crate::types::AccountCode>"
        );
        assert!(resolved.constructor_expr().is_none());
    }

    #[test]
    fn shape_options_resolve_associated_constructor_expr() {
        let expr: Expr = parse_quote!(crate::select::Select::<_>::searchable(true));
        let options = ShapeOptions::from_constructor_expr(expr, "expected component shape")
            .expect("configured shape expression should parse");
        let field_type: Type = syn::parse_quote!(String);

        let resolved = options.resolve("country".to_string(), field_type);

        assert_eq!(
            compact_path(resolved.shape()),
            "crate::select::Select<String>"
        );
        assert_eq!(resolved.component_suffix(), "select");
        assert_eq!(
            compact_tokens(
                resolved
                    .constructor_expr()
                    .expect("configured constructor should be retained")
                    .to_token_stream()
            ),
            "crate::select::Select::<String>::searchable(true)"
        );
    }

    #[test]
    fn shape_options_resolve_from_constructor_expr() {
        let expr: Expr = parse_quote!(crate::select::Select::<_>::from(
            crate::select::SelectArgs::builder()
                .searchable(true)
                .placeholder("Country")
                .build()
        ));
        let options = ShapeOptions::from_constructor_expr(expr, "expected component shape")
            .expect("configured shape expression should parse");
        let field_type: Type = syn::parse_quote!(String);

        let resolved = options.resolve("country".to_string(), field_type);

        assert_eq!(
            compact_path(resolved.shape()),
            "crate::select::Select<String>"
        );
        assert_eq!(
            compact_tokens(
                resolved
                    .constructor_expr()
                    .expect("configured constructor should be retained")
                    .to_token_stream()
            ),
            "crate::select::Select::<String>::from(crate::select::SelectArgs::builder().searchable(true).placeholder(\"Country\").build())"
        );
    }

    #[test]
    fn shape_path_from_constructor_expr_rejects_method_chains() {
        let expr: Expr = parse_quote!(crate::select::Select::<_>::args().searchable(true));

        let error = shape_path_from_constructor_expr(&expr, "expected component shape")
            .expect_err("method chain should fail");

        assert_eq!(error.to_string(), "expected component shape");
    }

    #[test]
    fn shape_path_from_constructor_expr_rejects_unanchored_call() {
        let expr: Expr = parse_quote!(searchable(true));

        let error = shape_path_from_constructor_expr(&expr, "expected component shape")
            .expect_err("unanchored call should fail");

        assert_eq!(error.to_string(), "expected component shape");
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

    #[test]
    fn doc_description_trims_outer_blank_doc_lines() {
        let input: syn::ItemStruct = syn::parse_quote! {
            ///
            /// Search arguments.
            ///
            /// Includes pagination.
            ///
            struct Search;
        };

        assert_eq!(
            doc_description(&input.attrs),
            Some("Search arguments.\n\nIncludes pagination.".to_string())
        );
    }

    #[test]
    fn mcp_tool_metadata_tokens_prefers_explicit_description_over_docs() {
        let input: syn::ItemStruct = syn::parse_quote! {
            /// Inferred description.
            struct Search;
        };
        let mcp_crate: Path = syn::parse_quote!(gpui_form::mcp);

        let tokens = mcp_tool_metadata_tokens(
            &mcp_crate,
            &input.attrs,
            McpToolMetadataParts {
                name: Some("search"),
                title: Some("Search"),
                description: Some("Explicit description."),
                read_only: Some(true),
                destructive: Some(false),
                idempotent: Some(true),
                open_world: Some(false),
            },
            Span::call_site(),
        )
        .expect("metadata should be valid");
        let compact = compact_tokens(tokens);

        assert!(compact.contains("gpui_form::mcp::McpToolMetadata::new()"));
        assert!(compact.contains(".with_name(\"search\")"));
        assert!(compact.contains(".with_title(\"Search\")"));
        assert!(compact.contains(".with_description(\"Explicitdescription.\")"));
        assert!(compact.contains(".with_read_only_hint(true)"));
        assert!(compact.contains(".with_destructive_hint(false)"));
        assert!(compact.contains(".with_idempotent_hint(true)"));
        assert!(compact.contains(".with_open_world_hint(false)"));
        assert!(!compact.contains("Inferreddescription."));
    }

    #[test]
    fn mcp_tool_metadata_tokens_validates_metadata() {
        let mcp_crate: Path = syn::parse_quote!(gpui_form::mcp);
        let error = mcp_tool_metadata_tokens(
            &mcp_crate,
            &[],
            McpToolMetadataParts {
                name: Some("bad name"),
                title: None,
                description: None,
                read_only: None,
                destructive: None,
                idempotent: None,
                open_world: None,
            },
            Span::call_site(),
        )
        .expect_err("invalid name should fail");

        assert!(error.to_string().contains("tool name"));

        let error = mcp_tool_metadata_tokens(
            &mcp_crate,
            &[],
            McpToolMetadataParts {
                name: None,
                title: None,
                description: None,
                read_only: Some(true),
                destructive: Some(true),
                idempotent: None,
                open_world: None,
            },
            Span::call_site(),
        )
        .expect_err("conflicting annotation hints should fail");

        assert!(
            error
                .to_string()
                .contains("cannot be both read-only and destructive"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn inferred_mcp_input_shape_handles_scalar_wrappers() {
        let string_ty: Type = parse_quote!(std::borrow::Cow<'static, str>);
        let boxed_date_ty: Type = parse_quote!(Box<chrono::NaiveDate>);
        let optional_decimal_ty: Type = parse_quote!(Option<std::sync::Arc<rust_decimal::Decimal>>);

        assert_eq!(
            inferred_mcp_input_shape_for_type(&string_ty),
            Some(component_shape::McpInputShape::Scalar(
                component_shape::McpPrimitiveKind::String
            ))
        );
        assert_eq!(
            inferred_mcp_input_shape_for_type(&boxed_date_ty),
            Some(component_shape::McpInputShape::Scalar(
                component_shape::McpPrimitiveKind::Date
            ))
        );
        assert_eq!(
            inferred_mcp_input_shape_for_type(&optional_decimal_ty),
            Some(component_shape::McpInputShape::Scalar(
                component_shape::McpPrimitiveKind::Decimal
            ))
        );
    }

    #[test]
    fn inferred_mcp_input_shape_handles_collections_and_ranges() {
        let vec_ty: Type = parse_quote!(std::collections::VecDeque<String>);
        let wrapped_vec_ty: Type = parse_quote!(Vec<Box<String>>);
        let set_ty: Type = parse_quote!(std::collections::BTreeSet<chrono::NaiveDate>);
        let map_ty: Type = parse_quote!(std::collections::HashMap<String, gpui_form::mcp::McpAny>);
        let json_map_ty: Type = parse_quote!(serde_json::Map<String, gpui_form::mcp::McpAny>);
        let any_value_ty: Type = parse_quote!(gpui_form::mcp::McpAny);
        let array_ty: Type = parse_quote!([u64; 3]);
        let slice_ty: Type = parse_quote!(&[bool]);
        let tuple_range_ty: Type =
            parse_quote!((Option<chrono::NaiveDate>, Option<chrono::NaiveDate>));
        let mcp_range_ty: Type = parse_quote!(gpui_form::mcp::McpRange<rust_decimal::Decimal>);

        assert_eq!(
            inferred_mcp_input_shape_for_type(&vec_ty),
            Some(component_shape::McpInputShape::List(
                component_shape::McpPrimitiveKind::String
            ))
        );
        assert_eq!(
            inferred_mcp_input_shape_for_type(&wrapped_vec_ty),
            Some(component_shape::McpInputShape::List(
                component_shape::McpPrimitiveKind::String
            ))
        );
        assert_eq!(
            inferred_mcp_input_shape_for_type(&set_ty),
            Some(component_shape::McpInputShape::Set(
                component_shape::McpPrimitiveKind::Date
            ))
        );
        assert_eq!(
            inferred_mcp_input_shape_for_type(&map_ty),
            Some(component_shape::McpInputShape::Object)
        );
        assert_eq!(
            inferred_mcp_input_shape_for_type(&json_map_ty),
            Some(component_shape::McpInputShape::Object)
        );
        assert_eq!(
            inferred_mcp_input_shape_for_type(&any_value_ty),
            Some(component_shape::McpInputShape::Scalar(
                component_shape::McpPrimitiveKind::Any
            ))
        );
        assert_eq!(
            inferred_mcp_input_shape_for_type(&array_ty),
            Some(component_shape::McpInputShape::List(
                component_shape::McpPrimitiveKind::Integer
            ))
        );
        assert_eq!(
            inferred_mcp_input_shape_for_type(&slice_ty),
            Some(component_shape::McpInputShape::List(
                component_shape::McpPrimitiveKind::Boolean
            ))
        );
        assert_eq!(
            inferred_mcp_input_shape_for_type(&tuple_range_ty),
            Some(component_shape::McpInputShape::Range(
                component_shape::McpRangeBoundKind::Date
            ))
        );
        assert_eq!(
            inferred_mcp_input_shape_for_type(&mcp_range_ty),
            Some(component_shape::McpInputShape::Range(
                component_shape::McpRangeBoundKind::Decimal
            ))
        );
    }

    #[test]
    fn common_inferred_mcp_input_shape_requires_agreement() {
        let string_ty: Type = parse_quote!(String);
        let string_ref_ty: Type = parse_quote!(&str);
        let number_ty: Type = parse_quote!(u64);

        assert_eq!(
            common_inferred_mcp_input_shape_for_types([&string_ty, &string_ref_ty]),
            Some(component_shape::McpInputShape::Scalar(
                component_shape::McpPrimitiveKind::String
            ))
        );
        assert_eq!(
            common_inferred_mcp_input_shape_for_types([&string_ty, &number_ty]),
            None
        );
    }

    #[test]
    fn mcp_input_expr_tokens_support_constructor_shorthand() {
        let component_shape_crate: Path = parse_quote!(component_shape_gpui);
        let shorthand: Expr = parse_quote!(string_list);
        let explicit: Expr = parse_quote!(component_shape_gpui::McpInput::object());
        let invalid: Expr = parse_quote!(strings);

        assert_eq!(
            compact_tokens(mcp_input_expr_tokens(&component_shape_crate, &shorthand)),
            "component_shape_gpui::McpInput::string_list()"
        );
        assert_eq!(
            compact_tokens(mcp_input_expr_tokens(&component_shape_crate, &explicit)),
            "component_shape_gpui::McpInput::object()"
        );
        assert!(
            validate_mcp_input_expr(&invalid)
                .expect_err("unknown shorthand should fail")
                .to_string()
                .contains("unknown `mcp_input` shorthand `strings`")
        );
    }
}
