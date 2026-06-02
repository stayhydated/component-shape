use component_shape::ComponentShapeMetadata as _;
use component_shape_gpui::{
    DeclaredGpuiComponentShape, GpuiComponentShape, GpuiComponentShapeFor, component_shape,
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
    Shape: GpuiComponentShape + DeclaredGpuiComponentShape + GpuiComponentShapeFor<String>,
{
}

fn main() {
    assert_shape::<InputShape>();
    assert_eq!(
        InputShape::PROTOTYPING
            .field_suffix
            .map(component_shape::ComponentSuffix::as_str),
        Some("input")
    );
}
