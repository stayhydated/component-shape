use super::*;

pub struct McpToolMetadataParts<'a> {
    pub name: Option<&'a str>,
    pub title: Option<&'a str>,
    pub description: Option<&'a str>,
    pub read_only: Option<bool>,
    pub destructive: Option<bool>,
    pub idempotent: Option<bool>,
    pub open_world: Option<bool>,
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
            let constructor = scalar_mcp_input_constructor(kind);
            quote! { #component_shape_crate::McpInput::#constructor() }
        },
        component_shape::McpInputShape::List(kind) => {
            if let Some(constructor) = list_mcp_input_constructor(kind) {
                quote! { #component_shape_crate::McpInput::#constructor() }
            } else {
                quote! {
                    #component_shape_crate::McpInput::list(
                        #component_shape_crate::McpPrimitiveKind::Any
                    )
                }
            }
        },
        component_shape::McpInputShape::Set(kind) => {
            if let Some(constructor) = set_mcp_input_constructor(kind) {
                quote! { #component_shape_crate::McpInput::#constructor() }
            } else {
                quote! {
                    #component_shape_crate::McpInput::set(
                        #component_shape_crate::McpPrimitiveKind::Any
                    )
                }
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

fn scalar_mcp_input_constructor(kind: component_shape::McpPrimitiveKind) -> Ident {
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
    format_ident!("{constructor}")
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
