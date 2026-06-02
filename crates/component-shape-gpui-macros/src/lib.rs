mod derives;

use proc_macro::TokenStream;
use proc_macro_error2::proc_macro_error;

/// Derive macro for GPUI component shape metadata.
///
/// When `#[gpui_component_shape(...)]` is present, `state = ...` may be omitted
/// if the backing state follows the `ComponentState` naming convention. By
/// default the derive calls `<State>::new(window, cx)`. Override the constructor
/// with `new = ...`, a closure, or a direct constructor expression.
///
/// `value = ...` or `values(...)` publish exact supported value types. For
/// `value_binding` shapes, value compatibility may also be inferred from
/// `State: GpuiComponentStateValueBinding<T>`.
#[proc_macro_derive(GpuiComponentShape, attributes(gpui_component_shape))]
#[proc_macro_error]
pub fn derive_gpui_component_shape(input: TokenStream) -> TokenStream {
    derives::component_shape_state::from(input)
}

/// Function-like macro for declaring a local GPUI component shape around
/// external component/state types.
///
/// The backing state may be declared as `type State = ...;` or `state = ...;`.
/// When no explicit `value = ...` or `values(...)` metadata is present, a
/// nested `GpuiComponentValueBinding<T>` impl publishes `T` as a supported value
/// and enables value-binding metadata.
#[proc_macro]
#[proc_macro_error]
pub fn component_shape(input: TokenStream) -> TokenStream {
    derives::component_shape::function(input)
}
