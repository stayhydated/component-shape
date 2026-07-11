use component_shape_gpui::{
    ComponentShapeFor, ComponentShapeMetadata as _, ComponentSuffix, GpuiComponentShape,
    GpuiComponentShapeFor, GpuiComponentStateValueBinding, GpuiComponentValueBinding,
    McpInputShape, McpPrimitiveKind, ValueChange,
};

pub struct InferredInputState;

impl InferredInputState {
    fn new(_window: &mut gpui::Window, _cx: &mut gpui::Context<'_, Self>) -> Self {
        Self
    }
}

#[derive(Clone, Debug)]
pub enum InputEvent {
    Change(String),
}

impl gpui::EventEmitter<InputEvent> for InferredInputState {}

impl GpuiComponentStateValueBinding<String> for InferredInputState {
    type Event = InputEvent;

    fn value_change(_state: &Self, event: &Self::Event) -> ValueChange<String> {
        match event {
            InputEvent::Change(value) => ValueChange::Set(value.clone()),
        }
    }
}

#[derive(GpuiComponentShape)]
#[gpui_component_shape(
    value = String,
    field_suffix = "input",
    value_binding
)]
pub struct InferredInput;

impl InferredInput {
    pub fn new(_entity: &gpui::Entity<InferredInputState>) -> impl gpui::IntoElement {
        gpui::div()
    }
}

fn assert_string_shape()
where
    InferredInput: ComponentShapeFor<String>
        + GpuiComponentShapeFor<String>
        + GpuiComponentValueBinding<String>,
{
}

fn assert_state_type<Shape>()
where
    Shape: GpuiComponentShape<State = InferredInputState>,
{
}

fn main() {
    assert_string_shape();
    assert_state_type::<InferredInput>();
    assert_eq!(
        InferredInput::PROTOTYPING
            .field_suffix
            .map(ComponentSuffix::as_str),
        Some("input")
    );
    assert_eq!(
        <InferredInput as component_shape_gpui::ComponentShapeMetadata>::MCP_INPUT.input_shape(),
        McpInputShape::Scalar(McpPrimitiveKind::String)
    );
}
