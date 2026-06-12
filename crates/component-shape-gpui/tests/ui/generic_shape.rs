use std::str::FromStr;

use component_shape_gpui::{ComponentShapeFor, GpuiComponentShapeFor, component_shape};

pub struct InputState;

impl InputState {
    fn new(_window: &mut gpui::Window, _cx: &mut gpui::Context<'_, Self>) -> Self {
        Self
    }
}

component_shape! {
    pub struct InputShape<T = String>
    where
        T: FromStr + ToString + 'static,
    {
        type State = InputState;
        value = T;
    }
}

fn assert_string_shape<Shape>()
where
    Shape: ComponentShapeFor<String> + GpuiComponentShapeFor<String>,
{
}

fn main() {
    assert_string_shape::<InputShape>();
    assert_string_shape::<InputShape<String>>();
}
