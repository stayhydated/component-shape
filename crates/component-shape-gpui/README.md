# component-shape-gpui

[![Codecov: component-shape-gpui][codecov-badge]][codecov]
[![crates.io: component-shape-gpui][crate-badge]][crate]

`component-shape-gpui` is the GPUI facade in the [component-shape project][project]
for declaring component shapes, constructing their state, rendering them, and
binding value changes. Depend on it directly for GPUI integrations; it re-exports
the shared metadata and both declaration macros.

Requires Rust 1.99 or newer and uses GPUI Kit 0.7.

## Example

This state-only declaration is adapted from the maintained [compile fixture][example].
It publishes `String` compatibility and a stable `input` suffix without opening a
GPUI window. Omitting `component` selects the non-rendering contract.

```rust
use component_shape_gpui::{ComponentShapeFor, McpInput, component_shape};

pub struct InputState;

impl InputState {
    fn new(_window: &mut gpui_kit::Window, _cx: &mut gpui_kit::Context<'_, Self>) -> Self {
        Self
    }
}

component_shape! {
    pub struct InputShape {
        state = InputState;
        value = String;
        field_suffix = "input";
    }
}

fn main() {
    assert_eq!(<InputShape as ComponentShapeFor<String>>::MCP_INPUT, McpInput::string());
}
```

Use `#[derive(GpuiComponentShape)]` for an owned rendered component. The
[API reference][api] documents builders, render contracts, and value binding.

[codecov-badge]: https://codecov.io/gh/stayhydated/component-shape/branch/master/graph/badge.svg?component=component-shape-gpui
[codecov]: https://codecov.io/gh/stayhydated/component-shape
[crate-badge]: https://img.shields.io/crates/v/component-shape-gpui.svg?label=component-shape-gpui
[crate]: https://crates.io/crates/component-shape-gpui
[project]: https://github.com/stayhydated/component-shape
[example]: https://github.com/stayhydated/component-shape/blob/master/crates/component-shape-gpui/tests/ui/basic_shape.rs
[api]: https://docs.rs/component-shape-gpui
