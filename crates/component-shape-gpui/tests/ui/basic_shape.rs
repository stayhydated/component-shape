use component_shape_gpui::{
    ComponentShapeFor, ComponentShapeMetadata as _, ComponentSuffix, DeclaredComponentShape,
    DeclaredGpuiComponentShape, GpuiComponentShape, GpuiComponentShapeFor, McpInputShape,
    McpPrimitiveKind, component_shape,
};

pub struct InputState;

impl InputState {
    fn new(_window: &mut gpui::Window, _cx: &mut gpui::Context<'_, Self>) -> Self {
        Self
    }
}

component_shape! {
    pub struct InputShape {
        type State = InputState;
        value = String;
        field_suffix = "input";
    }
}

fn assert_shape<Shape>()
where
    Shape: GpuiComponentShape
        + DeclaredComponentShape
        + DeclaredGpuiComponentShape
        + ComponentShapeFor<String>
        + GpuiComponentShapeFor<String>,
{
}

fn main() {
    assert_shape::<InputShape>();
    assert_eq!(
        InputShape::PROTOTYPING
            .field_suffix
            .map(ComponentSuffix::as_str),
        Some("input")
    );
    assert_eq!(
        <InputShape as component_shape_gpui::ComponentShapeMetadata>::MCP_INPUT.input_shape(),
        McpInputShape::Scalar(McpPrimitiveKind::String)
    );
}
