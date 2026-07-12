//! GPUI-specific component shape runtime contracts.
//!
//! This crate is also the GPUI facade for shared component-shape metadata such
//! as `ComponentShapeMetadata`, `ValueChange`, and `McpInput`.
//!
//! Declare owned components with the [`GpuiComponentShape`] derive and external
//! component/state pairs with [`component_shape!`]. Consumer-side configured
//! values implement [`GpuiComponentShapeBuilder`] and share the
//! [`build_component_shape`] construction path with
//! [`DefaultGpuiComponentShapeBuilder`].

pub use component_shape::{
    ComponentCapabilities, ComponentPrototyping, ComponentShapeFor, ComponentShapeMetadata,
    ComponentSuffix, DeclaredComponentShape, McpInput, McpInputShape, McpPrimitiveKind,
    McpRangeBoundKind, RenderCapability, ValueBindingCapability, ValueChange,
    component_suffix_from_suffix, is_valid_component_suffix, validate_component_suffix,
};
pub use component_shape_gpui_macros::{GpuiComponentShape, component_shape};

/// Renders a component UI value from a component state entity.
pub trait GpuiComponentRender<State: 'static>: 'static {
    /// Whether this contract renders a real component.
    const RENDERS: bool;

    /// Build a render component from the generated form field entity.
    fn new(entity: &gpui::Entity<State>) -> impl gpui::IntoElement;
}

/// Marker render contract for shapes that do not publish render metadata.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NoGpuiRenderComponent;

impl<State: 'static> GpuiComponentRender<State> for NoGpuiRenderComponent {
    const RENDERS: bool = false;

    fn new(_entity: &gpui::Entity<State>) -> impl gpui::IntoElement {
        gpui::div()
    }
}

/// Shape contract for GPUI components.
pub trait GpuiComponentShape: ComponentShapeMetadata {
    /// Backing GPUI component state type.
    type State: 'static;

    /// Shape-owned render component contract for prototyping output.
    type RenderComponent: GpuiComponentRender<Self::State>;

    /// Build the component state.
    fn new(window: &mut gpui::Window, cx: &mut gpui::Context<'_, Self::State>) -> Self::State;
}

/// Configured builder for a GPUI component shape.
///
/// Generated code can use this contract when a field selects a component shape
/// with a configuration expression, such as `Select::<_>.searchable(true)`.
/// The configured value decides how to initialize the same shape state that the
/// plain [`GpuiComponentShape::new`] path would otherwise construct.
pub trait GpuiComponentShapeBuilder<Shape: GpuiComponentShape> {
    /// Build the configured component state.
    fn build(
        self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<'_, Shape::State>,
    ) -> Shape::State;
}

/// Default builder for a shape's normal [`GpuiComponentShape::new`] behavior.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DefaultGpuiComponentShapeBuilder<Shape>(core::marker::PhantomData<fn() -> Shape>);

impl<Shape> DefaultGpuiComponentShapeBuilder<Shape> {
    /// Creates a builder that delegates to [`GpuiComponentShape::new`].
    pub const fn new() -> Self {
        Self(core::marker::PhantomData)
    }
}

impl<Shape> GpuiComponentShapeBuilder<Shape> for DefaultGpuiComponentShapeBuilder<Shape>
where
    Shape: GpuiComponentShape,
{
    fn build(
        self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<'_, Shape::State>,
    ) -> Shape::State {
        Shape::new(window, cx)
    }
}

/// Marker for component shapes declared through component-shape GPUI macros.
#[diagnostic::on_unimplemented(
    message = "GPUI component shape `{Self}` must be declared with `component_shape_gpui::component_shape!` or `#[derive(component_shape_gpui::GpuiComponentShape)]`",
    note = "hand-written `GpuiComponentShape` implementations are not accepted by consumers that require declared shapes"
)]
pub trait DeclaredGpuiComponentShape: GpuiComponentShape + DeclaredComponentShape {}

