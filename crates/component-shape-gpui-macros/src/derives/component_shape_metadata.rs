use component_shape_codegen::{
    common_inferred_mcp_input_shape_for_types, inferred_mcp_input_shape_for_type,
    mcp_input_expr_tokens, mcp_input_shape_tokens, validate_mcp_input_expr,
};
use proc_macro2::TokenStream;
use quote::{ToTokens as _, quote};
use syn::{
    Expr, Ident, LitStr, Path, Result, Token, Type, Visibility, parse::ParseStream,
    punctuated::Punctuated, spanned::Spanned as _,
};

use super::component_shape_constructor::constructor_body_tokens;
use super::crate_paths::CratePaths;

pub(super) const SHAPE_METADATA_OPTIONS: &str = "`new = ...`, `state = ...`, `component = ...`, `value = ...`, `values(...)`, \
     `value_binding`, `field_suffix = ...`, or `mcp_input = ...`";

pub(super) const FUNCTION_SHAPE_OPTIONS: &str = "`state = ...`, `new = ...`, `component = ...`, `value = ...`, `values(...)`, \
     `value_binding`, `field_suffix = ...`, or `mcp_input = ...`";

pub(super) mod kw {
    syn::custom_keyword!(component);
    syn::custom_keyword!(field_suffix);
    syn::custom_keyword!(mcp_input);
    syn::custom_keyword!(new);
    syn::custom_keyword!(state);
    syn::custom_keyword!(value);
    syn::custom_keyword!(values);
    syn::custom_keyword!(value_binding);
}

fn rust_type_key(ty: &Type) -> String {
    ty.to_token_stream()
        .to_string()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}

#[derive(Default)]
pub(super) struct ComponentShapeMetadata {
    new: Option<Expr>,
    state: Option<Type>,
    component: Option<Type>,
    values: Vec<Type>,
    value_binding: bool,
    field_suffix: Option<LitStr>,
    mcp_input: Option<Expr>,
}

pub(super) enum ShapeOption {
    New {
        expr: Expr,
        span: proc_macro2::Span,
    },
    State {
        ty: Type,
        span: proc_macro2::Span,
    },
    Component {
        ty: Type,
        span: proc_macro2::Span,
    },
    Value {
        ty: Type,
        span: proc_macro2::Span,
    },
    Values {
        values: Vec<Type>,
        span: proc_macro2::Span,
    },
    ValueBinding {
        span: proc_macro2::Span,
    },
    FieldSuffix {
        suffix: LitStr,
        span: proc_macro2::Span,
    },
    McpInput {
        expr: Expr,
        span: proc_macro2::Span,
    },
}

impl ShapeOption {
    pub(super) fn parse_function(input: ParseStream<'_>) -> Result<Self> {
        if input.peek(kw::new) {
            let key = input.parse::<kw::new>()?;
            input.parse::<Token![=]>()?;
            return Ok(Self::New {
                expr: input.parse()?,
                span: key.span,
            });
        }
        if input.peek(kw::state) {
            let key = input.parse::<kw::state>()?;
            input.parse::<Token![=]>()?;
            return Ok(Self::State {
                ty: input.parse()?,
                span: key.span,
            });
        }
        if input.peek(kw::component) {
            let key = input.parse::<kw::component>()?;
            input.parse::<Token![=]>()?;
            return Ok(Self::Component {
                ty: input.parse()?,
                span: key.span,
            });
        }
        if input.peek(kw::value) {
            let key = input.parse::<kw::value>()?;
            input.parse::<Token![=]>()?;
            return Ok(Self::Value {
                ty: input.parse()?,
                span: key.span,
            });
        }
        if input.peek(kw::values) {
            let key = input.parse::<kw::values>()?;
            let values_content;
            syn::parenthesized!(values_content in input);
            return Ok(Self::Values {
                values: ComponentShapeMetadata::parse_values(&values_content)?,
                span: key.span,
            });
        }
        if input.peek(kw::value_binding) {
            let key = input.parse::<kw::value_binding>()?;
            return Ok(Self::ValueBinding { span: key.span });
        }
        if input.peek(kw::field_suffix) {
            let key = input.parse::<kw::field_suffix>()?;
            input.parse::<Token![=]>()?;
            return Ok(Self::FieldSuffix {
                suffix: input.parse()?,
                span: key.span,
            });
        }
        if input.peek(kw::mcp_input) {
            let key = input.parse::<kw::mcp_input>()?;
            input.parse::<Token![=]>()?;
            return Ok(Self::McpInput {
                expr: input.parse()?,
                span: key.span,
            });
        }

        Err(input.error(format!("expected {FUNCTION_SHAPE_OPTIONS}")))
    }

