use super::*;

pub(crate) fn expand_mcp_tool_input(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    expand_mcp_tool_input_impl(input)
}

fn expand_mcp_tool_input_impl(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let schema_options = SchemaOptions::parse(&input)?;
    if schema_options.transparent {
        return Err(syn::Error::new(
            input.ident.span(),
            "McpToolInput requires an object-shaped struct; transparent inputs should derive McpJsonSchema and be used as fields",
        ));
    }

    let ident = input.ident;
    let mut generics = input.generics;
    let mcp_crate = schema_options
        .crate_path
        .clone()
        .map(Ok)
        .unwrap_or_else(resolve_default_mcp_crate_path)?;
    let data = match input.data {
        Data::Struct(data) => data,
        _ => {
            return Err(syn::Error::new(
                ident.span(),
                "McpToolInput can only be derived for structs with named fields",
            ));
        },
    };
    let fields = match data.fields {
        Fields::Named(fields) => fields,
        _ => {
            return Err(syn::Error::new(
                ident.span(),
                "McpToolInput requires a struct with named fields",
            ));
        },
    };

    let mut field_tokens = Vec::new();
    let mut schema_field_tokens = Vec::new();
    let mut wire_names = BTreeMap::new();

    for field in fields.named {
        let Some(field_ident) = field.ident.clone() else {
            continue;
        };
        let options = FieldOptions::parse(&field)?;
        let description = options
            .description
            .clone()
            .or_else(|| doc_description(&field.attrs))
            .map(|description| LitStr::new(&description, field_ident.span()));
        let ty = field.ty;
        if options.skip {
            let default = options
                .default_value
                .as_ref()
                .map(FieldDefault::tokens)
                .unwrap_or_else(|| {
                    generics
                        .make_where_clause()
                        .predicates
                        .push(parse_quote!(#ty: ::core::default::Default));
                    quote! { ::core::default::Default::default() }
                });
            field_tokens.push(quote! {
                #field_ident: #default
            });
            continue;
        }

        generics
            .make_where_clause()
            .predicates
            .push(parse_quote!(#ty: #mcp_crate::McpToolValue));

        let defaulted = options.defaulted();
        let rust_field_name = field_ident.to_string();
        let serde_field_name = options
            .serde_rename
            .clone()
            .or_else(|| {
                schema_options
                    .serde_rename_all
                    .map(|rule| rule.apply(&rust_field_name))
            })
            .unwrap_or_else(|| rust_field_name.clone());
        let field_name = options
            .mcp_rename
            .clone()
            .or_else(|| {
                schema_options
                    .mcp_rename_all
                    .map(|rule| rule.apply(&rust_field_name))
            })
            .unwrap_or_else(|| serde_field_name.clone());
        claim_wire_name(
            &mut wire_names,
            &field_name,
            field_ident.span(),
            "MCP tool input field or alias",
        )?;
        for alias in &options.aliases {
            claim_wire_name(
                &mut wire_names,
                alias,
                field_ident.span(),
                "MCP tool input field or alias",
            )?;
        }
        let field_name_lit = LitStr::new(&field_name, field_ident.span());
        let alias_lits = options
            .aliases
            .iter()
            .map(|alias| LitStr::new(alias, field_ident.span()))
            .collect::<Vec<_>>();
        let decode_name = (field_name != serde_field_name)
            .then(|| LitStr::new(&serde_field_name, field_ident.span()));
        let required = options
            .required
            .unwrap_or_else(|| !is_option_type(&ty) && !defaulted && !schema_options.defaulted);
        let default = options.default_value.as_ref();
        let description_tokens = description
            .map(|description| {
                quote! {
                    __component_shape_mcp_field_schema.set_description(#description);
                }
            })
            .unwrap_or_default();
        let alias_extension_tokens = alias_extension_tokens(&mcp_crate, &alias_lits);
        let decode_name_extension_tokens =
            decode_name_extension_tokens(&mcp_crate, decode_name.as_ref());
        schema_field_tokens.push(quote! {
            {
                let mut __component_shape_mcp_field_schema =
                    <#ty as #mcp_crate::McpToolValue>::tool_value_schema();
                #description_tokens
                #alias_extension_tokens
                #decode_name_extension_tokens
                __component_shape_mcp_properties.insert(
                    #field_name_lit.to_string(),
                    __component_shape_mcp_field_schema,
                );
                if #required {
                    __component_shape_mcp_required.push(
                        #field_name_lit.to_string(),
                    );
                }
            }
        });
        let field_decode = decode_field_tokens(
            DecodeField {
                ty: &ty,
                field_name: &field_name_lit,
                aliases: &alias_lits,
                required,
                default,
                container_defaulted: schema_options.defaulted,
            },
            &mut generics,
        )?;
        field_tokens.push(quote! {
            #field_ident: #field_decode
        });
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let schema_tokens = described_schema_tokens(
        quote! {
            #mcp_crate::object_schema(
                __component_shape_mcp_properties,
                __component_shape_mcp_required,
            )
        },
        schema_options.description.as_deref(),
        ident.span(),
    );

    Ok(quote! {
        impl #impl_generics #mcp_crate::McpToolInput for #ident #ty_generics
            #where_clause
        {
            fn input_schema() -> #mcp_crate::McpSchema {
                let mut __component_shape_mcp_properties =
                    #mcp_crate::McpSchemaProperties::new();
                let mut __component_shape_mcp_required = Vec::new();

                #(#schema_field_tokens)*

                #schema_tokens
            }

            fn from_tool_call(
                __component_shape_mcp_call: #mcp_crate::McpToolCall,
            ) -> ::core::result::Result<Self, #mcp_crate::McpToolError> {
                let mut __component_shape_mcp_arguments =
                    __component_shape_mcp_call.into_arguments();
                let __component_shape_mcp_input = Self {
                    #(#field_tokens,)*
                };
                __component_shape_mcp_arguments.finish()?;
                ::core::result::Result::Ok(__component_shape_mcp_input)
            }
        }

        impl #impl_generics #mcp_crate::McpJsonSchema for #ident #ty_generics
            #where_clause
        {
            fn json_schema() -> #mcp_crate::McpSchema {
                <Self as #mcp_crate::McpToolInput>::input_schema()
            }
        }
    })
}

struct DecodeField<'a> {
    ty: &'a Type,
    field_name: &'a LitStr,
    aliases: &'a [LitStr],
    required: bool,
    default: Option<&'a FieldDefault>,
    container_defaulted: bool,
}

