use crate::ComponentSuffix;

/// Whether a component shape publishes render-component metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderCapability {
    /// No render component is published.
    None,
    /// A render component is published.
    Component,
}

impl RenderCapability {
    /// Returns whether rendering is enabled.
    pub const fn enabled(self) -> bool {
        matches!(self, Self::Component)
    }
}

/// Whether a component shape publishes value-binding metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueBindingCapability {
    /// No value binding is published.
    None,
    /// Value binding is inherited from a backing component state contract.
    Inherited,
}

impl ValueBindingCapability {
    /// Returns whether value binding is enabled.
    pub const fn enabled(self) -> bool {
        matches!(self, Self::Inherited)
    }
}

/// Shape-owned capability flags consumed by generators and integrations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComponentCapabilities {
    render: RenderCapability,
    value_binding: ValueBindingCapability,
}

impl ComponentCapabilities {
    /// Creates capability metadata with no optional capabilities enabled.
    pub const fn new() -> Self {
        Self {
            render: RenderCapability::None,
            value_binding: ValueBindingCapability::None,
        }
    }

    /// Replaces the render capability.
    pub const fn with_render(mut self, render: RenderCapability) -> Self {
        self.render = render;
        self
    }

    /// Replaces the value-binding capability.
    pub const fn with_value_binding(mut self, value_binding: ValueBindingCapability) -> Self {
        self.value_binding = value_binding;
        self
    }

    /// Returns the render capability.
    pub const fn render(self) -> RenderCapability {
        self.render
    }

    /// Returns the value-binding capability.
    pub const fn value_binding(self) -> ValueBindingCapability {
        self.value_binding
    }

    /// Returns whether a render component is published.
    pub const fn render_component(self) -> bool {
        self.render.enabled()
    }

    /// Returns whether value binding is enabled.
    pub const fn value_binding_enabled(self) -> bool {
        self.value_binding.enabled()
    }
}

impl Default for ComponentCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

/// Shape-owned metadata for prototyping generators.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComponentPrototyping {
    /// Preferred generated prototyping helper suffix, such as `"input"` or `"select"`.
    pub field_suffix: Option<ComponentSuffix>,
}

impl ComponentPrototyping {
    /// Creates prototyping metadata with no preferred field suffix.
    pub const fn new() -> Self {
        Self { field_suffix: None }
    }

    /// Sets the preferred generated prototyping helper suffix.
    ///
    /// # Panics
    ///
    /// Panics when `suffix` is not a valid component suffix.
    pub const fn field_suffix(mut self, suffix: &'static str) -> Self {
        self.field_suffix = Some(ComponentSuffix::new(suffix));
        self
    }
}

impl Default for ComponentPrototyping {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ComponentCapabilities, ComponentPrototyping, RenderCapability, ValueBindingCapability,
    };

    #[test]
    fn component_capabilities_default_to_no_runtime_features() {
        let capabilities = ComponentCapabilities::new();

        assert_eq!(capabilities.render(), RenderCapability::None);
        assert_eq!(capabilities.value_binding(), ValueBindingCapability::None);
        assert!(!capabilities.render_component());
        assert!(!capabilities.value_binding_enabled());
        assert_eq!(ComponentCapabilities::default(), capabilities);
    }

    #[test]
    fn component_capabilities_builders_set_independent_features() {
        let capabilities = ComponentCapabilities::new()
            .with_render(RenderCapability::Component)
            .with_value_binding(ValueBindingCapability::Inherited);

        assert_eq!(capabilities.render(), RenderCapability::Component);
        assert_eq!(
            capabilities.value_binding(),
            ValueBindingCapability::Inherited
        );
        assert!(capabilities.render_component());
        assert!(capabilities.value_binding_enabled());
    }

    #[test]
    fn component_prototyping_field_suffix_records_suffix() {
        let prototyping = ComponentPrototyping::new().field_suffix("input");

        assert_eq!(
            prototyping.field_suffix.map(crate::ComponentSuffix::as_str),
            Some("input")
        );
    }
}
