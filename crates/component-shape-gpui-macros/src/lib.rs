mod derives;

use proc_macro::TokenStream;
use proc_macro_error2::proc_macro_error;

/// Derive macro for GPUI component shape metadata.
///
/// Requires `#[gpui_component_shape(state = ...)]`. By default it calls
/// `<State>::new(window, cx)`. Override the constructor with `new = ...`, a
/// closure, or a direct constructor expression.
#[proc_macro_derive(GpuiComponentShape, attributes(gpui_component_shape))]
#[proc_macro_error]
pub fn derive_gpui_component_shape(input: TokenStream) -> TokenStream {
    derives::component_shape_state::from(input)
}

/// Function-like macro for declaring a local GPUI component shape around
/// external component/state types.
#[proc_macro]
#[proc_macro_error]
pub fn component_shape(input: TokenStream) -> TokenStream {
    derives::component_shape::function(input)
}