/// Marker that a GPUI component shape supports a value type.
///
/// This also requires the framework-neutral [`ComponentShapeFor<Value>`]
/// marker so GPUI value compatibility always carries value-specific shape
/// metadata for downstream generators and MCP integrations.
#[diagnostic::on_unimplemented(
    message = "GPUI component shape `{Self}` is not compatible with value `{Value}`",
    note = "declare `value = {Value}`, include `{Value}` in `values(...)`, or publish value compatibility through a value-binding impl"
)]
pub trait GpuiComponentShapeFor<Value>: GpuiComponentShape + ComponentShapeFor<Value> {}

/// Optional value-binding contract for GPUI component shapes.
#[diagnostic::on_unimplemented(
    message = "GPUI component shape `{Self}` does not implement value binding for `{Value}`",
    note = "add a `GpuiComponentValueBinding<T>` impl inside `component_shape!`, or derive with `value_binding` and a matching state binding"
)]
pub trait GpuiComponentValueBinding<Value>: GpuiComponentShape
where
    Self::State: gpui::EventEmitter<Self::Event>,
{
    /// Event emitted by the component state.
    type Event: 'static;

    /// Seed component state from the current value.
    fn seed_value_binding_state(
        _state: &mut Self::State,
        _value: Option<&Value>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<'_, Self::State>,
    ) {
    }

    /// Convert an emitted component event into a normalized value change.
    fn value_change(state: &Self::State, event: &Self::Event) -> ValueChange<Value>;
}

/// Value-binding contract implemented by backing component state.
#[diagnostic::on_unimplemented(
    message = "GPUI component state `{Self}` does not implement value binding for `{Value}`",
    note = "implement `GpuiComponentStateValueBinding<T>` for the backing state"
)]
pub trait GpuiComponentStateValueBinding<Value>: gpui::EventEmitter<Self::Event> {
    /// Event emitted by the backing component state.
    type Event: 'static;

    /// Seed component state from the current value.
    fn seed_value_binding_state(
        _state: &mut Self,
        _value: Option<&Value>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<'_, Self>,
    ) where
        Self: Sized,
    {
    }

    /// Convert an emitted component event into a normalized value change.
    fn value_change(state: &Self, event: &Self::Event) -> ValueChange<Value>;
}

/// State type for a GPUI component shape.
pub type GpuiComponentStateOf<Shape> = <Shape as GpuiComponentShape>::State;

/// Event type for a value-bound GPUI component shape and value.
pub type GpuiComponentEventOf<Shape, Value> = <Shape as GpuiComponentValueBinding<Value>>::Event;

/// Build component state from a configured shape builder.
pub fn build_component_shape<Shape, Builder>(
    builder: Builder,
    window: &mut gpui::Window,
    cx: &mut gpui::Context<'_, GpuiComponentStateOf<Shape>>,
) -> GpuiComponentStateOf<Shape>
where
    Shape: GpuiComponentShape,
    Builder: GpuiComponentShapeBuilder<Shape>,
{
    builder.build(window, cx)
}

/// Seed component state from the current value without spelling out the
/// associated-type projection at every generated call site.
pub fn seed_value_binding_state<Shape, Value>(
    state: &mut GpuiComponentStateOf<Shape>,
    value: Option<&Value>,
    window: &mut gpui::Window,
    cx: &mut gpui::Context<'_, GpuiComponentStateOf<Shape>>,
) where
    Shape: GpuiComponentValueBinding<Value>,
    GpuiComponentStateOf<Shape>: gpui::EventEmitter<GpuiComponentEventOf<Shape, Value>>,
{
    Shape::seed_value_binding_state(state, value, window, cx);
}

/// Convert a component event into a value change without repeating UFCS
/// projections in generated code.
pub fn value_change<Shape, Value>(
    state: &GpuiComponentStateOf<Shape>,
    event: &GpuiComponentEventOf<Shape, Value>,
) -> ValueChange<Value>
where
    Shape: GpuiComponentValueBinding<Value>,
    GpuiComponentStateOf<Shape>: gpui::EventEmitter<GpuiComponentEventOf<Shape, Value>>,
{
    Shape::value_change(state, event)
}

