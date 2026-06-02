use component_shape::{ComponentShapeMetadata as _, ValueChange};
use component_shape::{RenderCapability, ValueBindingCapability};
use component_shape_gpui::{
    GpuiComponentShapeFor, GpuiComponentValueBinding, component_shape,
};

pub struct InputState;

impl InputState {
    fn new(_window: &mut gpui::Window, _cx: &mut gpui::Context<'_, Self>) -> Self {
        Self
    }
}

#[derive(Clone, Debug)]
pub enum InputEvent {
    Change(String),
}

impl gpui::EventEmitter<InputEvent> for InputState {}

component_shape! {
    pub struct InputShape {
        state = InputState;

        impl GpuiComponentValueBinding<String> for InputShape {
            type Event = InputEvent;

            fn value_change(_state: &Self::State, event: &Self::Event) -> ValueChange<String> {
                match event {
                    InputEvent::Change(value) => ValueChange::Set(value.clone()),
                }
            }
        }
    }
}

fn assert_string_shape()
where
    InputShape: GpuiComponentShapeFor<String> + GpuiComponentValueBinding<String>,
{
}

fn main() {
    assert_string_shape();
    assert_eq!(InputShape::CAPABILITIES.render(), RenderCapability::None);
    assert_eq!(
        InputShape::CAPABILITIES.value_binding(),
        ValueBindingCapability::Inherited
    );
}
