use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{
    Attribute, GenericArgument, GenericParam, Generics, Ident, ItemImpl, PathArguments, Result,
    Token, Type, Visibility, braced, parse_macro_input,
};

use super::component_shape_metadata::{
    ComponentShapeMetadata, FUNCTION_SHAPE_OPTIONS, ShapeOption, crate_paths, kw,
};

struct ComponentShapeInput {
    attrs: Vec<Attribute>,
    vis: Visibility,
    ident: Ident,
    generics: Generics,
    state: Type,
    metadata: ComponentShapeMetadata,
    impls: Vec<ItemImpl>,
}

impl Parse for ComponentShapeInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let vis = input.parse()?;
        input.parse::<Token![struct]>()?;
        let ident = input.parse()?;
        let mut generics: Generics = input.parse()?;
        generics.where_clause = input.parse()?;

        let content;
        braced!(content in input);

        let mut state = None;
        let mut metadata = ComponentShapeMetadata::default();
        let mut impls = Vec::new();

        while !content.is_empty() {
            if content.peek(Token![type]) {
                content.parse::<Token![type]>()?;
                let type_ident: Ident = content.parse()?;
                if type_ident != "State" {
                    return Err(syn::Error::new_spanned(
                        type_ident,
                        "expected `type State = ...;`",
                    ));
                }
                content.parse::<Token![=]>()?;
                if state.replace(content.parse()?).is_some() {
                    return Err(syn::Error::new_spanned(
                        type_ident,
                        "duplicate `type State = ...;`",
                    ));
                }
                parse_option_separator(&content)?;
            } else if is_shape_option_start(&content) {
                let option = content.call(ShapeOption::parse_function)?;
                match option {
                    ShapeOption::State { ty, span } => {
                        if state.replace(ty).is_some() {
                            return Err(syn::Error::new_spanned(
                                Ident::new("state", span),
                                "duplicate state metadata; use only one of `type State = ...;` or `state = ...;`",
                            ));
                        }
                    },
                    option => option.apply(&mut metadata)?,
                }
                parse_option_separator(&content)?;
            } else if content.peek(Token![impl]) || content.peek(Token![#]) {
                let impl_item: ItemImpl = content.parse()?;
                impls.push(impl_item);
            } else {
                return Err(content.error(format!(
                    "expected `type State = ...;`, an `impl` item, or {FUNCTION_SHAPE_OPTIONS}"
                )));
            }
        }

        Ok(Self {
            attrs,
            vis,
            ident,
            generics,
            state: state
                .ok_or_else(|| input.error("missing `type State = ...;` or `state = ...;`"))?,
            metadata,
            impls,
        })
    }
}

fn is_shape_option_start(input: ParseStream<'_>) -> bool {
    input.peek(kw::new)
        || input.peek(kw::state)
        || input.peek(kw::component)
        || input.peek(kw::value)
        || input.peek(kw::values)
        || input.peek(kw::value_binding)
        || input.peek(kw::field_suffix)
}

fn parse_option_separator(input: ParseStream<'_>) -> Result<()> {
    if input.peek(Token![;]) {
        input.parse::<Token![;]>()?;
        Ok(())
    } else {
        Err(input.error("expected `;` after component shape option"))
    }
}

