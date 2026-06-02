use crate::ComponentSuffix;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderCapability {
    None,
    Component,
}

impl RenderCapability {
    pub const fn enabled(self) -> bool {
        matches!(self, Self::Component)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueBindingCapability {
    None,
    Inherited,
}

impl ValueBindingCapability {
    pub const fn enabled(self) -> bool {
        matches!(self, Self::Inherited)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComponentCapabilities {
    render: RenderCapability,
    value_binding: ValueBindingCapability,
}

impl ComponentCapabilities {
    pub const fn new() -> Self {
        Self {
            render: RenderCapability::None,
            value_binding: ValueBindingCapability::None,
        }
    }

    pub const fn with_render(mut self, render: RenderCapability) -> Self {
        self.render = render;
        self
    }

    pub const fn with_value_binding(mut self, value_binding: ValueBindingCapability) -> Self {
        self.value_binding = value_binding;
        self
    }

    pub const fn render(self) -> RenderCapability {
        self.render
    }

    pub const fn value_binding(self) -> ValueBindingCapability {
        self.value_binding
    }

    pub const fn render_component(self) -> bool {
        self.render.enabled()
    }

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
    pub const fn new() -> Self {
        Self { field_suffix: None }
    }

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
