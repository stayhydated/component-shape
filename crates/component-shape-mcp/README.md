# component-shape-mcp

`component-shape-mcp` contains shared MCP server and JSON Schema helpers for
crates that consume `component-shape` metadata. Protocol lifecycle and stdio
transport are delegated to the official `rmcp` SDK.

Most component authors should start with `component-shape` or
`component-shape-gpui`. Use this crate when building an integration layer that
needs MCP tool definitions, a dynamic `rmcp` tool server, structured tool
results, sync or async tool executors, a blocking stdio server entry point, or
JSON Schema from `McpInput` metadata. Generated integration crates can use
`McpToolMetadata` to carry optional application-owned tool names, titles,
descriptions, icons, task-support execution metadata, and MCP
`ToolAnnotations` hints such as read-only, destructive, idempotent, and
open-world behavior beside their domain descriptors, and can call
`validate_tool_name`, `validate_tool_metadata_text`, or
`McpToolMetadata::validate()` to share the same validation as final tool
construction. Metadata validation rejects tools marked as both read-only and
destructive and rejects empty icon fields.
`McpServer::add_tool` and `add_tool_async` also validate raw custom
`ToolDefinition`s, so manual registrations cannot bypass shared name,
metadata, or annotation rules.

```rust
use component_shape::McpInput;

let schema = component_shape_mcp::schema_for_input(McpInput::date_time_range());
```

Use `McpJsonSchema` when a Rust type should publish a more precise typed schema
than the coarse `McpInput::object()` marker. Built-in implementations cover
common primitive, `Option<T>`, `Vec<T>`, slices, arrays, fixed tuples with 1 to
4 elements, sets, string-keyed maps, references, `Cow<T>`, and boxed values, so
type aliases inherit the underlying schema. `McpRange<T>` covers typed
`{ "min": ..., "max": ... }` range arguments. `McpAny` is the explicit typed
wrapper for fields that intentionally accept unconstrained JSON;
`serde_json::Value` also publishes the same unconstrained schema for dynamic
argument fields. Tool output schemas must declare an object root. App-owned
named structs, tuple or named transparent newtypes, and fieldless enums can
derive it. The derive
follows serde deserialize names, records field aliases in `x-mcpAliases`,
includes serde or `#[mcp(alias = "...")]` aliases for enum variants, skips
deserialization-skipped fields, rejects flattened fields that cannot be inferred
safely, rejects duplicate inferred field or enum value names, treats
serde-defaulted fields as not required, and uses Rust doc comments as JSON
Schema descriptions when `#[mcp(description = "...")]` is not set. When a value
is decoded through the default `McpToolValue` implementation, MCP field and enum
renames or aliases are normalized before serde deserialization:

```rust
/// Search arguments sent to the tool.
#[derive(component_shape_mcp::McpJsonSchema)]
struct SearchArgs {
    /// Search text.
    #[mcp(rename = "q")]
    query: String,
    page: Option<u32>,
}

#[derive(component_shape_mcp::McpJsonSchema)]
#[serde(rename_all = "kebab-case")]
enum IssueState {
    Open,
    InReview,
    Closed,
}
```

Use `McpToolValue` when a single field or filter value needs both schema and
strict decoding. It is implemented automatically for any
`T: McpJsonSchema + serde::de::DeserializeOwned`, and generated integrations
such as `gpui-form` submit fields and `gpui-table` filter shapes use that
contract for value-level MCP arguments. The default decoder checks JSON null
against the value schema before falling back to serde, so nullable schemas such
as `Option<T>` accept null while non-nullable schemas reject it consistently.

Use `McpToolInput` for top-level tool argument structs. It derives the schema
and strict typed decoding for the tool input together, follows the same `serde`
names and aliases, rejects unknown fields, and reports duplicate primary/alias
field names. It also implements `McpJsonSchema`, so the same struct can be
reused as a nested schema value without a second derive. Fields are
schema-and-decode checked through `McpToolValue`. `tool_definition_for_input`
returns an `McpTypedTool<T>`, so typed server registration keeps the handler
input and published schema paired by type:

```rust
#[derive(component_shape_mcp::McpToolInput)]
#[serde(rename_all = "camelCase")]
struct SearchArgs {
    #[serde(rename(deserialize = "q"), alias = "queryText")]
    query: String,
    page_size: Option<u32>,
}

let tool = component_shape_mcp::tool_definition_for_input::<SearchArgs>(
    "search",
    Some("Search".to_string()),
    None,
    None,
)?;

server.add_typed_tool::<SearchArgs, _>(tool, |args| {
    component_shape_mcp::tool_structured_result(
        component_shape_mcp::serde_json::json!({ "query": args.query }),
    )
})?;
```

