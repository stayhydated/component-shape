use component_shape_gpui::component_shape;

struct InputState;

component_shape! {
    pub struct MissingValue {
        type State = InputState;
    }
}

fn main() {}
