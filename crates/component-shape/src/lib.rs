//! Framework-neutral component shape metadata and naming utilities.
//!
//! `McpInput` describes model-controlled structured input at the metadata
//! level. Protocol handling and JSON Schema helpers live in `component-shape-mcp`.

mod component_suffix;
mod field_component;
mod mcp;
mod metadata;
mod rust_syntax;
mod value_change;

pub use component_suffix::{
    ComponentSuffix, ComponentSuffixError, component_suffix_from_suffix, is_valid_component_suffix,
    validate_component_suffix,
};
pub use field_component::{ComponentFieldName, ComponentShapeUse};
pub use mcp::{
    McpInput, McpInputShape, McpPrimitiveKind, McpRangeBoundKind, McpToolMetadataError,
    validate_mcp_tool_metadata_text, validate_mcp_tool_name,
};
pub use metadata::{
    ComponentCapabilities, ComponentPrototyping, RenderCapability, ValueBindingCapability,
};
pub use rust_syntax::{RustExpr, RustPath, RustSyntaxError, RustSyntaxKind, RustType};
pub use value_change::ValueChange;

/// Framework-neutral metadata published by a component shape.
pub trait ComponentShapeMetadata {
    /// Generator-facing prototyping metadata owned by the shape.
    const PROTOTYPING: ComponentPrototyping = ComponentPrototyping::new();
    /// Framework-neutral runtime capabilities published by the shape.
    const CAPABILITIES: ComponentCapabilities = ComponentCapabilities::new();
    /// Coarse model-controlled input metadata published by the shape.
    const MCP_INPUT: McpInput = McpInput::unsupported();
}

/// Marker for component shapes declared through a trusted declaration surface.
///
/// Backend crates implement this for shapes declared by their public macros or
/// other trusted declaration APIs. Hand-written runtime implementations should
/// not automatically satisfy this marker unless that backend explicitly accepts
/// them as declared shapes.
pub trait DeclaredComponentShape: ComponentShapeMetadata {}

/// Marker that a component shape supports a value or field type.
///
/// Backend-specific compatibility traits can extend or pair with this marker
/// while keeping backend-owned methods and diagnostics on their own traits.
/// The value-specific MCP input inherits the shape-level MCP input by default,
/// and declaration macros may emit a more precise value-specific override for
/// simple JSON-compatible value shapes.
pub trait ComponentShapeFor<Value>: ComponentShapeMetadata {
    /// Coarse model-controlled input metadata for this shape/value pair.
    ///
    /// This inherits [`ComponentShapeMetadata::MCP_INPUT`] unless the pair
    /// publishes a more precise value-specific shape.
    const MCP_INPUT: McpInput = <Self as ComponentShapeMetadata>::MCP_INPUT;
}

#[cfg(test)]
mod tests {
    use super::{
        ComponentShapeFor, ComponentShapeMetadata, McpInput, McpInputShape, McpPrimitiveKind,
    };

    struct TextShape;

    impl ComponentShapeMetadata for TextShape {
        const MCP_INPUT: McpInput = McpInput::string();
    }

    impl ComponentShapeFor<String> for TextShape {}

    impl ComponentShapeFor<Vec<String>> for TextShape {
        const MCP_INPUT: McpInput = McpInput::string_list();
    }

    #[test]
    fn component_shape_for_inherits_shape_level_mcp_input_by_default() {
        assert_eq!(
            <TextShape as ComponentShapeFor<String>>::MCP_INPUT.input_shape(),
            McpInputShape::Scalar(McpPrimitiveKind::String)
        );
    }

    #[test]
    fn component_shape_for_can_override_value_specific_mcp_input() {
        assert_eq!(
            <TextShape as ComponentShapeFor<Vec<String>>>::MCP_INPUT.input_shape(),
            McpInputShape::List(McpPrimitiveKind::String)
        );
    }
}