    pub(super) fn from_nested_meta(meta: &syn::meta::ParseNestedMeta<'_>) -> Result<Self> {
        let span = meta.path.span();
        if meta.path.is_ident("new") {
            Ok(Self::New {
                expr: meta.value()?.parse()?,
                span,
            })
        } else if meta.path.is_ident("state") {
            Ok(Self::State {
                ty: meta.value()?.parse()?,
                span,
            })
        } else if meta.path.is_ident("component") {
            Ok(Self::Component {
                ty: meta.value()?.parse()?,
                span,
            })
        } else if meta.path.is_ident("value") {
            Ok(Self::Value {
                ty: meta.value()?.parse()?,
                span,
            })
        } else if meta.path.is_ident("values") {
            let content;
            syn::parenthesized!(content in meta.input);
            Ok(Self::Values {
                values: ComponentShapeMetadata::parse_values(&content)?,
                span,
            })
        } else if meta.path.is_ident("field_suffix") {
            Ok(Self::FieldSuffix {
                suffix: meta.value()?.parse()?,
                span,
            })
        } else if meta.path.is_ident("mcp_input") {
            Ok(Self::McpInput {
                expr: meta.value()?.parse()?,
                span,
            })
        } else if meta.path.is_ident("value_binding") {
            Ok(Self::ValueBinding { span })
        } else {
            Err(meta.error(format!(
                "unsupported `gpui_component_shape` option; expected {SHAPE_METADATA_OPTIONS}",
            )))
        }
    }

    pub(super) fn apply(self, shape: &mut ComponentShapeMetadata) -> Result<()> {
        match self {
            Self::New { expr, span } => shape.set_new(expr, span_token("new", span)),
            Self::State { ty, span } => shape.set_state(ty, span_token("state", span)),
            Self::Component { ty, span } => shape.set_component(ty, span_token("component", span)),
            Self::Value { ty, span } => shape.add_value(ty, span_token("value", span)),
            Self::Values { values, span } => shape.add_values(values, span_token("values", span)),
            Self::ValueBinding { span } => {
                shape.enable_value_binding(span_token("value_binding", span))
            },
            Self::FieldSuffix { suffix, span } => {
                shape.set_field_suffix(suffix, span_token("field_suffix", span))
            },
            Self::McpInput { expr, span } => {
                shape.set_mcp_input(expr, span_token("mcp_input", span))
            },
        }
    }
}

fn span_token(name: &str, span: proc_macro2::Span) -> Ident {
    Ident::new(name, span)
}

impl ComponentShapeMetadata {
    pub(super) fn set_new<T: quote::ToTokens>(&mut self, new: Expr, span: T) -> Result<()> {
        set_once(&mut self.new, new, span, "new")
    }

    pub(super) fn set_state<T: quote::ToTokens>(&mut self, state: Type, span: T) -> Result<()> {
        set_once(&mut self.state, state, span, "state")
    }

    pub(super) fn state(&self) -> Option<&Type> {
        self.state.as_ref()
    }

    pub(super) fn set_component<T: quote::ToTokens>(
        &mut self,
        component: Type,
        span: T,
    ) -> Result<()> {
        match &component {
            Type::Path(_) => {},
            Type::Infer(_) => {
                return Err(syn::Error::new_spanned(
                    span,
                    "component type cannot be inferred with bare `_`; use an explicit component type",
                ));
            },
            _ => {
                return Err(syn::Error::new_spanned(
                    span,
                    "component metadata must be a path-like type, such as `my_crate::Input`",
                ));
            },
        }

        set_once(&mut self.component, component, span, "component")
    }

    pub(super) fn component(&self) -> Option<&Type> {
        self.component.as_ref()
    }

    pub(super) fn add_value<T: quote::ToTokens>(&mut self, value: Type, _span: T) -> Result<()> {
        if self.has_value(&value) {
            return Err(syn::Error::new_spanned(
                value,
                "duplicate value metadata; remove the duplicate `value = ...` or `values(...)` entry",
            ));
        }

        self.values.push(value);
        Ok(())
    }