#[cfg(test)]
mod tests {
    use super::{
        ComponentShapeMetadata, DefaultGpuiComponentShapeBuilder, GpuiComponentRender,
        GpuiComponentShape, GpuiComponentShapeBuilder, GpuiComponentStateValueBinding,
        GpuiComponentValueBinding, NoGpuiRenderComponent, ValueChange, build_component_shape,
        seed_value_binding_state, value_change,
    };

    #[derive(Debug, Default, Eq, PartialEq)]
    struct TestState {
        value: Option<u32>,
    }

    impl gpui::Render for TestState {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<'_, Self>,
        ) -> impl gpui::IntoElement {
            gpui::div()
        }
    }

    struct TestEvent(Option<u32>);

    impl gpui::EventEmitter<TestEvent> for TestState {}

    struct TestShape;

    impl ComponentShapeMetadata for TestShape {}

    impl GpuiComponentShape for TestShape {
        type State = TestState;
        type RenderComponent = NoGpuiRenderComponent;

        fn new(
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<'_, Self::State>,
        ) -> Self::State {
            TestState { value: Some(1) }
        }
    }

    impl GpuiComponentValueBinding<u32> for TestShape {
        type Event = TestEvent;

        fn value_change(_state: &Self::State, event: &Self::Event) -> ValueChange<u32> {
            match event.0 {
                Some(value) => ValueChange::Set(value),
                None => ValueChange::Clear,
            }
        }
    }

    impl GpuiComponentStateValueBinding<u32> for TestState {
        type Event = TestEvent;

        fn value_change(_state: &Self, event: &Self::Event) -> ValueChange<u32> {
            match event.0 {
                Some(value) => ValueChange::Set(value),
                None => ValueChange::Clear,
            }
        }
    }

    struct ConfiguredBuilder(u32);

    impl GpuiComponentShapeBuilder<TestShape> for ConfiguredBuilder {
        fn build(
            self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<'_, TestState>,
        ) -> TestState {
            TestState {
                value: Some(self.0),
            }
        }
    }

    #[test]
    fn marker_and_default_builder_metadata_are_stable() {
        assert_eq!(
            DefaultGpuiComponentShapeBuilder::<()>::new(),
            DefaultGpuiComponentShapeBuilder::default()
        );
        const {
            assert!(!<NoGpuiRenderComponent as GpuiComponentRender<()>>::RENDERS);
        }
    }

    #[test]
    fn runtime_helpers_dispatch_through_shape_contracts() {
        let mut app = gpui::TestApp::new();
        let mut window = app.open_window(|window, cx| {
            build_component_shape::<TestShape, _>(
                DefaultGpuiComponentShapeBuilder::new(),
                window,
                cx,
            )
        });

        assert_eq!(window.read(|state, _| state.value), Some(1));
        let root = window.root();
        let _render = NoGpuiRenderComponent::new(&root);

        window.update(|state, window, cx| {
            seed_value_binding_state::<TestShape, u32>(state, Some(&7), window, cx);
            assert_eq!(state.value, Some(1), "the default seed hook is a no-op");
            assert_eq!(
                value_change::<TestShape, u32>(state, &TestEvent(Some(9))),
                ValueChange::Set(9)
            );
            assert_eq!(
                <TestState as GpuiComponentStateValueBinding<u32>>::value_change(
                    state,
                    &TestEvent(None),
                ),
                ValueChange::Clear
            );
            <TestState as GpuiComponentStateValueBinding<u32>>::seed_value_binding_state(
                state,
                Some(&11),
                window,
                cx,
            );
            assert_eq!(
                state.value,
                Some(1),
                "the state default seed hook is a no-op"
            );

            let configured =
                build_component_shape::<TestShape, _>(ConfiguredBuilder(42), window, cx);
            assert_eq!(configured.value, Some(42));
        });
    }
}
