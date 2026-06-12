use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Result, Type, parse_macro_input};

use super::component_shape_metadata::{ComponentShapeMetadata, ShapeOption, crate_paths};

fn parse_meta(attrs: &[syn::Attribute]) -> Result<(ComponentShapeMetadata, bool)> {
    let mut shape = ComponentShapeMetadata::default();
    let mut has_shape_attr = false;

    for attr in attrs
        .iter()
        .filter(|attr| attr.path().is_ident("gpui_component_shape"))
    {
        has_shape_attr = true;
        attr.parse_nested_meta(|meta| ShapeOption::from_nested_meta(&meta)?.apply(&mut shape))?;
    }

    Ok((shape, has_shape_attr))
}

fn inferred_state_type(input: &DeriveInput) -> Result<Type> {
    let state_ident = format_ident!("{}State", input.ident);
    let (_, ty_generics, _) = input.generics.split_for_impl();

    if input.generics.params.is_empty() {
        Ok(syn::parse_quote!(#state_ident))
    } else {
        syn::parse2(quote! { #state_ident #ty_generics })
    }
}

fn expand(input: DeriveInput) -> Result<TokenStream> {
    let ident = &input.ident;
    let (meta, has_shape_attr) = parse_meta(&input.attrs)?;
    let state = match meta.state().cloned() {
        Some(state) => state,
        None if has_shape_attr => inferred_state_type(&input)?,
        None => {
            return Err(syn::Error::new_spanned(
                ident,
                "`#[derive(GpuiComponentShape)]` requires `#[gpui_component_shape(state = ...)]`; \
             use `component_shape_gpui::component_shape!` for wrapper shapes",
            ));
        },
    };
    let default_constructor = quote! { <#state>::new(window, cx) };
    let constructor_body = meta.constructor_body_or(default_constructor);
    let paths = crate_paths();
    let component_shape_crate = paths.component_shape;
    let component_shape_gpui_crate = paths.component_shape_gpui;
    let gpui_crate = paths.gpui;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let component_shape_for_impls = meta.value_impl_tokens(
        &component_shape_crate,
        &component_shape_gpui_crate,
        ident,
        &input.generics,
    );
    let state_value_binding_value_impls = meta.state_value_binding_value_impl_tokens(
        &component_shape_crate,
        &component_shape_gpui_crate,
        ident,
        &input.generics,
        &state,
    );
    let inferred_component_type: Type = if input.generics.params.is_empty() {
        syn::parse_quote!(#ident)
    } else {
        syn::parse2(quote! { #ident #ty_generics })?
    };
    let render_component = meta.component().unwrap_or(&inferred_component_type);
    let render_component_adapter_ident = format_ident!("__{}RenderComponent", ident);
    let (render_component_assoc, render_component_adapter) =
        ComponentShapeMetadata::render_component_tokens(
            &gpui_crate,
            &component_shape_gpui_crate,
            &input.vis,
            &render_component_adapter_ident,
            &state,
            Some(render_component),
            &input.generics,
        )?;
    let metadata_impl_items = meta.metadata_impl_tokens(&component_shape_crate);
    let binding_impl = if meta.has_value_binding() {
        let mut binding_generics = input.generics.clone();
        binding_generics
            .params
            .push(syn::parse_quote!(__GpuiComponentValueBindingValue));
        binding_generics
            .make_where_clause()
            .predicates
            .push(syn::parse_quote! {
                #state: #component_shape_gpui_crate::GpuiComponentStateValueBinding<
                    __GpuiComponentValueBindingValue
                >
            });
        let (binding_impl_generics, _, binding_where_clause) = binding_generics.split_for_impl();

        Some(quote! {
            impl #binding_impl_generics #component_shape_gpui_crate::GpuiComponentValueBinding<
                __GpuiComponentValueBindingValue
            > for #ident #ty_generics
                #binding_where_clause
            {
                type Event =
                    <#state as #component_shape_gpui_crate::GpuiComponentStateValueBinding<
                        __GpuiComponentValueBindingValue
                    >>::Event;

                fn seed_value_binding_state(
                    state: &mut Self::State,
                    value: Option<&__GpuiComponentValueBindingValue>,
                    window: &mut #gpui_crate::Window,
                    cx: &mut #gpui_crate::Context<'_, Self::State>,
                ) {
                    <#state as #component_shape_gpui_crate::GpuiComponentStateValueBinding<
                        __GpuiComponentValueBindingValue
                    >>::seed_value_binding_state(state, value, window, cx);
                }

                fn value_change(
                    state: &Self::State,
                    event: &Self::Event,
                ) -> #component_shape_crate::ValueChange<__GpuiComponentValueBindingValue> {
                    <#state as #component_shape_gpui_crate::GpuiComponentStateValueBinding<
                        __GpuiComponentValueBindingValue
                    >>::value_change(state, event)
                }
            }
        })
    } else {
        None
    };

    Ok(quote! {
        impl #impl_generics #component_shape_crate::ComponentShapeMetadata
            for #ident #ty_generics
            #where_clause
        {
            #metadata_impl_items
        }

        impl #impl_generics #component_shape_gpui_crate::GpuiComponentShape
            for #ident #ty_generics
            #where_clause
        {
            type State = #state;
            #render_component_assoc

            fn new(
                window: &mut #gpui_crate::Window,
                cx: &mut #gpui_crate::Context<'_, Self::State>,
            ) -> Self::State {
                #constructor_body
            }
        }

        #render_component_adapter

        impl #impl_generics #component_shape_crate::DeclaredComponentShape
            for #ident #ty_generics
            #where_clause
        {
        }

        impl #impl_generics #component_shape_gpui_crate::DeclaredGpuiComponentShape
            for #ident #ty_generics
            #where_clause
        {
        }

        #binding_impl

        #state_value_binding_value_impls
        #component_shape_for_impls
    })
}

pub fn from(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

#[cfg(test)]
mod tests {
    use super::expand;
    use quote::quote;
    use syn::DeriveInput;

    fn compact_tokens(tokens: &str) -> String {
        tokens.chars().filter(|c| !c.is_whitespace()).collect()
    }

    #[test]
    fn derive_requires_state() {
        let input: DeriveInput = syn::parse2(quote! {
            #[derive(GpuiComponentShape)]
            struct TagsInput;
        })
        .unwrap();

        let err = expand(input).unwrap_err();

        assert!(
            err.to_string()
                .contains("requires `#[gpui_component_shape(state = ...)]`"),
            "derive should require explicit backing state: {err}"
        );
    }

    #[test]
    fn derive_emits_gpui_contract_impl() {
        let input: DeriveInput = syn::parse2(quote! {
            #[derive(GpuiComponentShape)]
            #[gpui_component_shape(state = crate::state::TagsState, value = Vec<String>)]
            struct TagsInput;
        })
        .unwrap();

        let expanded = expand(input).unwrap();
        let compact = compact_tokens(&expanded.to_string());

        assert!(compact.contains("impl::component_shape_gpui::ComponentShapeMetadataforTagsInput"));
        assert!(compact.contains("impl::component_shape_gpui::DeclaredComponentShapeforTagsInput"));
        assert!(compact.contains("impl::component_shape_gpui::GpuiComponentShapeforTagsInput"));
        assert!(
            compact
                .contains("impl::component_shape_gpui::ComponentShapeFor<Vec<String>>forTagsInput")
        );
        assert!(compact.contains(
            "impl::component_shape_gpui::GpuiComponentShapeFor<Vec<String>>forTagsInput"
        ));
    }

    #[test]
    fn derive_infers_mcp_input_from_value_metadata() {
        let input: DeriveInput = syn::parse2(quote! {
            #[derive(GpuiComponentShape)]
            #[gpui_component_shape(state = crate::state::TagsState, value = Vec<String>)]
            struct TagsInput;
        })
        .unwrap();

        let expanded = expand(input).unwrap();
        let compact = compact_tokens(&expanded.to_string());

        assert!(compact.contains("McpInput::string_list"));
    }

    #[test]
    fn derive_infers_state_when_shape_attribute_is_present() {
        let input: DeriveInput = syn::parse2(quote! {
            #[derive(GpuiComponentShape)]
            #[gpui_component_shape(value = Vec<String>)]
            struct TagsInput;
        })
        .unwrap();

        let expanded = expand(input).unwrap();
        let compact = compact_tokens(&expanded.to_string());

        assert!(compact.contains("typeState=TagsInputState"));
    }

    #[test]
    fn derive_infers_generic_state_with_component_generics() {
        let input: DeriveInput = syn::parse2(quote! {
            #[derive(GpuiComponentShape)]
            #[gpui_component_shape(value = T)]
            struct Input<T>
            where
                T: 'static,
            {
                value: T,
            }
        })
        .unwrap();

        let expanded = expand(input).unwrap();
        let compact = compact_tokens(&expanded.to_string());

        assert!(compact.contains("typeState=InputState<T>"));
    }

    #[test]
    fn derive_infers_value_markers_from_state_value_binding() {
        let input: DeriveInput = syn::parse2(quote! {
            #[derive(GpuiComponentShape)]
            #[gpui_component_shape(state = crate::state::TagsState, value_binding)]
            struct TagsInput;
        })
        .unwrap();

        let expanded = expand(input).unwrap();
        let compact = compact_tokens(&expanded.to_string());

        assert!(
            compact.contains("ComponentShapeFor<__GpuiComponentValueBindingValue>forTagsInput")
        );
        assert!(
            compact.contains("GpuiComponentStateValueBinding<__GpuiComponentValueBindingValue>")
        );
    }
}
