use std::marker::PhantomData;
use std::str::FromStr;

use component_shape::ComponentShapeMetadata as _;
use component_shape::{RenderCapability, ValueBindingCapability};
use component_shape_gpui::{GpuiComponentRender, GpuiComponentShape, component_shape};

pub struct InputState<T>(PhantomData<T>);

impl<T> InputState<T> {
    fn new(_window: &mut gpui::Window, _cx: &mut gpui::Context<'_, Self>) -> Self {
        Self(PhantomData)
    }
}

pub struct GenericInput<T>(PhantomData<T>);

impl<T> GenericInput<InputState<T>> {
    pub fn new(_entity: &gpui::Entity<InputState<T>>) -> impl gpui::IntoElement {
        gpui::div()
    }
}

component_shape! {
    pub struct InputShape<T = String>
    where
        T: FromStr + ToString + 'static,
    {
        type State = InputState<T>;
        component = GenericInput<_>;
        value = T;
    }
}

fn main() {
    assert!(<InputShape as GpuiComponentShape>::RenderComponent::RENDERS);
    assert_eq!(
        InputShape::<String>::CAPABILITIES.render(),
        RenderCapability::Component
    );
    assert_eq!(
        InputShape::<String>::CAPABILITIES.value_binding(),
        ValueBindingCapability::None
    );
}