    pub(super) fn add_values<I, T>(&mut self, values: I, span: T) -> Result<()>
    where
        I: IntoIterator<Item = Type>,
        T: quote::ToTokens,
    {
        let mut added = false;
        for value in values {
            self.add_value(value, &span)?;
            added = true;
        }

        if !added {
            return Err(syn::Error::new_spanned(
                span,
                "`values(...)` requires at least one value type",
            ));
        }

        Ok(())
    }

    pub(super) fn add_inferred_values<I>(&mut self, values: I)
    where
        I: IntoIterator<Item = Type>,
    {
        for value in values {
            if !self.has_value(&value) {
                self.values.push(value);
            }
        }
    }

    pub(super) fn parse_values(input: ParseStream<'_>) -> Result<Vec<Type>> {
        Ok(Punctuated::<Type, Token![,]>::parse_terminated(input)?
            .into_iter()
            .collect())
    }

    pub(super) fn has_value_metadata(&self) -> bool {
        !self.values.is_empty()
    }

    fn has_value(&self, value: &Type) -> bool {
        let value_key = rust_type_key(value);
        self.values
            .iter()
            .any(|existing| rust_type_key(existing) == value_key)
    }

    pub(super) fn value_impl_tokens(
        &self,
        component_shape_crate: &Path,
        gpui_crate: &Path,
        ident: &Ident,
        generics: &syn::Generics,
    ) -> TokenStream {
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
        let value_impls = self.values.iter().map(|value| {
            let mcp_input_const = self
                .explicit_mcp_input_tokens(component_shape_crate)
                .or_else(|| {
                    inferred_mcp_input_shape_for_type(value)
                        .map(|input| mcp_input_shape_tokens(component_shape_crate, input))
                })
                .map(|input| {
                    quote! {
                        const MCP_INPUT: #component_shape_crate::McpInput = #input;
                    }
                });
            quote! {
                impl #impl_generics #component_shape_crate::ComponentShapeFor<#value>
                    for #ident #ty_generics
                    #where_clause
                {
                    #mcp_input_const
                }

                impl #impl_generics #gpui_crate::GpuiComponentShapeFor<#value>
                    for #ident #ty_generics
                    #where_clause
                {
                }
            }
        });

        quote! {
            #(#value_impls)*
        }
    }

    pub(super) fn state_value_binding_value_impl_tokens(
        &self,
        component_shape_crate: &Path,
        component_shape_gpui_crate: &Path,
        ident: &Ident,
        generics: &syn::Generics,
        state: &Type,
    ) -> Option<TokenStream> {
        if self.has_value_metadata() || !self.has_value_binding() {
            return None;
        }

        let mut binding_generics = generics.clone();
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

        let (impl_generics, _, where_clause) = binding_generics.split_for_impl();
        let (_, ty_generics, _) = generics.split_for_impl();
        let mcp_input_const =
            self.explicit_mcp_input_tokens(component_shape_crate)
                .map(|mcp_input| {
                    quote! {
                        const MCP_INPUT: #component_shape_crate::McpInput = #mcp_input;
                    }
                });

        Some(quote! {
            impl #impl_generics #component_shape_crate::ComponentShapeFor<
                __GpuiComponentValueBindingValue
            > for #ident #ty_generics
                #where_clause
            {
                #mcp_input_const
            }

            impl #impl_generics #component_shape_gpui_crate::GpuiComponentShapeFor<
                __GpuiComponentValueBindingValue
            > for #ident #ty_generics
                #where_clause
            {
            }
        })
    }

    pub(super) fn enable_value_binding<T: quote::ToTokens>(&mut self, span: T) -> Result<()> {
        if self.value_binding {
            return Err(syn::Error::new_spanned(
                span,
                "duplicate `value_binding` option",
            ));
        }

        self.value_binding = true;
        Ok(())
    }

    pub(super) fn infer_value_binding(&mut self) {
        self.value_binding = true;
    }

    pub(super) fn has_value_binding(&self) -> bool {
        self.value_binding
    }

    pub(super) fn set_field_suffix<T: quote::ToTokens>(
        &mut self,
        field_suffix: LitStr,
        span: T,
    ) -> Result<()> {
        if !component_shape::is_valid_component_suffix(&field_suffix.value()) {
            return Err(syn::Error::new_spanned(
                &field_suffix,
                format!(
                    "`field_suffix` must be a non-empty ASCII identifier suffix, got `{}`",
                    field_suffix.value()
                ),
            ));
        }
        set_once(&mut self.field_suffix, field_suffix, span, "field_suffix")
    }

