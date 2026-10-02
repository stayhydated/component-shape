use super::{
    McpAny, McpInput, McpJsonSchema as _, McpRange, McpServer, McpToolInput as _,
    McpValidationParam, McpValidationRule, McpValidationScope, McpValidationTarget,
    McpValidationTypeArgMode, ToolDefinition,
};
use serde_json::{Value, json};

mod derive;
mod metadata;
mod output_schema;
mod protocol;
mod registry_server;
mod resource_prompt;
mod schema;
mod serde_names;
mod support;
mod tool_value;
mod validation;

use support::schema;
