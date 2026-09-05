# GPUI shapes

`component-shape-gpui` connects framework-neutral metadata to GPUI state,
construction, optional rendering, configured builders, and value binding.

## Declare an owned component

Derive `GpuiComponentShape` when your crate owns the rendered component and
its backing state:

```rust
use component_shape_gpui::GpuiComponentShape;

pub struct TextInputState;

impl TextInputState {
    pub fn new(
        _window: &mut gpui_kit::Window,
        _cx: &mut gpui_kit::Context<'_, Self>,
    ) -> Self {
        Self
    }
}

#[derive(GpuiComponentShape)]
#[gpui_component_shape(value = String, field_suffix = "input")]
pub struct TextInput;

impl TextInput {
    pub fn new(_state: &gpui_kit::Entity<TextInputState>) -> impl gpui_kit::IntoElement {
        gpui_kit::div()
    }
}
```

The derive infers `TextInputState` from the component name. Set
`state = path::State` for another name or location. By default, generated
construction calls `State::new(window, cx)`.

## Wrap external component types

Use `component_shape!` when the state or rendered component belongs to another
crate. The local wrapper owns the generated implementations and avoids
orphan-rule conflicts.

```rust
component_shape_gpui::component_shape! {
    pub struct EmailInputShape {
        state = gpui_kit::component::input::InputState;
        component = gpui_kit::component::input::Input;
        value = String;
        field_suffix = "input";
    }
}
```

Omit `component = ...` for a metadata-only shape. That shape publishes no
render component.

## Publish value and construction behavior

- Add `value = T` or `values(...)` for supported values.
- Add `value_binding` when the shape delegates through
  `GpuiComponentStateValueBinding<T>`.
- Put a `GpuiComponentValueBinding<T>` implementation inside
  `component_shape!` when the wrapper owns the binding behavior.
- Implement both `ComponentShapeFor<T>` and `GpuiComponentShapeFor<T>` for a
  hand-written compatibility pair.
- Use `GpuiComponentShapeBuilder<Shape>` for configuration selected at the
  field-use site. Use `DefaultGpuiComponentShapeBuilder<Shape>` for the
  shape's normal constructor.

The derive and function-like macro implement `DeclaredGpuiComponentShape` and
the framework-neutral `DeclaredComponentShape`. Consumers can require those
markers when they accept only macro-declared shapes.

Common Rust values infer coarse `McpInput` metadata. Add an explicit
`mcp_input = ...` only for a known custom wire form; use
[`component-shape-mcp`](mcp-integration.md) for precise schema and decoding.
