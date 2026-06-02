use component_shape_gpui::component_shape;

struct InputState;

component_shape! {
    pub struct MissingValueBindingImpl {
        type State = InputState;
        value = String;
        value_binding;
    }
}

fn main() {}