fn decode_field_tokens(
    field: DecodeField<'_>,
    generics: &mut syn::Generics,
) -> syn::Result<proc_macro2::TokenStream> {
    let DecodeField {
        ty,
        field_name,
        aliases,
        required,
        default,
        container_defaulted,
    } = field;
    let alias_tokens = quote! { &[#(#aliases),*] };

    if is_option_type(ty) {
        if required {
            return Ok(quote! {
                __component_shape_mcp_arguments
                    .take_required_tool_value_from::<#ty>(#field_name, #alias_tokens)?
            });
        }

        return Ok(quote! {
            __component_shape_mcp_arguments
                .take_present_tool_value_from::<#ty>(#field_name, #alias_tokens)?
                .flatten()
        });
    }

    if required {
        return Ok(quote! {
            __component_shape_mcp_arguments
                .take_required_tool_value_from::<#ty>(#field_name, #alias_tokens)?
        });
    }

    let default_tokens = match default {
        Some(default) => default.tokens(),
        None if container_defaulted => {
            generics
                .make_where_clause()
                .predicates
                .push(parse_quote!(#ty: ::core::default::Default));
            quote! { ::core::default::Default::default() }
        },
        None => {
            return Err(syn::Error::new_spanned(
                ty,
                "McpToolInput cannot decode an optional non-Option field without a default; use `Option<T>`, `#[mcp(default)]`, `#[serde(default)]`, or make the field required",
            ));
        },
    };

    Ok(quote! {
        match __component_shape_mcp_arguments
            .take_present_tool_value_from::<#ty>(#field_name, #alias_tokens)?
        {
            ::core::option::Option::Some(__component_shape_mcp_value) => {
                __component_shape_mcp_value
            }
            ::core::option::Option::None => #default_tokens,
        }
    })
}
