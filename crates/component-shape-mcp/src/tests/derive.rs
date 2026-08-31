use super::*;

#[test]
fn json_schema_derive_builds_object_schema() {
    #[derive(crate::McpJsonSchema)]
    #[allow(dead_code)]
    struct SearchArgs {
        #[mcp(rename = "q", alias = "query", description = "Search text")]
        query: String,
        page: Option<u32>,
        #[serde(skip)]
        internal: String,
    }

    let schema = SearchArgs::json_schema();

    assert_eq!(schema["type"], "object");
    assert_eq!(schema["properties"]["q"]["type"], "string");
    assert_eq!(schema["properties"]["q"]["description"], "Search text");
    assert_eq!(schema["properties"]["q"]["x-mcpAliases"], json!(["query"]));
    assert_eq!(schema["properties"]["q"]["x-mcpDecodeName"], "query");
    assert_eq!(schema["properties"]["page"]["anyOf"][0]["type"], "integer");
    assert_eq!(schema["required"], json!(["q"]));
    assert!(schema["properties"].get("internal").is_none());
}

#[test]
fn json_schema_derive_infers_doc_descriptions() {
    /// Search arguments sent to the tool.
    #[derive(crate::McpJsonSchema)]
    #[allow(dead_code)]
    struct SearchArgs {
        /// Full text query.
        query: String,
    }

    let schema = SearchArgs::json_schema();

    assert_eq!(schema["description"], "Search arguments sent to the tool.");
    assert_eq!(
        schema["properties"]["query"]["description"],
        "Full text query."
    );
}

#[test]
fn json_schema_derive_builds_enum_schema() {
    /// Issue state.
    #[derive(crate::McpJsonSchema)]
    #[allow(dead_code)]
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

    let schema = IssueState::json_schema();

    assert_eq!(schema["type"], "string");
    assert_eq!(schema["description"], "Issue state.");
    assert_eq!(
        schema["enum"],
        json!(["open", "in-review", "reviewing", "done", "resolved"])
    );
    assert_eq!(schema["x-mcpEnumDecodeAliases"]["done"], "closed");
    assert_eq!(schema["x-mcpEnumDecodeAliases"]["resolved"], "closed");
}

#[test]
fn tool_input_derive_builds_schema_and_decodes_strict_arguments() {
    fn default_limit() -> usize {
        25
    }

    #[derive(Debug, crate::McpToolInput, PartialEq)]
    #[allow(dead_code)]
    #[serde(rename_all = "camelCase")]
    struct SearchInput {
        #[serde(rename(deserialize = "q"), alias = "queryText")]
        query: String,
        page_size: Option<u32>,
        #[serde(default = "default_limit")]
        limit: usize,
        #[serde(skip)]
        internal: String,
    }

    let schema = SearchInput::input_schema();
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["properties"]["q"]["type"], "string");
    assert_eq!(
        schema["properties"]["q"]["x-mcpAliases"],
        json!(["queryText"])
    );
    assert_eq!(
        schema["properties"]["pageSize"]["anyOf"][0]["type"],
        "integer"
    );
    assert_eq!(schema["required"], json!(["q"]));
    assert!(schema["properties"].get("internal").is_none());
    assert_eq!(SearchInput::json_schema(), schema);

    let input = SearchInput::from_tool_call(
        crate::McpToolCall::from_value(Some(json!({
            "queryText": "rust",
            "pageSize": 2
        })))
        .expect("tool call should normalize"),
    )
    .expect("input should decode");

    assert_eq!(
        input,
        SearchInput {
            query: "rust".to_string(),
            page_size: Some(2),
            limit: 25,
            internal: String::new(),
        }
    );
}

#[test]
fn tool_input_derive_uses_custom_tool_value_decoders() {
    #[derive(Debug, PartialEq)]
    struct SlashSeparatedTags(Vec<String>);

    impl crate::McpToolValue for SlashSeparatedTags {
        fn tool_value_schema() -> crate::McpSchema {
            schema(json!({ "type": "string" }))
        }

        fn from_tool_value(field: &str, value: Value) -> Result<Self, crate::McpToolError> {
            let raw = value.as_str().ok_or_else(|| {
                crate::McpToolError::decode(field, "expected slash-separated tags")
            })?;
            Ok(Self(
                raw.split('/')
                    .map(str::trim)
                    .filter(|tag| !tag.is_empty())
                    .map(ToString::to_string)
                    .collect(),
            ))
        }
    }

    #[derive(Debug, crate::McpToolInput, PartialEq)]
    struct TagInput {
        tags: SlashSeparatedTags,
    }

    assert_eq!(
        TagInput::input_schema()["properties"]["tags"]["type"],
        "string"
    );

    let input = TagInput::from_tool_call(
        crate::McpToolCall::from_value(Some(json!({
            "tags": "alpha / beta"
        })))
        .expect("tool call should normalize"),
    )
    .expect("custom tool value should decode");

    assert_eq!(
        input,
        TagInput {
            tags: SlashSeparatedTags(vec!["alpha".to_string(), "beta".to_string()])
        }
    );

    let error = TagInput::from_tool_call(
        crate::McpToolCall::from_value(Some(json!({
            "tags": ["alpha"]
        })))
        .expect("tool call should normalize"),
    )
    .expect_err("custom tool value should reject invalid JSON");

    assert_eq!(
        error,
        crate::McpToolError::DecodeField {
            field: "tags".to_string(),
            message: "expected slash-separated tags".to_string(),
        }
    );
}

#[test]
fn tool_input_derive_rejects_missing_duplicate_and_unknown_fields() {
    #[derive(Debug, crate::McpToolInput, PartialEq)]
    #[allow(dead_code)]
    struct SearchInput {
        #[serde(alias = "queryText")]
        query: String,
    }

    let missing = SearchInput::from_tool_call(
        crate::McpToolCall::from_value(Some(json!({}))).expect("tool call should normalize"),
    )
    .expect_err("required field should be enforced");
    assert_eq!(
        missing,
        crate::McpToolError::MissingField {
            field: "query".to_string()
        }
    );

    let duplicate = SearchInput::from_tool_call(
        crate::McpToolCall::from_value(Some(json!({
            "query": "rust",
            "queryText": "go"
        })))
        .expect("tool call should normalize"),
    )
    .expect_err("duplicate aliases should be rejected");
    assert_eq!(
        duplicate,
        crate::McpToolError::DuplicateField {
            field: "query".to_string()
        }
    );

    let unknown = SearchInput::from_tool_call(
        crate::McpToolCall::from_value(Some(json!({
            "query": "rust",
            "extra": true
        })))
        .expect("tool call should normalize"),
    )
    .expect_err("unknown fields should be rejected");
    assert_eq!(
        unknown,
        crate::McpToolError::UnknownField {
            field: "extra".to_string()
        }
    );
}

#[test]
fn unit_tool_input_accepts_only_empty_arguments() {
    assert_eq!(
        <() as crate::McpToolInput>::input_schema()["properties"],
        json!({})
    );
    assert_eq!(
        <() as crate::McpToolInput>::from_tool_call(crate::McpToolCall::empty()),
        Ok(())
    );
    assert_eq!(
        <() as crate::McpToolInput>::from_tool_call(
            crate::McpToolCall::from_value(Some(json!({ "extra": true })))
                .expect("tool call should normalize")
        ),
        Err(crate::McpToolError::UnknownField {
            field: "extra".to_string()
        })
    );
}
