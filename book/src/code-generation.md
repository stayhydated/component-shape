# Code generation

Use `component-shape-codegen` directly when writing a procedural macro or
generator that consumes component shapes. Applications declaring GPUI shapes or
MCP inputs should use the macros re-exported by `component-shape-gpui` or
`component-shape-mcp` instead.

Requires Rust 1.99 or newer. The parsing helpers accept Syn 3 syntax trees.

## Resolve a field's shape

Create `ShapeOptions` from a parsed shape path, then call `resolve` with the
source field name and type. Resolution replaces inferred `_` types and computes
the generated helper suffix:

```rust
# extern crate component_shape_codegen;
# extern crate syn;
use component_shape_codegen::ShapeOptions;

let options = ShapeOptions::from_shape(syn::parse_quote!(crate::EmailInputShape<_>));
let resolved = options.resolve("email".to_owned(), syn::parse_quote!(String));

assert_eq!(resolved.component_suffix(), "input");
assert_eq!(resolved.field_name(), "email");
```

The resolved shape is `crate::EmailInputShape<String>`. Use `shape()` and
`field_type()` when emitting the consumer's trait checks or metadata.

For a configured shape such as `Select::<_>.searchable(true)`, use
`ShapeOptions::from_constructor_expr` and propagate its `syn::Result`. The
resolved `constructor_expr()` retains expression-position turbofish syntax and
replaces inferred types. An associated constructor call such as
`Select::<_>::searchable(true)` is not accepted as this declaration syntax.

## Preserve paths and diagnostics

Use `shape_path_from_expr` or `shape_path_from_type_path` for plain shape paths.
Both reject qualified self syntax such as `<T as Trait>::Output`; dropping its
self type would change what the generated path names. Use
`shape_path_from_constructor_expr` when configured dot-call chains are allowed.

`normalize_shape_path` removes expression-position turbofish markers from path
arguments for metadata comparisons. It does not resolve a path to a Rust type.
Keep errors attached to the caller's syntax and propagate them through the
macro's diagnostic path instead of replacing invalid input with a default shape.

The [API reference](https://docs.rs/component-shape-codegen/) covers imports,
assertion tokens, MCP metadata, and span helpers. For framework-neutral metadata
contracts, see [Framework-neutral shapes](framework-neutral-shapes.md).
