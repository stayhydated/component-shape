---
name: use-component-shape-gpui
description: "Use when Codex needs to add, review, or refactor GPUI component shape declarations with component_shape_gpui::GpuiComponentShape, component_shape_gpui::component_shape!, GPUI render contracts, value-binding metadata, or component-shape-gpui macro syntax."
---

# Use Component Shape GPUI

## Scope Boundary

Use this skill for GPUI-specific component shape declarations and runtime
contracts in the `component-shape-gpui` surface. It covers the
`GpuiComponentShape` derive, the function-like `component_shape!` macro, render
component metadata, constructor metadata, value compatibility, value-binding
declarations, and MCP input metadata.

Use `use-component-shape` for framework-neutral metadata such as
`ComponentShapeMetadata`, capabilities, suffix validation, syntax wrappers, and
generic value-change concepts.

Use downstream integration guidance, such as `use-gpui-form-component-shapes`,
when the task is about a form framework consuming the shape rather than
declaring the shape itself.

This reusable skill does not cover proc-macro implementation internals,
trybuild fixture maintenance, or contributor-only implementation docs. Use the
workspace `AGENTS.md` and crate docs for those tasks.

## Decision Rule

First classify component ownership:

- Owned rendered component: when the crate owns the rendered component and
  backing state, derive `component_shape_gpui::GpuiComponentShape` on the
  rendered component with `state = ...` when needed.
- External component/state pair: when the state type or rendered component
  lives in another crate, declare a local wrapper shape with
  `component_shape_gpui::component_shape!`. This avoids orphan-rule problems and
  gives the local crate a type that owns the shape implementations.
- Existing shape: when a suitable reusable shape already exists, use it
  directly instead of wrapping it again.

If a prompt says "functional macro" or "functional! macro", interpret that as
the function-like `component_shape_gpui::component_shape!` proc macro unless
the codebase has a different local macro with that exact name.

## Owned Component Pattern

Use `#[derive(GpuiComponentShape)]` when the rendered component type is local:

```rust
use component_shape_gpui::GpuiComponentShape;

#[derive(GpuiComponentShape)]
#[gpui_component_shape(value = Vec<String>, field_suffix = "input")]
pub struct TagsInput {
    state: gpui::Entity<TagsInputState>,
}

pub struct TagsInputState;
```

The rendered component type must provide a constructor compatible with the
generated render contract, commonly:

```rust
impl TagsInput {
    pub fn new(state: &gpui::Entity<TagsInputState>) -> impl gpui::IntoElement {
        Self {
            state: state.clone(),
        }
    }
}
```

Metadata rules:

- `state = ...` is optional when the backing state is a same-module type named
  after the component, such as `TagsInputState`; add it for different names or
  paths.
- Omit `new` when the state has `State::new(window, cx)`.
- Use `new = some_function` or `new = |window, cx| ...` when the macro should
  pass `(window, cx)` for you.
- Use a full constructor expression such as
  `new = Self::with_mode(window, cx, Mode::Compact)` when the expression should
  be emitted as written.
- Add `component = ...` only when generated metadata should use a path-like
  render component type different from the derived type.
- Add `value = ...` or `values(...)` once for each supported value type unless
  value compatibility should be inferred from value-binding declarations or
  implemented manually.
- Add `value_binding` when the derived shape should delegate value binding
  through the backing state's value-binding implementation.
- Add `field_suffix = "..."` when downstream prototyping or generators need a
  stable suffix for generated identifiers.
- Common MCP input metadata is inferred from unambiguous declared values such
  as `String`, booleans, numbers, dates, `Vec<T>`, set-like primitive
  collections, fixed arrays, `component_shape_mcp::McpRange<T>`, or
  `(Option<T>, Option<T>)` ranges. `Vec<T>` and fixed arrays publish list
  metadata; set-like collections publish set metadata. Each generated
  `ComponentShapeFor<Value>` impl carries the value-specific MCP metadata, and
  shape-level MCP metadata is emitted only when all declared values agree.
  Manual `ComponentShapeFor<Value>` impls inherit shape-level MCP metadata
  unless they override the value-specific `MCP_INPUT`.
  For custom or ambiguous wire schemas, use the downstream MCP integration's
  typed schema derive or a manual decode/schema implementation.

## External State Pattern

Use `component_shape_gpui::component_shape!` when wrapping state or rendered
components from another crate:

```rust
component_shape_gpui::component_shape! {
    pub struct EmailInputShape {
        state = gpui_component::input::InputState;
        component = gpui_component::input::Input;
        value = String;
        field_suffix = "input";
    }
}
```

For generic external wrappers, put the generic parameters and bounds on the
local shape:

```rust
component_shape_gpui::component_shape! {
    pub struct Input<T = String>
    where
        T: std::str::FromStr + ToString + 'static,
    {
        state = gpui_component::input::InputState;
        new = |window, cx| gpui_component::input::InputState::new(window, cx)
            .validate(|value, _| value.parse::<T>().is_ok());
        component = gpui_component::input::Input;
        field_suffix = "input";

        impl<T> component_shape_gpui::GpuiComponentValueBinding<T> for Input<T>
        where
            T: std::str::FromStr + ToString + 'static,
        {
            type Event = gpui_component::input::InputEvent;
            /* seed_value_binding_state and value_change */
        }
    }
}
```

When no explicit value metadata is present, a nested
`GpuiComponentValueBinding<T>` impl can publish both `T` compatibility and
value-binding metadata. Do not duplicate `value = T;` and `value_binding;`
entries unless the crate's current macro contract requires explicit metadata.

## Value Compatibility

A shape can advertise support for form-side values through:

- explicit `value = ...` metadata,
- explicit `values(...)` metadata,
- generated compatibility from `value_binding`,
- manual `GpuiComponentShapeFor<Value>` implementations.

`GpuiComponentShapeFor<Value>` includes the framework-neutral
`ComponentShapeFor<Value>` contract, so manual GPUI compatibility impls must
also publish value-specific shape metadata.

Keep value compatibility separate from downstream storage policy. This crate
should declare which values a component can represent; downstream consumers
decide how required, optional, or missing values are stored.

## Suffix and Prototyping Metadata

Use `field_suffix = "..."` when generator output needs stable names for DOM
IDs, event handlers, helper methods, or field-local component roles. The suffix
should be a non-empty ASCII identifier suffix.

Prefer stable semantic suffixes such as `"input"`, `"select"`, or `"picker"`
over type-name-derived strings when generated names are part of a public or
checked output surface.

## Documentation Sync

When changing public GPUI shape behavior, keep these surfaces aligned:

- `component-shape-gpui` README or rustdoc for user-facing macro syntax,
- `component-shape-gpui` trybuild pass/fail tests when macro behavior changes,
- stderr fixtures only when diagnostic output intentionally changes,
- framework-neutral docs when shared metadata behavior changes,
- downstream integration skills only when their public workflow changes.

Do not duplicate downstream form-framework rules here; keep this skill focused
on declaring GPUI component shapes.
