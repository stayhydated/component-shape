use super::*;

pub(crate) fn expand_mcp_json_schema(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let schema_options = SchemaOptions::parse(&input)?;
    let ident = input.ident;
    let mut generics = input.generics;
    let mcp_crate = schema_options
        .crate_path
        .clone()
        .map(Ok)
        .unwrap_or_else(resolve_default_mcp_crate_path)?;
    let data = match input.data {
        Data::Struct(data) => data,
        Data::Enum(data) => {
            return expand_enum_mcp_json_schema(ident, generics, data, schema_options, mcp_crate);
        },
        _ => {
            return Err(syn::Error::new(
                ident.span(),
                "McpJsonSchema can only be derived for structs or fieldless enums",
            ));
        },
    };
    if schema_options.transparent {
        let ty = transparent_field_type(&data.fields)?;
        let schema_tokens = described_schema_tokens(
            quote! { <#ty as #mcp_crate::McpJsonSchema>::json_schema() },
            schema_options.description.as_deref(),
            ident.span(),
        );
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote!(#ty: #mcp_crate::McpJsonSchema));
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
        return Ok(quote! {
            impl #impl_generics #mcp_crate::McpJsonSchema for #ident #ty_generics
                #where_clause
            {
                fn json_schema() -> #mcp_crate::McpSchema {
                    #schema_tokens
                }
            }
        });
    }

    let fields = match data.fields {
        Fields::Named(fields) => fields,
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            let ty = &fields
                .unnamed
                .first()
                .expect("newtype field should exist")
                .ty;
            let schema_tokens = described_schema_tokens(
                quote! { <#ty as #mcp_crate::McpJsonSchema>::json_schema() },
                schema_options.description.as_deref(),
                ident.span(),
            );
            generics
                .make_where_clause()
                .predicates
                .push(parse_quote!(#ty: #mcp_crate::McpJsonSchema));
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
            return Ok(quote! {
                impl #impl_generics #mcp_crate::McpJsonSchema for #ident #ty_generics
                    #where_clause
                {
                    fn json_schema() -> #mcp_crate::McpSchema {
                        #schema_tokens
                    }
                }
            });
        },
        _ => {
            return Err(syn::Error::new(
                ident.span(),
                "McpJsonSchema requires a struct with named fields or a single-field tuple newtype",
            ));
        },
    };

    let mut field_tokens = Vec::new();
    let mut field_names = BTreeMap::new();

    for field in fields.named {
        let Some(field_ident) = field.ident.clone() else {
            continue;
        };
        let options = FieldOptions::parse(&field)?;
        if options.skip {
            continue;
        }

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
            &mut field_names,
            &field_name,
            field_ident.span(),
            "MCP schema field or alias",
        )?;
        for alias in &options.aliases {
            claim_wire_name(
                &mut field_names,
                alias,
                field_ident.span(),
                "MCP schema field or alias",
            )?;
        }
        let field_name = LitStr::new(&field_name, field_ident.span());
        let alias_lits = options
            .aliases
            .iter()
            .map(|alias| LitStr::new(alias, field_ident.span()))
            .collect::<Vec<_>>();
        let decode_name = (field_name.value() != serde_field_name)
            .then(|| LitStr::new(&serde_field_name, field_ident.span()));
        let description = options
            .description
            .or_else(|| doc_description(&field.attrs))
            .map(|description| LitStr::new(&description, field_ident.span()));
        let required = options.required.unwrap_or_else(|| {
            !is_option_type(&field.ty) && !defaulted && !schema_options.defaulted
        });
        let ty = field.ty;
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote!(#ty: #mcp_crate::McpJsonSchema));
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

        field_tokens.push(quote! {
            {
                let mut __component_shape_mcp_field_schema =
                    <#ty as #mcp_crate::McpJsonSchema>::json_schema();
                #description_tokens
                #alias_extension_tokens
                #decode_name_extension_tokens
                __component_shape_mcp_properties.insert(
                    #field_name.to_string(),
                    __component_shape_mcp_field_schema,
                );
                if #required {
                    __component_shape_mcp_required.push(
                        #field_name.to_string(),
                    );
                }
            }
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
        impl #impl_generics #mcp_crate::McpJsonSchema for #ident #ty_generics
            #where_clause
        {
            fn json_schema() -> #mcp_crate::McpSchema {
                let mut __component_shape_mcp_properties =
                    #mcp_crate::McpSchemaProperties::new();
                let mut __component_shape_mcp_required = Vec::new();

                #(#field_tokens)*

                #schema_tokens
            }
        }
    })
}

pub(crate) fn described_schema_tokens(
    schema_tokens: proc_macro2::TokenStream,
    description: Option<&str>,
    span: proc_macro2::Span,
) -> proc_macro2::TokenStream {
    let Some(description) = description else {
        return schema_tokens;
    };
    let description = LitStr::new(description, span);
    quote! {
        #schema_tokens.with_description(#description)
    }
}

pub(crate) fn alias_extension_tokens(
    mcp_crate: &Path,
    aliases: &[LitStr],
) -> proc_macro2::TokenStream {
    if aliases.is_empty() {
        return proc_macro2::TokenStream::new();
    }

    quote! {
        __component_shape_mcp_field_schema.set_extension(
            "x-mcpAliases",
            #mcp_crate::serde_json::json!([#(#aliases),*]),
        );
    }
}

pub(crate) fn decode_name_extension_tokens(
    mcp_crate: &Path,
    decode_name: Option<&LitStr>,
) -> proc_macro2::TokenStream {
    let Some(decode_name) = decode_name else {
        return proc_macro2::TokenStream::new();
    };

    quote! {
        __component_shape_mcp_field_schema.set_extension(
            "x-mcpDecodeName",
            #mcp_crate::serde_json::Value::String(#decode_name.to_string()),
        );
    }
}

fn transparent_field_type(fields: &Fields) -> syn::Result<Type> {
    match fields {
        Fields::Named(fields) if fields.named.len() == 1 => Ok(fields
            .named
            .first()
            .expect("transparent named field should exist")
            .ty
            .clone()),
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => Ok(fields
            .unnamed
            .first()
            .expect("transparent tuple field should exist")
            .ty
            .clone()),
        _ => Err(syn::Error::new_spanned(
            fields,
            "McpJsonSchema transparent structs must have exactly one field",
        )),
    }
}

fn expand_enum_mcp_json_schema(
    ident: Ident,
    generics: syn::Generics,
    data: syn::DataEnum,
    schema_options: SchemaOptions,
    mcp_crate: Path,
) -> syn::Result<proc_macro2::TokenStream> {
    let mut value_tokens = Vec::new();
    let mut decode_aliases = Vec::new();
    let mut enum_values = BTreeMap::new();

    for variant in data.variants {
        let options = VariantOptions::parse(&variant)?;
        if options.skip {
            continue;
        }
        if !matches!(&variant.fields, Fields::Unit) {
            return Err(syn::Error::new_spanned(
                variant.ident,
                "McpJsonSchema can only infer fieldless enum variants; add `#[mcp(skip)]` or implement `McpJsonSchema` manually",
            ));
        }

        let rust_variant_name = variant.ident.to_string();
        let serde_name = options
            .serde_rename
            .clone()
            .or_else(|| {
                schema_options
                    .serde_rename_all
                    .map(|rule| rule.apply(&rust_variant_name))
            })
            .unwrap_or_else(|| rust_variant_name.clone());
        let primary_name = options
            .mcp_rename
            .clone()
            .or_else(|| {
                schema_options
                    .mcp_rename_all
                    .map(|rule| rule.apply(&rust_variant_name))
            })
            .unwrap_or_else(|| serde_name.clone());
        claim_wire_name(
            &mut enum_values,
            &primary_name,
            variant.ident.span(),
            "MCP enum value",
        )?;
        if primary_name != serde_name {
            decode_aliases.push((
                LitStr::new(&primary_name, variant.ident.span()),
                LitStr::new(&serde_name, variant.ident.span()),
            ));
        }
        value_tokens.push(enum_value_push_tokens(
            &mcp_crate,
            &primary_name,
            variant.ident.span(),
        ));
        for alias in options.aliases {
            claim_wire_name(
                &mut enum_values,
                &alias,
                variant.ident.span(),
                "MCP enum value",
            )?;
            if alias != serde_name {
                decode_aliases.push((
                    LitStr::new(&alias, variant.ident.span()),
                    LitStr::new(&serde_name, variant.ident.span()),
                ));
            }
            value_tokens.push(enum_value_push_tokens(
                &mcp_crate,
                &alias,
                variant.ident.span(),
            ));
        }
    }

    if value_tokens.is_empty() {
        return Err(syn::Error::new(
            ident.span(),
            "McpJsonSchema enum schemas require at least one visible unit variant",
        ));
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let enum_decode_alias_extension_tokens =
        enum_decode_alias_extension_tokens(&mcp_crate, &decode_aliases);
    let schema_tokens = described_schema_tokens(
        quote! {
            {
                let mut __component_shape_mcp_schema =
                    #mcp_crate::McpSchema::new(#mcp_crate::serde_json::json!({
                        "type": "string",
                        "enum": __component_shape_mcp_enum_values
                    }));
                #enum_decode_alias_extension_tokens
                __component_shape_mcp_schema
            }
        },
        schema_options.description.as_deref(),
        ident.span(),
    );

    Ok(quote! {
        impl #impl_generics #mcp_crate::McpJsonSchema for #ident #ty_generics
            #where_clause
        {
            fn json_schema() -> #mcp_crate::McpSchema {
                let mut __component_shape_mcp_enum_values = Vec::new();
                #(#value_tokens)*
                #schema_tokens
            }
        }
    })
}

fn enum_value_push_tokens(
    mcp_crate: &Path,
    value: &str,
    span: proc_macro2::Span,
) -> proc_macro2::TokenStream {
    let value = LitStr::new(value, span);
    quote! {
        __component_shape_mcp_enum_values.push(
            #mcp_crate::serde_json::Value::String(#value.to_string())
        );
    }
}

fn enum_decode_alias_extension_tokens(
    mcp_crate: &Path,
    aliases: &[(LitStr, LitStr)],
) -> proc_macro2::TokenStream {
    if aliases.is_empty() {
        return proc_macro2::TokenStream::new();
    }

    let wire_values = aliases.iter().map(|(wire, _)| wire);
    let decode_values = aliases.iter().map(|(_, decode)| decode);

    quote! {
        __component_shape_mcp_schema.set_extension(
            "x-mcpEnumDecodeAliases",
            #mcp_crate::serde_json::json!({
                #(#wire_values: #decode_values),*
            }),
        );
    }
}
