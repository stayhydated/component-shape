# component-shape-gpui-macros

[![Codecov: component-shape-gpui-macros][codecov-badge]][codecov]
[![crates.io: component-shape-gpui-macros][crate-badge]][crate]

`component-shape-gpui-macros` implements `GpuiComponentShape` and
`component_shape!` for the [component-shape project][project]. Applications should
use the re-exports from [`component-shape-gpui`][gpui], which also supplies the
runtime traits required by generated code. A direct dependency is useful only
when building a facade that forwards these macros and the GPUI contracts.

Requires Rust 1.99 or newer.

The [macro API reference][api] describes declaration syntax.

[codecov-badge]: https://codecov.io/gh/stayhydated/component-shape/branch/master/graph/badge.svg?component=component-shape-gpui-macros
[codecov]: https://codecov.io/gh/stayhydated/component-shape
[crate-badge]: https://img.shields.io/crates/v/component-shape-gpui-macros.svg?label=component-shape-gpui-macros
[crate]: https://crates.io/crates/component-shape-gpui-macros
[project]: https://github.com/stayhydated/component-shape
[gpui]: https://crates.io/crates/component-shape-gpui
[api]: https://docs.rs/component-shape-gpui-macros
