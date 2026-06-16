//! Shared code generation helpers for component shape consumers.

pub mod imports;

mod assertions;
mod docs;
mod identifiers;
mod mcp;
mod paths;
mod rust_syntax;
mod shapes;
mod spans;
mod substitute;

use proc_macro2::{Group, Span, TokenStream, TokenTree};
use quote::{format_ident, quote, quote_spanned};
use syn::{
    Expr, GenericArgument, Ident, LitStr, Path, PathArguments, Type, TypePath, parse::ParseStream,
    spanned::Spanned as _,
};

pub use assertions::{
    shape_type_assertion_block_tokens_with_suffixes, shape_type_assertion_tokens,
    shape_type_assertion_tokens_with_suffixes,
};
pub use docs::doc_description;
pub use identifiers::{field_assertion_ident_fragment, validate_shape_field_suffix};
pub use mcp::{
    McpToolMetadataParts, common_inferred_mcp_input_shape_for_types,
    inferred_mcp_input_shape_for_type, is_mcp_input_constructor, mcp_input_constructor_shorthand,
    mcp_input_expr_tokens, mcp_input_shape_tokens, mcp_tool_metadata_tokens,
    validate_mcp_input_expr,
};
pub(crate) use paths::component_shape_expression_parts;
pub use paths::{
    component_suffix_for_shape, component_suffix_from_suffix, normalize_shape_path,
    parse_single_shape_path, path_suffix_source, shape_path_from_constructor_expr,
    shape_path_from_expr, shape_path_from_type_path,
};
pub use rust_syntax::{
    optional_rust_expr_metadata_tokens, rust_expr_metadata_tokens, rust_path_metadata_tokens,
    rust_syntax_metadata_tokens, rust_type_metadata_tokens,
};
pub use shapes::{ComponentShapeConstructor, ResolvedComponentShape, ShapeOptions};
pub use spans::tokens_with_span;
pub use substitute::{
    substitute_infer_in_expr, substitute_infer_in_path, substitute_infer_in_type,
};

#[cfg(test)]
mod tests;
