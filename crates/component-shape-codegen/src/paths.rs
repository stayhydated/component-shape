use super::*;

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

pub(crate) struct ComponentShapeExpressionParts {
    pub(crate) shape: Path,
    pub(crate) configured: bool,
}

pub(crate) fn component_shape_expression_parts(
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
