use super::*;
use quote::ToTokens as _;

#[test]
fn derive_schema_uses_field_attributes() {
    let input: DeriveInput = syn::parse_quote! {
        struct SearchArgs {
            #[mcp(rename = "q", description = "Search text")]
            query: String,
            page: Option<u32>,
            #[serde(skip)]
            internal: String,
        }
    };

    let expanded = expand_mcp_json_schema(input)
        .expect("schema derive should expand")
        .to_token_stream()
        .to_string();

    assert!(expanded.contains("\"q\""));
    assert!(expanded.contains("Search text"));
    assert!(expanded.contains("x-mcpDecodeName"));
    assert!(expanded.contains("page"));
    assert!(!expanded.contains("internal"));
}

#[test]
fn derive_schema_infers_doc_descriptions() {
    let input: DeriveInput = syn::parse_quote! {
        /// Search arguments sent to the tool.
        ///
        /// Blank doc lines are preserved between paragraphs.
        struct SearchArgs {
            /// Full text query.
            query: String,
        }
    };

    let expanded = expand_mcp_json_schema(input)
        .expect("schema derive should expand")
        .to_token_stream()
        .to_string();

    assert!(expanded.contains("Search arguments sent to the tool."));
    assert!(expanded.contains("Blank doc lines are preserved between paragraphs."));
    assert!(expanded.contains("Full text query."));
}

#[test]
fn derive_schema_supports_tuple_newtypes() {
    let input: DeriveInput = syn::parse_quote! {
        struct UserId(u64);
    };

    let expanded = expand_mcp_json_schema(input)
        .expect("newtype schema derive should expand")
        .to_token_stream()
        .to_string();

    assert!(expanded.contains("< u64 as"));
    assert!(expanded.contains("json_schema"));
}

#[test]
fn derive_schema_supports_named_transparent_newtypes() {
    let input: DeriveInput = syn::parse_quote! {
        #[serde(transparent)]
        struct UserId {
            value: u64,
        }
    };

    let expanded = expand_mcp_json_schema(input)
        .expect("transparent named newtype schema derive should expand")
        .to_token_stream()
        .to_string();

    assert!(expanded.contains("< u64 as"));
    assert!(expanded.contains("json_schema"));
    assert!(!expanded.contains("object_schema"));
}

#[test]
fn derive_schema_rejects_multi_field_transparent_structs() {
    let input: DeriveInput = syn::parse_quote! {
        #[mcp(transparent)]
        struct Bad {
            min: u32,
            max: u32,
        }
    };

    let error = expand_mcp_json_schema(input).unwrap_err().to_string();

    assert!(error.contains("transparent structs must have exactly one field"));
}

#[test]
fn derive_schema_accepts_crate_override() {
    let input: DeriveInput = syn::parse_quote! {
        #[mcp(crate = gpui_form::mcp)]
        struct UserId(u64);
    };

    let expanded = expand_mcp_json_schema(input)
        .expect("crate path override should expand")
        .to_token_stream()
        .to_string();

    assert!(expanded.contains("gpui_form :: mcp :: McpJsonSchema"));
}

#[test]
fn multiple_facade_error_lists_available_overrides() {
    let message = multiple_facade_error_message(&[
        ("gpui-form", parse_quote!(::gpui_form::mcp)),
        ("gpui-table", parse_quote!(::gpui_table::mcp)),
    ]);

    assert!(message.contains("`gpui-form`, `gpui-table`"));
    assert!(message.contains("#[mcp(crate = gpui_form::mcp)]"));
    assert!(message.contains("#[mcp(crate = gpui_table::mcp)]"));
}

#[test]
fn derive_schema_rejects_legacy_crate_path_option() {
    let input: DeriveInput = syn::parse_quote! {
        #[mcp(crate_path = gpui_form::mcp)]
        struct UserId(u64);
    };

    let error = expand_mcp_json_schema(input).unwrap_err().to_string();

    assert!(error.contains("unknown `mcp` container option"));
}

#[test]
fn derive_schema_rejects_field_name_alias() {
    let input: DeriveInput = syn::parse_quote! {
        struct SearchArgs {
            #[mcp(name = "q")]
            query: String,
        }
    };

    let error = expand_mcp_json_schema(input).unwrap_err().to_string();

    assert!(error.contains("unknown `mcp` field option"));
}

#[test]
fn derive_schema_rejects_duplicate_field_names() {
    let input: DeriveInput = syn::parse_quote! {
        #[serde(rename_all = "camelCase")]
        struct SearchArgs {
            query_text: String,
            #[mcp(rename = "queryText")]
            query: String,
        }
    };

    let error = expand_mcp_json_schema(input).unwrap_err().to_string();

    assert!(error.contains("duplicate MCP schema field or alias name `queryText`"));
}

#[test]
fn derive_schema_supports_fieldless_enums() {
    let input: DeriveInput = syn::parse_quote! {
        #[serde(rename_all = "kebab-case")]
        enum IssueState {
            Open,
            #[serde(alias = "reviewing")]
            InReview,
            #[mcp(rename = "done", alias = "resolved")]
            Closed,
            #[serde(other)]
            Unknown,
        }
    };

    let expanded = expand_mcp_json_schema(input)
        .expect("enum schema derive should expand")
        .to_token_stream()
        .to_string();

    assert!(expanded.contains("open"));
    assert!(expanded.contains("in-review"));
    assert!(expanded.contains("reviewing"));
    assert!(expanded.contains("done"));
    assert!(expanded.contains("resolved"));
    assert!(expanded.contains("x-mcpEnumDecodeAliases"));
    assert!(!expanded.contains("Unknown"));
}

