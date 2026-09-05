use component_shape_gpui::GpuiComponentShape;

pub struct TextInputState;

impl TextInputState {
    fn new(_window: &mut gpui_kit::Window, _cx: &mut gpui_kit::Context<'_, Self>) -> Self {
        Self
    }
}

#[derive(GpuiComponentShape)]
#[gpui_component_shape(
    state = TextInputState,
    value = String,
    mcp_input = strings
)]
pub struct TextInput;

fn main() {}
