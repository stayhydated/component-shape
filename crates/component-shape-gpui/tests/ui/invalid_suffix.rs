use component_shape_gpui::component_shape;

struct InputState;

component_shape! {
    pub struct InvalidSuffix {
        type State = InputState;
        value = String;
        field_suffix = "input-field";
    }
}

fn main() {}
