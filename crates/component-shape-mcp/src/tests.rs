use super::{
    McpAny, McpInput, McpJsonSchema as _, McpRange, McpServer, McpToolInput as _,
    McpValidationParam, McpValidationRule, McpValidationScope, McpValidationTarget,
    McpValidationTypeArgMode, ToolDefinition,
};
use serde_json::{Value, json};

mod derive;
mod metadata;
mod protocol;
mod registry_server;
mod resource_prompt;
mod schema;
mod support;
mod tool_value;
mod validation;

use support::schema;
