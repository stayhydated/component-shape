use component_shape_gpui::{
    DefaultGpuiComponentShapeBuilder, GpuiComponentShape, GpuiComponentShapeBuilder,
    build_component_shape, component_shape,
};

pub struct SelectState {
    searchable: bool,
}

impl SelectState {
    fn new(_window: &mut gpui::Window, _cx: &mut gpui::Context<'_, Self>) -> Self {
        Self { searchable: false }
    }
}

component_shape! {
    pub struct SelectShape {
        type State = SelectState;
        value = String;
    }
}

pub struct SelectArgs {
    searchable: bool,
}

impl SelectShape {
    pub fn searchable(searchable: bool) -> SelectArgs {
        SelectArgs { searchable }
    }

    pub fn from(args: SelectArgs) -> SelectArgs {
        args
    }
}

impl GpuiComponentShapeBuilder<SelectShape> for SelectArgs {
    fn build(
        self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<'_, SelectState>,
    ) -> SelectState {
        SelectState {
            searchable: self.searchable,
        }
    }
}

fn default_select(
    window: &mut gpui::Window,
    cx: &mut gpui::Context<'_, <SelectShape as GpuiComponentShape>::State>,
) -> SelectState {
    build_component_shape::<SelectShape, _>(
        DefaultGpuiComponentShapeBuilder::<SelectShape>::new(),
        window,
        cx,
    )
}

fn searchable_select(
    window: &mut gpui::Window,
    cx: &mut gpui::Context<'_, <SelectShape as GpuiComponentShape>::State>,
) -> SelectState {
    build_component_shape::<SelectShape, _>(SelectShape::searchable(true), window, cx)
}

fn configured_select(
    window: &mut gpui::Window,
    cx: &mut gpui::Context<'_, <SelectShape as GpuiComponentShape>::State>,
) -> SelectState {
    build_component_shape::<SelectShape, _>(
        SelectShape::from(SelectArgs { searchable: true }),
        window,
        cx,
    )
}

fn main() {
    let _ = SelectState { searchable: false }.searchable;
    let _ = default_select;
    let _ = searchable_select;
    let _ = configured_select;
}
