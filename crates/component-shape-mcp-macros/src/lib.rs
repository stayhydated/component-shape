use proc_macro::TokenStream;
#[cfg(not(test))]
use proc_macro_crate::{FoundCrate, crate_name};
use quote::quote;
use std::collections::BTreeMap;
use syn::{
    Attribute, Data, DeriveInput, Expr, Fields, GenericArgument, Ident, LitBool, LitStr, Path,
    PathArguments, Type, parse_macro_input, parse_quote, spanned::Spanned as _,
};

mod crate_paths;
mod json_schema;
mod options;
mod tool_input;

use crate_paths::*;
use json_schema::*;
use options::*;
use tool_input::*;

/// Derive JSON Schema metadata for structs, transparent newtypes, and
/// fieldless enums used in MCP tool schemas.
#[proc_macro_derive(McpJsonSchema, attributes(mcp, serde))]
pub fn derive_mcp_json_schema(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_mcp_json_schema(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Derive a top-level MCP input schema and strict MCP argument decoding for a
/// named tool input struct.
#[proc_macro_derive(McpToolInput, attributes(mcp, serde))]
pub fn derive_mcp_tool_input(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_mcp_tool_input(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[cfg(test)]
mod tests;
