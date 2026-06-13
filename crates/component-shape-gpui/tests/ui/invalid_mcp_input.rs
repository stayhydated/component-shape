use component_shape_gpui::GpuiComponentShape;

pub struct TextInputState;

impl TextInputState {
    fn new(_window: &mut gpui::Window, _cx: &mut gpui::Context<'_, Self>) -> Self {
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
