use crate::{
    ComponentCapabilities, ComponentPrototyping, ComponentShapeFor, ComponentShapeMetadata,
    McpInput, RustPath, RustType,
};

/// Typed borrowed source field name used in component shape metadata.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    derive_more::AsRef,
    derive_more::Display,
    derive_more::From,
    derive_more::Into,
)]
#[as_ref(forward)]
#[display("{_0}")]
pub struct ComponentFieldName<'a>(&'a str);

impl<'a> ComponentFieldName<'a> {
    /// Creates a typed field-name wrapper from borrowed identifier text.
    pub const fn new(value: &'a str) -> Self {
        Self(value)
    }

    /// Returns the wrapped source field name.
    pub const fn as_str(self) -> &'a str {
        self.0
    }
}

/// Framework-neutral metadata describing a field's component shape use.
///
/// This records that a source field is associated with a component shape path,
/// along with optional field type metadata and shape-owned capability or
/// prototyping metadata. It is metadata only: runtime construction contracts,
/// framework-specific render behavior, and storage semantics belong in the
/// backend-specific crates that consume it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComponentShapeUse {
    field_name: ComponentFieldName<'static>,
    field_type: Option<RustType>,
    shape_path: RustPath,
    capabilities: ComponentCapabilities,
    prototyping: ComponentPrototyping,
    mcp_input: McpInput,
}

impl ComponentShapeUse {
    /// Records that `field_name` uses the component shape at `shape_path`.
    pub const fn new(field_name: ComponentFieldName<'static>, shape_path: RustPath) -> Self {
        Self {
            field_name,
            field_type: None,
            shape_path,
            capabilities: ComponentCapabilities::new(),
            prototyping: ComponentPrototyping::new(),
            mcp_input: McpInput::unsupported(),
        }
    }

    /// Records that a source field uses the component shape at `shape_path`.
    pub const fn for_field(field_name: &'static str, shape_path: RustPath) -> Self {
        Self::new(ComponentFieldName::new(field_name), shape_path)
    }

    /// Adds Rust type metadata for the source field.
    pub const fn with_field_type(mut self, field_type: RustType) -> Self {
        self.field_type = Some(field_type);
        self
    }

    /// Replaces the shape capability metadata.
    pub const fn with_capabilities(mut self, capabilities: ComponentCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Replaces the shape prototyping metadata.
    pub const fn with_prototyping(mut self, prototyping: ComponentPrototyping) -> Self {
        self.prototyping = prototyping;
        self
    }

    /// Replaces the shape's model-controlled MCP input metadata.
    pub const fn with_mcp_input(mut self, mcp_input: McpInput) -> Self {
        self.mcp_input = mcp_input;
        self
    }

    /// Copies type-owned shape metadata from a concrete component shape.
    pub const fn with_shape_metadata<Shape>(mut self) -> Self
    where
        Shape: ComponentShapeMetadata,
    {
        self.capabilities = Shape::CAPABILITIES;
        self.prototyping = Shape::PROTOTYPING;
        self.mcp_input = Shape::MCP_INPUT;
        self
    }

    /// Copies value-specific MCP input metadata from a component shape/value pair.
    pub const fn with_value_mcp_input<Shape, Value>(mut self) -> Self
    where
        Shape: ComponentShapeFor<Value>,
    {
        self.mcp_input = <Shape as ComponentShapeFor<Value>>::MCP_INPUT;
        self
    }

    /// Source field name.
    pub const fn field_name(self) -> ComponentFieldName<'static> {
        self.field_name
    }

    /// Optional Rust type metadata for the source field.
    pub const fn field_type(self) -> Option<RustType> {
        self.field_type
    }

    /// Component shape path used by the source field.
    pub const fn shape_path(self) -> RustPath {
        self.shape_path
    }

    /// Shape capability metadata.
    pub const fn capabilities(self) -> ComponentCapabilities {
        self.capabilities
    }

    /// Shape prototyping metadata.
    pub const fn prototyping(self) -> ComponentPrototyping {
        self.prototyping
    }

    /// Shape metadata for model-controlled MCP input.
    pub const fn mcp_input(self) -> McpInput {
        self.mcp_input
    }
}

#[cfg(test)]
mod tests {
    use super::{ComponentFieldName, ComponentShapeUse};
    use crate::{
        ComponentCapabilities, ComponentPrototyping, ComponentShapeFor, ComponentShapeMetadata,
        McpInput, McpInputShape, McpPrimitiveKind, RenderCapability, RustPath, RustType,
        ValueBindingCapability,
    };

    struct TextShape;