#[test]
fn derive_schema_rejects_duplicate_enum_values() {
    let input: DeriveInput = syn::parse_quote! {
        enum IssueState {
            Open,
            #[serde(alias = "Open")]
            InReview,
        }
    };

    let error = expand_mcp_json_schema(input).unwrap_err().to_string();

    assert!(error.contains("duplicate MCP enum value name `Open`"));
}

#[test]
fn derive_tool_input_generates_schema_and_strict_decoder() {
    let input: DeriveInput = syn::parse_quote! {
        #[serde(rename_all = "camelCase")]
        struct SearchArgs {
            #[serde(rename(deserialize = "q"), alias = "queryText")]
            query: String,
            page_size: Option<u32>,
            #[serde(default = "default_limit")]
            limit: usize,
            #[serde(skip)]
            internal: String,
        }
    };

    let expanded = expand_mcp_tool_input(input)
        .expect("tool input derive should expand")
        .to_token_stream()
        .to_string();

    assert!(expanded.contains("McpToolInput for SearchArgs"));
    assert!(expanded.contains("McpJsonSchema for SearchArgs"));
    assert!(expanded.contains("McpToolValue"));
    assert!(expanded.contains("x-mcpAliases"));
    assert!(expanded.contains("take_required_tool_value_from :: < String >"));
    assert!(expanded.contains("take_present_tool_value_from :: < Option < u32 > >"));
    assert!(expanded.contains("default_limit ()"));
    assert!(expanded.contains("\"queryText\""));
}

#[test]
fn derive_tool_input_treats_mcp_default_value_as_rust_expression() {
    let input: DeriveInput = syn::parse_quote! {
        struct SearchArgs {
            #[mcp(optional, default = false)]
            include_archived: bool,
        }
    };

    let expanded = expand_mcp_tool_input(input)
        .expect("tool input derive should expand")
        .to_token_stream()
        .to_string();

    assert!(expanded.contains("false"));
}

#[test]
fn derive_tool_input_rejects_tuple_structs() {
    let input: DeriveInput = syn::parse_quote! {
        struct SearchArgs(String);
    };

    let error = expand_mcp_tool_input(input).unwrap_err().to_string();

    assert!(error.contains("struct with named fields"));
}

#[test]
fn derive_tool_input_rejects_optional_non_option_fields_without_defaults() {
    let input: DeriveInput = syn::parse_quote! {
        struct SearchArgs {
            #[mcp(optional)]
            limit: usize,
        }
    };

    let error = expand_mcp_tool_input(input).unwrap_err().to_string();

    assert!(error.contains("optional non-Option field without a default"));
}

#[test]
fn derive_tool_input_rejects_duplicate_field_aliases() {
    let input: DeriveInput = syn::parse_quote! {
        struct SearchArgs {
            #[serde(alias = "q")]
            query: String,
            #[mcp(rename = "q")]
            text: String,
        }
    };

    let error = expand_mcp_tool_input(input).unwrap_err().to_string();

    assert!(error.contains("duplicate MCP tool input field or alias name `q`"));
}

#[test]
fn derive_tool_input_rejects_alias_matching_primary_name() {
    let input: DeriveInput = syn::parse_quote! {
        struct SearchArgs {
            #[serde(alias = "query")]
            query: String,
        }
    };

    let error = expand_mcp_tool_input(input).unwrap_err().to_string();

    assert!(error.contains("duplicate MCP tool input field or alias name `query`"));
}

#[test]
fn derive_schema_rejects_data_bearing_enums() {
    let input: DeriveInput = syn::parse_quote! {
        enum Event {
            Click { x: u32, y: u32 },
        }
    };

    let error = expand_mcp_json_schema(input).unwrap_err().to_string();

    assert!(error.contains("fieldless enum variants"));
}

#[test]
fn derive_schema_follows_serde_deserialize_metadata() {
    let input: DeriveInput = syn::parse_quote! {
        #[serde(rename_all = "camelCase")]
        struct SearchArgs {
            created_at: String,
            #[serde(rename(deserialize = "q", serialize = "query"))]
            query: String,
            #[serde(default)]
            page_size: u32,
            #[serde(skip_deserializing)]
            cache_key: String,
        }
    };

    let expanded = expand_mcp_json_schema(input)
        .expect("schema derive should expand")
        .to_token_stream()
        .to_string();

    assert!(expanded.contains("\"createdAt\""));
    assert!(expanded.contains("\"q\""));
    assert!(expanded.contains("\"pageSize\""));
    assert!(!expanded.contains("\"cache_key\""));
    assert!(!expanded.contains("\"cacheKey\""));
    assert!(expanded.contains("if false"));
}

#[test]
fn derive_schema_rejects_flattened_fields() {
    let input: DeriveInput = syn::parse_quote! {
        struct SearchArgs {
            #[serde(flatten)]
            filters: Filters,
        }
    };

    let error = expand_mcp_json_schema(input).unwrap_err().to_string();

    assert!(error.contains("cannot infer `serde(flatten)`"));
}

#[test]
fn derive_schema_rejects_unknown_container_mcp_options() {
    let input: DeriveInput = syn::parse_quote! {
        #[mcp(schema = "anything")]
        struct SearchArgs {
            query: String,
        }
    };

    let error = expand_mcp_json_schema(input).unwrap_err().to_string();

    assert!(error.contains("unknown `mcp` container option"));
}
