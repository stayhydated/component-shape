use crate::{ComponentCapabilities, ComponentPrototyping, RustPath, RustType};

/// Framework-neutral metadata describing a field's component shape use.
///
/// This records that a source field is associated with a component shape path,
/// along with optional field type metadata and shape-owned capability or
/// prototyping metadata. It is metadata only: runtime construction contracts,
/// framework-specific render behavior, and storage semantics belong in the
/// backend-specific crates that consume it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComponentShapeUse {
    field_name: &'static str,
    field_type: Option<RustType>,
    shape_path: RustPath,
    capabilities: ComponentCapabilities,
    prototyping: ComponentPrototyping,
}

impl ComponentShapeUse {
    /// Records that `field_name` uses the component shape at `shape_path`.
    pub const fn new(field_name: &'static str, shape_path: RustPath) -> Self {
        Self {
            field_name,
            field_type: None,
            shape_path,
            capabilities: ComponentCapabilities::new(),
            prototyping: ComponentPrototyping::new(),
        }
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

    /// Source field name.
    pub const fn field_name(self) -> &'static str {
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
}

#[cfg(test)]
mod tests {
    use super::ComponentShapeUse;
    use crate::{
        ComponentCapabilities, ComponentPrototyping, RenderCapability, RustPath, RustType,
        ValueBindingCapability,
    };

    #[test]
    fn component_shape_use_defaults_to_no_capabilities_or_prototyping() {
        let shape_use = ComponentShapeUse::new(
            "title",
            RustPath::from_macro_tokens_unchecked("crate::fields::TitleInput"),
        );

        assert_eq!(shape_use.capabilities(), ComponentCapabilities::new());
        assert_eq!(shape_use.prototyping(), ComponentPrototyping::new());
    }

    #[test]
    fn component_shape_use_preserves_field_name_and_shape_path() {
        let shape_use = ComponentShapeUse::new(
            "title",
            RustPath::from_macro_tokens_unchecked("crate::fields::TitleInput"),
        );

        assert_eq!(shape_use.field_name(), "title");
        assert_eq!(shape_use.shape_path().as_str(), "crate::fields::TitleInput");
    }

    #[test]
    fn component_shape_use_records_optional_field_type() {
        let shape_use = ComponentShapeUse::new(
            "title",
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
            "title",
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
}