fn phantom_type_tokens(generics: &Generics) -> TokenStream {
    let params: Vec<TokenStream> = generics
        .params
        .iter()
        .filter_map(|param| match param {
            GenericParam::Type(param) => {
                let ident = &param.ident;
                Some(quote! { #ident })
            },
            GenericParam::Lifetime(param) => {
                let lifetime = &param.lifetime;
                Some(quote! { &#lifetime () })
            },
            GenericParam::Const(_) => None,
        })
        .collect();

    if params.is_empty() {
        quote! { () }
    } else {
        quote! { (#(#params),*) }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NestedShapeImplKind {
    GpuiComponentValueBinding,
    Other,
}

fn classify_nested_shape_impl(impl_item: &ItemImpl) -> NestedShapeImplKind {
    let Some((_, path, _)) = impl_item.trait_.as_ref() else {
        return NestedShapeImplKind::Other;
    };
    let Some(last) = path.segments.last() else {
        return NestedShapeImplKind::Other;
    };

    if last.ident == "GpuiComponentValueBinding" {
        NestedShapeImplKind::GpuiComponentValueBinding
    } else {
        NestedShapeImplKind::Other
    }
}

fn nested_value_binding_value(impl_item: &ItemImpl) -> Option<Type> {
    let (_, path, _) = impl_item.trait_.as_ref()?;
    let last = path.segments.last()?;
    if last.ident != "GpuiComponentValueBinding" {
        return None;
    }

    let PathArguments::AngleBracketed(arguments) = &last.arguments else {
        return None;
    };

    arguments.args.iter().find_map(|argument| match argument {
        GenericArgument::Type(value) => Some(value.clone()),
        _ => None,
    })
}

fn expand(input: ComponentShapeInput) -> TokenStream {
    let ComponentShapeInput {
        attrs,
        vis,
        ident,
        generics,
        state,
        metadata,
        impls,
    } = input;
    let mut metadata = metadata;

    let paths = crate_paths();
    let component_shape_crate = paths.component_shape;
    let component_shape_gpui_crate = paths.component_shape_gpui;
    let gpui_crate = paths.gpui;
    let phantom_type = phantom_type_tokens(&generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let constructor_body = metadata.constructor_body_or(quote! { <#state>::new(window, cx) });
    let render_component_adapter_ident = format_ident!("__{}RenderComponent", ident);
    let (render_component_assoc, render_component_adapter) =
        match ComponentShapeMetadata::render_component_tokens(
            &gpui_crate,
            &component_shape_gpui_crate,
            &vis,
            &render_component_adapter_ident,
            &state,
            metadata.component(),
            &generics,
        ) {
            Ok(tokens) => tokens,
            Err(error) => return error.to_compile_error(),
        };
    let nested_impl_kinds = impls
        .iter()
        .map(classify_nested_shape_impl)
        .collect::<Vec<_>>();
    let nested_value_binding_values = impls
        .iter()
        .filter_map(nested_value_binding_value)
        .collect::<Vec<_>>();
    if !metadata.has_value_metadata() && !nested_value_binding_values.is_empty() {
        metadata.add_inferred_values(nested_value_binding_values);
        metadata.infer_value_binding();
    }
    if !metadata.has_value_metadata() {
        return syn::Error::new_spanned(
            ident,
            "`component_shape!` requires value metadata; add `value = ...`, `values(...)`, or a nested `GpuiComponentValueBinding<T>` impl",
        )
        .to_compile_error();
    }
    if metadata.has_value_binding()
        && !nested_impl_kinds.contains(&NestedShapeImplKind::GpuiComponentValueBinding)
    {
        return syn::Error::new_spanned(
            ident,
            "`value_binding` requires a nested `GpuiComponentValueBinding<T>` impl in the `component_shape!` block",
        )
        .to_compile_error();
    }
    let metadata_impl_items = metadata.metadata_impl_tokens(&component_shape_crate);
    let component_shape_for_impls = metadata.value_impl_tokens(
        &component_shape_crate,
        &component_shape_gpui_crate,
        &ident,
        &generics,
    );

    quote! {
        #(#attrs)*
        #vis struct #ident #generics(
            ::core::marker::PhantomData<fn() -> #phantom_type>
        );

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

        #(#impls)*
        #component_shape_for_impls
    }
}

pub fn function(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as ComponentShapeInput);
    expand(input).into()
}

#[cfg(test)]
mod tests {
    use super::{
        ComponentShapeInput, NestedShapeImplKind, classify_nested_shape_impl, expand,
        nested_value_binding_value,
    };
    use quote::quote;

    fn compact_tokens(tokens: &str) -> String {
        tokens.chars().filter(|c| !c.is_whitespace()).collect()
    }

    #[test]
    fn function_macro_emits_gpui_contract_impl() {
        let input: ComponentShapeInput = syn::parse2(quote! {
            pub struct LocalInputShape {
                type State = crate::InputState;
                value = String;
                field_suffix = "input";
            }
        })
        .unwrap();

        let expanded = expand(input);
        let compact = compact_tokens(&expanded.to_string());

        assert!(
            compact.contains("impl::component_shape::ComponentShapeMetadataforLocalInputShape")
        );
        assert!(
            compact.contains("impl::component_shape::DeclaredComponentShapeforLocalInputShape")
        );
        assert!(
            compact.contains("impl::component_shape_gpui::GpuiComponentShapeforLocalInputShape")
        );
        assert!(
            compact.contains(
                "impl::component_shape_gpui::DeclaredGpuiComponentShapeforLocalInputShape"
            )
        );
        assert!(
            compact.contains("impl::component_shape::ComponentShapeFor<String>forLocalInputShape")
        );
        assert!(compact.contains(
            "impl::component_shape_gpui::GpuiComponentShapeFor<String>forLocalInputShape"
        ));
        assert!(compact.contains("ComponentPrototyping::new().field_suffix(\"input\")"));
    }

    #[test]
    fn function_macro_accepts_state_option() {
        let input: ComponentShapeInput = syn::parse2(quote! {
            pub struct LocalInputShape {
                state = crate::InputState;
                value = String;
            }
        })
        .unwrap();

        let expanded = expand(input);
        let compact = compact_tokens(&expanded.to_string());

        assert!(compact.contains("typeState=crate::InputState"));
    }

    #[test]
    fn classify_nested_value_binding_impl() {
        let impl_item: syn::ItemImpl = syn::parse2(quote! {
            impl component_shape_gpui::GpuiComponentValueBinding<String> for Input {
                type Event = InputEvent;

                fn value_change(
                    _state: &Self::State,
                    _event: &Self::Event,
                ) -> component_shape::ValueChange<String> {
                    component_shape::ValueChange::Unchanged
                }
            }
        })
        .unwrap();

        assert_eq!(
            classify_nested_shape_impl(&impl_item),
            NestedShapeImplKind::GpuiComponentValueBinding
        );
    }

    #[test]
    fn nested_value_binding_value_extracts_bound_value_type() {
        let impl_item: syn::ItemImpl = syn::parse2(quote! {
            impl<T> component_shape_gpui::GpuiComponentValueBinding<Vec<T>> for Input<T> {
                type Event = InputEvent;

                fn value_change(
                    _state: &Self::State,
                    _event: &Self::Event,
                ) -> component_shape::ValueChange<Vec<T>> {
                    component_shape::ValueChange::Unchanged
                }
            }
        })
        .unwrap();

        let value = nested_value_binding_value(&impl_item).expect("value type should be inferred");

        assert_eq!(compact_tokens(&quote! { #value }.to_string()), "Vec<T>");
    }

    #[test]
    fn function_macro_infers_value_metadata_from_nested_value_binding_impl() {
        let input: ComponentShapeInput = syn::parse2(quote! {
            pub struct LocalInputShape<T> {
                type State = crate::InputState;

                impl<T> component_shape_gpui::GpuiComponentValueBinding<T>
                    for LocalInputShape<T>
                {
                    type Event = InputEvent;

                    fn value_change(
                        _state: &Self::State,
                        _event: &Self::Event,
                    ) -> component_shape::ValueChange<T> {
                        component_shape::ValueChange::Unchanged
                    }
                }
            }
        })
        .unwrap();

        let expanded = expand(input);
        let compact = compact_tokens(&expanded.to_string());

        assert!(compact.contains("ComponentShapeFor<T>forLocalInputShape<T>"));
        assert!(compact.contains("GpuiComponentShapeFor<T>forLocalInputShape<T>"));
        assert!(
            compact.contains(
                ".with_value_binding(::component_shape::ValueBindingCapability::Inherited)"
            )
        );
    }

    #[test]
    fn function_macro_requires_value_metadata() {
        let input: ComponentShapeInput = syn::parse2(quote! {
            struct MissingValueShape {
                type State = crate::InputState;
            }
        })
        .unwrap();

        let expanded = expand(input).to_string();

        assert!(expanded.contains("requires value metadata"));
    }
}
