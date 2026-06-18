//! Shared MCP schema helpers and `rmcp` server glue for component-shape integrations.
//!
//! This crate owns protocol-level building blocks, schema-paired value
//! decoding, and common validation metadata/error helpers. Domain integrations
//! still own validation execution, authorization, and handler contracts.

mod arguments;
mod definitions;
mod error;
mod input_schema;
mod metadata;
mod names;
mod results;
mod schema;
mod server;
mod validation;

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt,
    future::Future,
    hash::Hash,
    marker::PhantomData,
    ops::Index,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
};

pub use arguments::*;
pub use component_shape::{
    ComponentShapeFor, ComponentShapeMetadata, McpInput, McpInputShape, McpPrimitiveKind,
    McpRangeBoundKind,
};
#[cfg(feature = "derive")]
pub use component_shape_mcp_macros::{McpJsonSchema, McpToolInput};
pub use definitions::*;
pub(crate) use definitions::{validate_required_metadata_text, validate_tool_annotation_hints};
pub use error::*;
pub(crate) use error::{
    normalize_value_against_schema, reject_unknown_arguments, type_includes,
    validate_value_against_closed_schema,
};
pub(crate) use input_schema::value_schema_allows_null;
pub use input_schema::*;
pub use metadata::*;
pub use names::*;
pub use results::*;
pub use rmcp;
pub use rmcp::model::{
    CallToolResult as ToolCallResult, Content as ContentBlock, GetPromptResult as McpPromptResult,
    Icon as McpIcon, IconTheme as McpIconTheme, Prompt as PromptDefinition,
    PromptArgument as McpPromptArgument, PromptMessage as McpPromptMessage,
    PromptMessageContent as McpPromptMessageContent, PromptMessageRole as McpPromptMessageRole,
    RawResource as RawResourceDefinition, RawResourceTemplate as RawResourceTemplateDefinition,
    ReadResourceResult as McpResourceResult, Resource as ResourceDefinition,
    ResourceContents as McpResourceContents, ResourceTemplate as ResourceTemplateDefinition,
    TaskSupport as McpToolTaskSupport, Tool as ToolDefinition,
    ToolAnnotations as McpToolAnnotations, ToolExecution as McpToolExecution,
};
use rmcp::{
    ErrorData, RoleServer, ServerHandler, ServiceExt as _,
    model::{
        AnnotateAble, CallToolRequestParams, GetPromptRequestParams, GetPromptResult,
        Implementation, JsonObject, ListPromptsResult, ListResourceTemplatesResult,
        ListResourcesResult, ListToolsResult, PaginatedRequestParams, Prompt, PromptMessage,
        PromptMessageRole, ProtocolVersion, RawResource, RawResourceTemplate,
        ReadResourceRequestParams, ReadResourceResult, ResourceContents, ResourceTemplate,
        ServerCapabilities, ServerInfo, Tool,
    },
    service::{MaybeSendFuture, RequestContext},
    transport::stdio,
};
pub use schema::*;
pub use serde;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
pub use serde_json;
use serde_json::{Map, Number, Value, json};
pub use server::*;
use strum::IntoStaticStr;
pub use validation::*;

pub const MCP_PROTOCOL_VERSION: &str = "2025-11-25";

pub type ServeStdioResult = Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>;
pub type McpToolArguments = Map<String, Value>;
pub type McpSchemaProperties = BTreeMap<String, McpSchema>;
pub type McpSchemaFn = fn() -> McpSchema;
type ToolFuture = Pin<Box<dyn Future<Output = ToolCallResult> + Send + 'static>>;
type ResourceFuture =
    Pin<Box<dyn Future<Output = Result<ReadResourceResult, ErrorData>> + Send + 'static>>;
type PromptFuture =
    Pin<Box<dyn Future<Output = Result<GetPromptResult, ErrorData>> + Send + 'static>>;

#[cfg(test)]
mod tests;