    impl ComponentShapeMetadata for TextShape {
        const CAPABILITIES: ComponentCapabilities =
            ComponentCapabilities::new().with_render(RenderCapability::Component);
        const PROTOTYPING: ComponentPrototyping = ComponentPrototyping::new().field_suffix("input");
        const MCP_INPUT: McpInput = McpInput::string();
    }

    impl ComponentShapeFor<Vec<String>> for TextShape {
        const MCP_INPUT: McpInput = McpInput::string_list();
    }

    #[test]
    fn component_field_name_exposes_borrowed_identifier_text() {
        let field_name = ComponentFieldName::new("title");
        let field_name_ref: &str = field_name.as_ref();

        assert_eq!(field_name.as_str(), "title");
        assert_eq!(field_name_ref, "title");
        assert_eq!(field_name.to_string(), "title");
    }

    #[test]
    fn component_shape_use_defaults_to_no_capabilities_or_prototyping() {
        let shape_use = ComponentShapeUse::new(
            ComponentFieldName::new("title"),
            RustPath::from_macro_tokens_unchecked("crate::fields::TitleInput"),
        );

        assert_eq!(shape_use.capabilities(), ComponentCapabilities::new());
        assert_eq!(shape_use.prototyping(), ComponentPrototyping::new());
        assert_eq!(shape_use.mcp_input(), McpInput::unsupported());
    }

    #[test]
    fn component_shape_use_preserves_field_name_and_shape_path() {
        let shape_use = ComponentShapeUse::new(
            ComponentFieldName::new("title"),
            RustPath::from_macro_tokens_unchecked("crate::fields::TitleInput"),
        );

        assert_eq!(shape_use.field_name().as_str(), "title");
        assert_eq!(shape_use.shape_path().as_str(), "crate::fields::TitleInput");
    }

    #[test]
    fn component_shape_use_records_optional_field_type() {
        let shape_use = ComponentShapeUse::new(
            ComponentFieldName::new("title"),
            RustPath::from_macro_tokens_unchecked("crate::fields::TitleInput"),
        )
        .with_field_type(RustType::from_macro_tokens_unchecked("String"));

        assert_eq!(shape_use.field_type().map(RustType::as_str), Some("String"));
    }

    #[test]
    fn component_shape_use_preserves_explicit_capabilities_and_prototyping() {
        let capabilities = ComponentCapabilities::new()
            .with_render(RenderCapability::Component)
            .with_value_binding(ValueBindingCapability::Inherited);
        let prototyping = ComponentPrototyping::new().field_suffix("title_input");

        let shape_use = ComponentShapeUse::new(
            ComponentFieldName::new("title"),
            RustPath::from_macro_tokens_unchecked("crate::fields::TitleInput"),
        )
        .with_capabilities(capabilities)
        .with_prototyping(prototyping);

        assert_eq!(shape_use.capabilities(), capabilities);
        assert_eq!(
            shape_use
                .prototyping()
                .field_suffix
                .map(crate::ComponentSuffix::as_str),
            Some("title_input")
        );
    }

    #[test]
    fn component_shape_use_preserves_mcp_input_metadata() {
        let input = McpInput::string();

        let shape_use = ComponentShapeUse::new(
            ComponentFieldName::new("title"),
            RustPath::from_macro_tokens_unchecked("crate::fields::TitleInput"),
        )
        .with_mcp_input(input);

        assert_eq!(shape_use.mcp_input(), input);
        assert_eq!(
            shape_use.mcp_input().input_shape(),
            McpInputShape::Scalar(McpPrimitiveKind::String)
        );
    }

    #[test]
    fn component_shape_use_can_copy_shape_metadata_from_type() {
        let shape_use = ComponentShapeUse::new(
            ComponentFieldName::new("title"),
            RustPath::from_macro_tokens_unchecked("crate::fields::TitleInput"),
        )
        .with_shape_metadata::<TextShape>();

        assert_eq!(
            shape_use.capabilities().render(),
            RenderCapability::Component
        );
        assert_eq!(
            shape_use
                .prototyping()
                .field_suffix
                .map(crate::ComponentSuffix::as_str),
            Some("input")
        );
        assert_eq!(
            shape_use.mcp_input().input_shape(),
            McpInputShape::Scalar(McpPrimitiveKind::String)
        );
    }

    #[test]
    fn component_shape_use_can_copy_value_specific_mcp_input_from_type() {
        let shape_use = ComponentShapeUse::new(
            ComponentFieldName::new("tags"),
            RustPath::from_macro_tokens_unchecked("crate::fields::TagsInput"),
        )
        .with_shape_metadata::<TextShape>()
        .with_value_mcp_input::<TextShape, Vec<String>>();

        assert_eq!(
            shape_use.mcp_input().input_shape(),
            McpInputShape::List(McpPrimitiveKind::String)
        );
    }
}