    pub(super) fn set_mcp_input<T: quote::ToTokens>(
        &mut self,
        mcp_input: Expr,
        span: T,
    ) -> Result<()> {
        validate_mcp_input_expr(&mcp_input)?;
        set_once(&mut self.mcp_input, mcp_input, span, "mcp_input")
    }

    pub(super) fn constructor_body_or(&self, default_body: TokenStream) -> TokenStream {
        self.new
            .as_ref()
            .map(constructor_body_tokens)
            .unwrap_or(default_body)
    }

    pub(super) fn render_component_tokens(
        gpui_crate: &Path,
        component_shape_gpui_crate: &Path,
        vis: &Visibility,
        adapter_ident: &Ident,
        state: &Type,
        component: Option<&Type>,
        generics: &syn::Generics,
    ) -> Result<(TokenStream, TokenStream)> {
        let Some(component) = component else {
            return Ok((
                quote! { type RenderComponent = #component_shape_gpui_crate::NoGpuiRenderComponent; },
                quote! {},
            ));
        };

        let component = component.clone();
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
        let phantom_type = phantom_type_tokens(generics);

        Ok((
            quote! { type RenderComponent = #adapter_ident #ty_generics; },
            quote! {
                #[doc(hidden)]
                #vis struct #adapter_ident #generics(
                    ::core::marker::PhantomData<fn() -> #phantom_type>
                ) #where_clause;

                impl #impl_generics #component_shape_gpui_crate::GpuiComponentRender<#state>
                    for #adapter_ident #ty_generics
                    #where_clause
                {
                    const RENDERS: bool = true;

                    fn new(entity: &#gpui_crate::Entity<#state>) -> impl #gpui_crate::IntoElement {
                        <#component>::new(entity)
                    }
                }
            },
        ))
    }

    pub(super) fn metadata_impl_tokens(&self, component_shape_crate: &Path) -> TokenStream {
        let prototyping_const = self.field_suffix.as_ref().map(|field_suffix| {
            quote! {
                const PROTOTYPING: #component_shape_crate::ComponentPrototyping =
                    #component_shape_crate::ComponentPrototyping::new()
                        .field_suffix(#field_suffix);
            }
        });
        let render_capability = if self.component.is_some() {
            quote! { #component_shape_crate::RenderCapability::Component }
        } else {
            quote! { #component_shape_crate::RenderCapability::None }
        };
        let value_binding_capability = if self.value_binding {
            quote! { #component_shape_crate::ValueBindingCapability::Inherited }
        } else {
            quote! { #component_shape_crate::ValueBindingCapability::None }
        };
        let mcp_input_const = self
            .explicit_mcp_input_tokens(component_shape_crate)
            .or_else(|| self.inferred_mcp_input_tokens(component_shape_crate))
            .map(|mcp_input| {
                quote! {
                    const MCP_INPUT: #component_shape_crate::McpInput = #mcp_input;
                }
            });

        quote! {
            #prototyping_const
            #mcp_input_const

            const CAPABILITIES: #component_shape_crate::ComponentCapabilities =
                #component_shape_crate::ComponentCapabilities::new()
                    .with_render(#render_capability)
                    .with_value_binding(#value_binding_capability);
        }
    }

    fn inferred_mcp_input_tokens(&self, component_shape_crate: &Path) -> Option<TokenStream> {
        common_inferred_mcp_input_shape_for_types(&self.values)
            .map(|input| mcp_input_shape_tokens(component_shape_crate, input))
    }

    fn explicit_mcp_input_tokens(&self, component_shape_crate: &Path) -> Option<TokenStream> {
        self.mcp_input
            .as_ref()
            .map(|input| mcp_input_expr_tokens(component_shape_crate, input))
    }
}

fn phantom_type_tokens(generics: &syn::Generics) -> TokenStream {
    let params: Vec<TokenStream> = generics
        .params
        .iter()
        .filter_map(|param| match param {
            syn::GenericParam::Type(param) => {
                let ident = &param.ident;
                Some(quote! { #ident })
            },
            syn::GenericParam::Lifetime(param) => {
                let lifetime = &param.lifetime;
                Some(quote! { &#lifetime () })
            },
            syn::GenericParam::Const(_) => None,
        })
        .collect();

    if params.is_empty() {
        quote! { () }
    } else {
        quote! { (#(#params),*) }
    }
}

fn set_once<T, S: quote::ToTokens>(
    slot: &mut Option<T>,
    value: T,
    span: S,
    option: &str,
) -> Result<()> {
    if slot.replace(value).is_some() {
        return Err(syn::Error::new_spanned(
            span,
            format!("duplicate `{option}` option"),
        ));
    }

    Ok(())
}

pub(super) fn crate_paths() -> CratePaths {
    CratePaths::resolve()
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use syn::{parse::Parser as _, parse_quote};

    use super::*;

    fn parse_function(tokens: TokenStream) -> syn::Result<ShapeOption> {
        (|input: ParseStream<'_>| ShapeOption::parse_function(input)).parse2(tokens)
    }

    fn compact(tokens: impl quote::ToTokens) -> String {
        tokens
            .to_token_stream()
            .to_string()
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect()
    }

    #[test]
    fn function_options_parse_and_apply_every_metadata_variant() {
        let mut metadata = ComponentShapeMetadata::default();
        for tokens in [
            quote!(new = State::new),
            quote!(state = State<String>),
            quote!(component = Input<String>),
            quote!(value = String),
            quote!(values(u64, bool)),
            quote!(value_binding),
            quote!(field_suffix = "input"),
            quote!(mcp_input = string),
        ] {
            parse_function(tokens)
                .expect("option should parse")
                .apply(&mut metadata)
                .expect("option should apply");
        }

        assert_eq!(compact(metadata.state().expect("state")), "State<String>");
        assert_eq!(
            compact(metadata.component().expect("component")),
            "Input<String>"
        );
        assert!(metadata.has_value_metadata());
        assert!(metadata.has_value_binding());
        assert_eq!(
            compact(metadata.constructor_body_or(quote!(fallback))),
            "(State::new)(window,cx)"
        );
        assert!(parse_function(quote!(unknown = true)).is_err());
    }

    #[test]
    fn derive_attribute_options_parse_and_reject_unknown_metadata() {
        let attr: syn::Attribute = parse_quote! {
            #[gpui_component_shape(
                new = State::new,
                state = State,
                component = Input,
                value = String,
                values(u64, bool),
                value_binding,
                field_suffix = "input",
                mcp_input = string
            )]
        };
        let mut metadata = ComponentShapeMetadata::default();
        attr.parse_nested_meta(|meta| ShapeOption::from_nested_meta(&meta)?.apply(&mut metadata))
            .expect("attribute should parse");
        assert!(metadata.has_value_binding());

        let unknown: syn::Attribute = parse_quote!(#[gpui_component_shape(unknown = true)]);
        assert!(
            unknown
                .parse_nested_meta(|meta| ShapeOption::from_nested_meta(&meta).map(|_| ()))
                .is_err()
        );
    }

    #[test]
    fn metadata_rejects_duplicates_empty_values_and_invalid_components() {
        let mut metadata = ComponentShapeMetadata::default();
        metadata
            .set_new(parse_quote!(State::new), quote!(new))
            .expect("first new should apply");
        assert!(
            metadata
                .set_new(parse_quote!(State::default), quote!(new))
                .is_err()
        );

        assert!(
            metadata
                .set_component(parse_quote!(_), quote!(component))
                .is_err()
        );
        assert!(
            metadata
                .set_component(parse_quote!((Input, Output)), quote!(component))
                .is_err()
        );
        metadata
            .add_value(parse_quote!(String), quote!(value))
            .expect("first value should apply");
        assert!(
            metadata
                .add_value(parse_quote!(String), quote!(value))
                .is_err()
        );
        assert!(
            metadata
                .add_values(Vec::<Type>::new(), quote!(values))
                .is_err()
        );

        metadata
            .enable_value_binding(quote!(value_binding))
            .expect("first binding should apply");
        assert!(
            metadata
                .enable_value_binding(quote!(value_binding))
                .is_err()
        );
        assert!(
            metadata
                .set_field_suffix(parse_quote!("bad-suffix"), quote!(field_suffix))
                .is_err()
        );
    }

    #[test]
    fn phantom_type_tokens_include_types_and_lifetimes_but_not_consts() {
        let generics: syn::Generics = parse_quote!(<'a, T, const N: usize>);
        assert_eq!(compact(phantom_type_tokens(&generics)), "(&'a(),T)");
        assert_eq!(
            compact(phantom_type_tokens(&syn::Generics::default())),
            "()"
        );
    }
}
