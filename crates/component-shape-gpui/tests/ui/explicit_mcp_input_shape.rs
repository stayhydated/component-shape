use component_shape_gpui::{
    ComponentShapeFor, GpuiComponentShapeFor, McpInputShape, component_shape,
};

pub struct JsonEditorState;

impl JsonEditorState {
    fn new(_window: &mut gpui::Window, _cx: &mut gpui::Context<'_, Self>) -> Self {
        Self
    }
}

component_shape! {
    pub struct JsonEditorShape<T>
    where
        T: 'static,
    {
        state = JsonEditorState;
        value = T;
        mcp_input = object;
    }
}

fn assert_string_shape()
where
    JsonEditorShape<String>: ComponentShapeFor<String> + GpuiComponentShapeFor<String>,
{
}

fn main() {
    assert_string_shape();
    assert_eq!(
        <JsonEditorShape<String> as component_shape_gpui::ComponentShapeMetadata>::MCP_INPUT
            .input_shape(),
        McpInputShape::Object
    );
    assert_eq!(
        <JsonEditorShape<String> as ComponentShapeFor<String>>::MCP_INPUT.input_shape(),
        McpInputShape::Object
    );
}
