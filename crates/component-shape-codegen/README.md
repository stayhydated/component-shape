# component-shape-codegen

[![Codecov: component-shape-codegen][codecov-badge]][codecov]
[![crates.io: component-shape-codegen][crate-badge]][crate]

`component-shape-codegen` provides token-generation helpers for procedural macro
and generator authors in the [component-shape project][project]. Use it directly
when generating shape metadata, normalizing paths, or substituting inferred Rust
types. Application code normally uses [`component-shape-gpui`][gpui] or
[`component-shape-mcp`][mcp].

Requires Rust 1.99 or newer. Its syntax helpers use Syn 3.

## Example

Normalize a shape suffix for a generated field helper:

```rust
use component_shape_codegen::component_suffix_from_suffix;

fn main() {
    let suffix = component_suffix_from_suffix("email", "EmailInput");
    assert_eq!(suffix.as_deref(), Some("input"));
    assert_eq!(component_suffix_from_suffix("tags", "tags"), None);
}
```

The repeated field prefix is removed; an identical field and suffix returns
`None` so the generator can choose its own fallback. See the [API reference][api]
for token, span, import, and shape-resolution helpers.

[codecov-badge]: https://codecov.io/gh/stayhydated/component-shape/branch/master/graph/badge.svg?component=component-shape-codegen
[codecov]: https://codecov.io/gh/stayhydated/component-shape
[crate-badge]: https://img.shields.io/crates/v/component-shape-codegen.svg?label=component-shape-codegen
[crate]: https://crates.io/crates/component-shape-codegen
[project]: https://github.com/stayhydated/component-shape
[gpui]: https://crates.io/crates/component-shape-gpui
[mcp]: https://crates.io/crates/component-shape-mcp
[api]: https://docs.rs/component-shape-codegen
