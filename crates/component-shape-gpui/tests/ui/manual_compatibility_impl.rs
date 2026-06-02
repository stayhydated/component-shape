use component_shape_gpui::{GpuiComponentShapeFor, component_shape};

pub struct InputState;

component_shape! {
    pub struct ManualCompatibilityShape {
        type State = InputState;
        value = String;

        impl GpuiComponentShapeFor<String> for ManualCompatibilityShape {}
    }
}

fn main() {}
