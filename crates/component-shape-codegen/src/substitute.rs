use super::*;

/// Substitute a field type for every `_` occurrence inside a type.
pub fn substitute_infer_in_type(ty: &Type, replacement: &Type) -> Type {
    attribute_dsl::substitute_infer_in_type(ty, replacement)
}

/// Substitute a field type for every `_` occurrence inside an expression.
///
/// This is primarily useful for configured component shape expressions such as
/// `crate::Select::<_>.searchable(true)`, where the expression must retain
/// expression-position turbofish syntax while its base shape metadata uses a
/// type-position path.
pub fn substitute_infer_in_expr(expr: &Expr, replacement: &Type) -> Expr {
    attribute_dsl::substitute_infer_in_expr(expr, replacement)
}

/// Substitute a field type for every `_` occurrence inside path arguments.
pub fn substitute_infer_in_path(path: &Path, replacement: &Type) -> Path {
    attribute_dsl::substitute_infer_in_path(path, replacement)
}