Generated integrations that already have `McpToolMetadata` can keep name,
title, description, icons, task-support execution metadata, and
`ToolAnnotations` hints together with
`tool_definition_for_input_with_metadata::<SearchArgs>(...)`. Use
`tool_definition_with_annotations` or
`tool_definition_for_input_with_annotations::<SearchArgs>(...)` when a custom
integration computes annotations separately from `McpToolMetadata`.

GPUI shape macros infer coarse `McpInput` metadata for common primitive values,
arrays, sets, ranges, `McpAny` unconstrained JSON values, and string-keyed map
object values. Use explicit `mcp_input = ...` only when a custom value type
cannot be inferred from its Rust type.

When the derive is re-exported through `gpui_form::mcp` or `gpui_table::mcp`,
the macro infers that facade path when it is unambiguous. Use
`#[mcp(crate = path::to::mcp)]` only for renamed crates or manifests that expose
multiple MCP facades.
For example, a crate that enables both facades should pin custom schema derives
to the facade that owns the generated integration:

```rs
#[derive(gpui_form::mcp::McpJsonSchema)]
#[mcp(crate = gpui_form::mcp)]
struct AxisName(String);
```

`McpInput::unsupported()` maps to an impossible schema. Use
`McpInput::any()` for coarse metadata and `McpAny` for typed tool fields when a
tool should accept unconstrained JSON. Tool definitions reject input schemas
that do not declare object arguments with `type: "object"`, and reject output
schemas that do not declare an object root with `type: "object"`. Build custom
schemas with
`McpSchema::new(serde_json::json!(...))`; raw tool definitions registered with
`McpServer::add_tool` and `add_tool_async` are checked against the same schema
rules.
Successful calls for tools that publish `output_schema` must return object
`structured_content` that matches the declared schema. Handler error results
are passed through as errors and can still include their own structured
`error` object.
`McpInput::*_list()` maps to an ordered JSON array; `McpInput::*_set()` maps
to an array with `uniqueItems: true`.

This crate provides schema-paired value decoding, but domain integrations such
as form submit or table query crates still own validation, authorization, and
handler policy. Those integrations can register into the same `McpServer`, so
an application can serve form submit tools, table query tools, and custom tools
from one MCP server. Use `McpServer::builder(name, version)` to chain generated
registrars from integration crates with `.register(...)`, or add custom tools
with `.tool(...)` and `.tool_async(...)`. The shared server also supports MCP
resources, resource templates, and prompts with `.resource(...)`,
`.resource_async(...)`, `.resource_template(...)`, `.prompt(...)`, and
`.prompt_async(...)`; mutable servers expose matching `add_*`, `list_*`, and
`contains_*` helpers. Use `resource_definition`, `resource_template_definition`,
`json_resource_result`, `prompt_definition`, and `text_prompt_result` for the
common static JSON/text cases. If you already have a mutable server,
call `server.add_tool(...)` or `server.add_tool_async(...)` directly. Call
`.build()?`, `.serve_stdio().await`, or `.serve_stdio_blocking()` when
registration is complete. Registration returns an error for duplicate tool
names, resource URIs, or prompt names so composed server construction can fail
explicitly.

Tool failures produced by the shared server helpers keep a text content message
and also set `structured_content.error`. Typed `McpToolError` failures include
a stable `kind` plus relevant fields such as `field`, `name`, `label`,
`value`, or `detail`, so MCP clients can branch on decode, validation,
registration, unknown-tool, and handler failures without parsing text.
Validation failures that represent multiple domain errors can additionally
include a `details` array with each individual validation message.

Custom untyped tool executors receive a typed `McpToolCall`, not raw JSON. The
shared server has already normalized missing arguments to an empty object and
rejected non-object arguments before dispatch. Use `call.into_arguments()` to
get an `McpArguments` decoder, consume expected fields with
`take_required_tool_value::<T>` or `take_present_tool_value::<T>`, and finish
with `arguments.finish()?` to reject unknown fields. These helpers require
`T: McpToolValue`, so custom decoders stay paired with the schema published for
that value. Use `take_raw` or `into_inner()` only when an integration
intentionally needs raw JSON ownership.
