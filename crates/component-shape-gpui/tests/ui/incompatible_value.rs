use component_shape_gpui::{GpuiComponentShapeFor, component_shape};

pub struct InputState;

impl InputState {
    fn new(_window: &mut gpui::Window, _cx: &mut gpui::Context<'_, Self>) -> Self {
        Self
    }
}

component_shape! {
    pub struct StringShape {
        type State = InputState;
        value = String;
    }
}

fn assert_shape_for_u64<Shape>()
where
    Shape: GpuiComponentShapeFor<u64>,
{
}

fn main() {
    assert_shape_for_u64::<StringShape>();
}
