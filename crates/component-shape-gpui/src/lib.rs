//! GPUI-specific component shape runtime contracts.

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
pub trait GpuiComponentShape: component_shape::ComponentShapeMetadata {
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
/// with a configuration expression, such as `Select::<_>::searchable(true)`.
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
pub trait DeclaredGpuiComponentShape: GpuiComponentShape {}

/// Marker that a GPUI component shape supports a value type.
#[diagnostic::on_unimplemented(
    message = "GPUI component shape `{Self}` is not compatible with value `{Value}`",
    note = "declare `value = {Value}` or include `{Value}` in `values(...)`, or choose a component shape whose value type matches the field"
)]
pub trait GpuiComponentShapeFor<Value>: GpuiComponentShape {}

/// Optional value-binding contract for GPUI component shapes.
#[diagnostic::on_unimplemented(
    message = "GPUI component shape `{Self}` does not implement value binding for `{Value}`",
    note = "add `value_binding` shape metadata with a `GpuiComponentValueBinding<T>` impl"
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
    fn value_change(
        state: &Self::State,
        event: &Self::Event,
    ) -> component_shape::ValueChange<Value>;
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
    fn value_change(state: &Self, event: &Self::Event) -> component_shape::ValueChange<Value>;
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
) -> component_shape::ValueChange<Value>
where
    Shape: GpuiComponentValueBinding<Value>,
    GpuiComponentStateOf<Shape>: gpui::EventEmitter<GpuiComponentEventOf<Shape, Value>>,
{
    Shape::value_change(state, event)
}
