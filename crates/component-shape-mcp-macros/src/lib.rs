use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use quote::quote;
use std::collections::BTreeMap;
use syn::{
    Attribute, Data, DeriveInput, Expr, Fields, GenericArgument, Ident, LitBool, LitStr, Path,
    PathArguments, Type, parse_macro_input, parse_quote, spanned::Spanned as _,
};

/// Derive JSON Schema metadata for structs, transparent newtypes, and
/// fieldless enums used in MCP tool schemas.
#[proc_macro_derive(McpJsonSchema, attributes(mcp, serde))]
pub fn derive_mcp_json_schema(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_mcp_json_schema(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Derive a top-level MCP input schema and strict MCP argument decoding for a
/// named tool input struct.
#[proc_macro_derive(McpToolInput, attributes(mcp, serde))]
pub fn derive_mcp_tool_input(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_mcp_tool_input(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_mcp_tool_input(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    expand_mcp_tool_input_impl(input)
}

fn expand_mcp_json_schema(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
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

fn described_schema_tokens(
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

fn alias_extension_tokens(mcp_crate: &Path, aliases: &[LitStr]) -> proc_macro2::TokenStream {
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

fn decode_name_extension_tokens(
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

#[derive(Default)]
struct SchemaOptions {
    crate_path: Option<Path>,
    description: Option<String>,
    mcp_rename_all: Option<RenameRule>,
    serde_rename_all: Option<RenameRule>,
    defaulted: bool,
    transparent: bool,
}

impl SchemaOptions {
    fn parse(input: &DeriveInput) -> syn::Result<Self> {
        let mut options = Self::default();
        for attr in input
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("mcp"))
        {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("crate") {
                    set_option(
                        &mut options.crate_path,
                        parse_path_value(&meta)?,
                        "crate",
                        meta.path.span(),
                    )
                } else if meta.path.is_ident("description") {
                    set_option(
                        &mut options.description,
                        meta.value()?.parse::<LitStr>()?.value(),
                        "description",
                        meta.path.span(),
                    )
                } else if meta.path.is_ident("rename_all") {
                    set_option(
                        &mut options.mcp_rename_all,
                        parse_rename_rule_value(&meta)?,
                        "rename_all",
                        meta.path.span(),
                    )
                } else if meta.path.is_ident("default") {
                    set_flag(
                        &mut options.defaulted,
                        parse_bool_flag_or_value(&meta)?,
                        "default",
                        meta.path.span(),
                    )
                } else if meta.path.is_ident("transparent") {
                    set_flag(
                        &mut options.transparent,
                        parse_bool_flag_or_value(&meta)?,
                        "transparent",
                        meta.path.span(),
                    )
                } else {
                    Err(meta.error("unknown `mcp` container option"))
                }
            })?;
        }
        for attr in input
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("serde"))
        {
            parse_container_serde_attr(attr, &mut options)?;
        }
        if options.description.is_none() {
            options.description = doc_description(&input.attrs);
        }
        Ok(options)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RenameRule {
    Lower,
    Upper,
    Pascal,
    Camel,
    Snake,
    ScreamingSnake,
    Kebab,
    ScreamingKebab,
}

impl RenameRule {
    fn parse_lit(value: &LitStr) -> syn::Result<Self> {
        match value.value().as_str() {
            "lowercase" => Ok(Self::Lower),
            "UPPERCASE" => Ok(Self::Upper),
            "PascalCase" => Ok(Self::Pascal),
            "camelCase" => Ok(Self::Camel),
            "snake_case" => Ok(Self::Snake),
            "SCREAMING_SNAKE_CASE" => Ok(Self::ScreamingSnake),
            "kebab-case" => Ok(Self::Kebab),
            "SCREAMING-KEBAB-CASE" => Ok(Self::ScreamingKebab),
            _ => Err(syn::Error::new(
                value.span(),
                format!("unsupported serde rename_all rule `{}`", value.value()),
            )),
        }
    }

    fn apply(self, ident: &str) -> String {
        let words = split_words(ident);
        match self {
            Self::Lower => words.concat().to_ascii_lowercase(),
            Self::Upper => words.concat().to_ascii_uppercase(),
            Self::Pascal => words.iter().map(|word| capitalize(word)).collect(),
            Self::Camel => {
                let mut renamed = String::new();
                for (index, word) in words.iter().enumerate() {
                    if index == 0 {
                        renamed.push_str(&word.to_ascii_lowercase());
                    } else {
                        renamed.push_str(&capitalize(word));
                    }
                }
                renamed
            },
            Self::Snake => words.join("_").to_ascii_lowercase(),
            Self::ScreamingSnake => words.join("_").to_ascii_uppercase(),
            Self::Kebab => words.join("-").to_ascii_lowercase(),
            Self::ScreamingKebab => words.join("-").to_ascii_uppercase(),
        }
    }
}

fn split_words(ident: &str) -> Vec<String> {
    #[derive(Clone, Copy, Eq, PartialEq)]
    enum CharKind {
        Upper,
        Lower,
        Digit,
    }

    fn char_kind(ch: char) -> Option<CharKind> {
        if ch.is_ascii_uppercase() {
            Some(CharKind::Upper)
        } else if ch.is_ascii_lowercase() {
            Some(CharKind::Lower)
        } else if ch.is_ascii_digit() {
            Some(CharKind::Digit)
        } else {
            None
        }
    }

    let chars = ident.chars().collect::<Vec<_>>();
    let mut words = Vec::new();
    let mut current = String::new();
    let mut previous_kind = None;

    for (index, ch) in chars.iter().copied().enumerate() {
        let Some(kind) = char_kind(ch) else {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            previous_kind = None;
            continue;
        };

        let next_kind = chars.get(index + 1).and_then(|ch| char_kind(*ch));
        let boundary = !current.is_empty()
            && matches!(
                (previous_kind, kind, next_kind),
                (Some(CharKind::Lower), CharKind::Upper, _)
                    | (Some(CharKind::Digit), CharKind::Upper | CharKind::Lower, _)
                    | (Some(CharKind::Upper | CharKind::Lower), CharKind::Digit, _)
                    | (
                        Some(CharKind::Upper),
                        CharKind::Upper,
                        Some(CharKind::Lower)
                    )
            );

        if boundary {
            words.push(std::mem::take(&mut current));
        }
        current.push(ch);
        previous_kind = Some(kind);
    }

    if !current.is_empty() {
        words.push(current);
    }

    words
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut output = String::new();
    output.extend(first.to_uppercase());
    output.push_str(&chars.as_str().to_ascii_lowercase());
    output
}

#[derive(Default)]
struct FieldOptions {
    mcp_rename: Option<String>,
    serde_rename: Option<String>,
    aliases: Vec<String>,
    description: Option<String>,
    required: Option<bool>,
    default_value: Option<FieldDefault>,
    flatten: Option<proc_macro2::Span>,
    skip: bool,
}

#[derive(Clone)]
enum FieldDefault {
    Default,
    Expr(Expr),
    CallPath(Path),
}

impl FieldDefault {
    fn tokens(&self) -> proc_macro2::TokenStream {
        match self {
            Self::Default => quote! { ::core::default::Default::default() },
            Self::Expr(expr) => quote! { #expr },
            Self::CallPath(path) => quote! { #path() },
        }
    }
}

impl FieldOptions {
    fn parse(field: &syn::Field) -> syn::Result<Self> {
        let mut options = Self::default();
        for attr in &field.attrs {
            if attr.path().is_ident("mcp") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("rename") {
                        set_option(
                            &mut options.mcp_rename,
                            meta.value()?.parse::<LitStr>()?.value(),
                            "rename",
                            meta.path.span(),
                        )
                    } else if meta.path.is_ident("alias") {
                        options
                            .aliases
                            .push(meta.value()?.parse::<LitStr>()?.value());
                        Ok(())
                    } else if meta.path.is_ident("description") {
                        set_option(
                            &mut options.description,
                            meta.value()?.parse::<LitStr>()?.value(),
                            "description",
                            meta.path.span(),
                        )
                    } else if meta.path.is_ident("required") {
                        set_option(
                            &mut options.required,
                            parse_bool_flag_or_value(&meta)?,
                            "required",
                            meta.path.span(),
                        )
                    } else if meta.path.is_ident("optional") {
                        set_option(
                            &mut options.required,
                            !parse_bool_flag_or_value(&meta)?,
                            "optional",
                            meta.path.span(),
                        )
                    } else if meta.path.is_ident("skip") {
                        set_flag(
                            &mut options.skip,
                            parse_bool_flag_or_value(&meta)?,
                            "skip",
                            meta.path.span(),
                        )
                    } else if meta.path.is_ident("default") {
                        if let Some(default_value) = parse_mcp_default_value(&meta)? {
                            set_option(
                                &mut options.default_value,
                                default_value,
                                "default",
                                meta.path.span(),
                            )
                        } else {
                            Ok(())
                        }
                    } else {
                        Err(meta.error("unknown `mcp` field option"))
                    }
                })?;
            } else if attr.path().is_ident("serde") {
                parse_serde_attr(attr, &mut options)?;
            }
        }
        if let Some(flatten_span) = options.flatten
            && !options.skip
        {
            return Err(syn::Error::new(
                flatten_span,
                "McpJsonSchema cannot infer `serde(flatten)` fields; add `#[mcp(skip)]` or implement `McpJsonSchema` manually",
            ));
        }
        Ok(options)
    }

    fn defaulted(&self) -> bool {
        self.default_value.is_some()
    }
}

#[derive(Default)]
struct VariantOptions {
    mcp_rename: Option<String>,
    serde_rename: Option<String>,
    aliases: Vec<String>,
    skip: bool,
}

impl VariantOptions {
    fn parse(variant: &syn::Variant) -> syn::Result<Self> {
        let mut options = Self::default();
        for attr in &variant.attrs {
            if attr.path().is_ident("mcp") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("rename") {
                        set_option(
                            &mut options.mcp_rename,
                            meta.value()?.parse::<LitStr>()?.value(),
                            "rename",
                            meta.path.span(),
                        )
                    } else if meta.path.is_ident("alias") {
                        options
                            .aliases
                            .push(meta.value()?.parse::<LitStr>()?.value());
                        Ok(())
                    } else if meta.path.is_ident("skip") {
                        set_flag(
                            &mut options.skip,
                            parse_bool_flag_or_value(&meta)?,
                            "skip",
                            meta.path.span(),
                        )
                    } else {
                        Err(meta.error("unknown `mcp` enum variant option"))
                    }
                })?;
            } else if attr.path().is_ident("serde") {
                parse_variant_serde_attr(attr, &mut options)?;
            }
        }
        Ok(options)
    }
}

fn parse_serde_attr(attr: &syn::Attribute, options: &mut FieldOptions) -> syn::Result<()> {
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("rename") {
            if let Some(rename) = parse_serde_rename(&meta)?
                && options.serde_rename.is_none()
            {
                options.serde_rename = Some(rename);
            }
            Ok(())
        } else if meta.path.is_ident("skip") {
            options.skip = true;
            Ok(())
        } else if meta.path.is_ident("skip_deserializing") {
            options.skip = true;
            consume_optional_serde_meta_value(&meta)
        } else if meta.path.is_ident("default") {
            if options.default_value.is_none() {
                options.default_value = Some(parse_serde_default_value(&meta)?);
            } else {
                consume_optional_serde_meta_value(&meta)?;
            }
            Ok(())
        } else if meta.path.is_ident("alias") {
            options
                .aliases
                .push(meta.value()?.parse::<LitStr>()?.value());
            Ok(())
        } else if meta.path.is_ident("flatten") {
            options.flatten = Some(meta.path.span());
            Ok(())
        } else if meta.path.is_ident("skip_serializing")
            || meta.path.is_ident("skip_serializing_if")
            || meta.path.is_ident("with")
            || meta.path.is_ident("serialize_with")
            || meta.path.is_ident("deserialize_with")
            || meta.path.is_ident("rename_all")
            || meta.path.is_ident("bound")
            || meta.path.is_ident("borrow")
        {
            consume_optional_serde_meta_value(&meta)
        } else {
            consume_optional_serde_meta_value(&meta)
        }
    })
}

fn parse_variant_serde_attr(
    attr: &syn::Attribute,
    options: &mut VariantOptions,
) -> syn::Result<()> {
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("rename") {
            if let Some(rename) = parse_serde_rename(&meta)?
                && options.serde_rename.is_none()
            {
                options.serde_rename = Some(rename);
            }
            Ok(())
        } else if meta.path.is_ident("alias") {
            options
                .aliases
                .push(meta.value()?.parse::<LitStr>()?.value());
            Ok(())
        } else if meta.path.is_ident("skip")
            || meta.path.is_ident("skip_deserializing")
            || meta.path.is_ident("other")
        {
            options.skip = true;
            consume_optional_serde_meta_value(&meta)
        } else {
            consume_optional_serde_meta_value(&meta)
        }
    })
}

fn parse_container_serde_attr(
    attr: &syn::Attribute,
    options: &mut SchemaOptions,
) -> syn::Result<()> {
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("rename_all") {
            if let Some(rename_all) = parse_serde_rename_rule(&meta)?
                && options.serde_rename_all.is_none()
            {
                options.serde_rename_all = Some(rename_all);
            }
            Ok(())
        } else if meta.path.is_ident("default") {
            options.defaulted = true;
            consume_optional_serde_meta_value(&meta)
        } else if meta.path.is_ident("transparent") {
            options.transparent = true;
            consume_optional_serde_meta_value(&meta)
        } else {
            consume_optional_serde_meta_value(&meta)
        }
    })
}

fn parse_serde_rename(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<Option<String>> {
    if meta.input.peek(syn::Token![=]) {
        return Ok(Some(meta.value()?.parse::<LitStr>()?.value()));
    }

    let mut deserialize = None;
    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("deserialize") {
            deserialize = Some(nested.value()?.parse::<LitStr>()?.value());
            Ok(())
        } else if nested.path.is_ident("serialize") {
            let _ = nested.value()?.parse::<LitStr>()?;
            Ok(())
        } else {
            Ok(())
        }
    })?;
    Ok(deserialize)
}

fn parse_serde_rename_rule(
    meta: &syn::meta::ParseNestedMeta<'_>,
) -> syn::Result<Option<RenameRule>> {
    if meta.input.peek(syn::Token![=]) {
        let value = meta.value()?.parse::<LitStr>()?;
        return RenameRule::parse_lit(&value).map(Some);
    }

    let mut deserialize = None;
    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("deserialize") {
            let value = nested.value()?.parse::<LitStr>()?;
            deserialize = Some(RenameRule::parse_lit(&value)?);
            Ok(())
        } else if nested.path.is_ident("serialize") {
            let _ = nested.value()?.parse::<LitStr>()?;
            Ok(())
        } else {
            Ok(())
        }
    })?;
    Ok(deserialize)
}

fn parse_mcp_default_value(
    meta: &syn::meta::ParseNestedMeta<'_>,
) -> syn::Result<Option<FieldDefault>> {
    if !meta.input.peek(syn::Token![=]) {
        return Ok(Some(FieldDefault::Default));
    }

    let expr = meta.value()?.parse::<Expr>()?;
    Ok(Some(FieldDefault::Expr(expr)))
}

fn parse_serde_default_value(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<FieldDefault> {
    if !meta.input.peek(syn::Token![=]) {
        return Ok(FieldDefault::Default);
    }

    let value = meta.value()?.parse::<LitStr>()?;
    Ok(FieldDefault::CallPath(value.parse()?))
}

fn consume_optional_serde_meta_value(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<()> {
    if meta.input.peek(syn::Token![=]) {
        let _ = meta.value()?.parse::<Expr>()?;
    } else if meta.input.peek(syn::token::Paren) {
        let content;
        syn::parenthesized!(content in meta.input);
        let _ = content.parse::<proc_macro2::TokenStream>()?;
    }
    Ok(())
}

fn doc_description(attrs: &[Attribute]) -> Option<String> {
    let mut lines = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .filter_map(|attr| match &attr.meta {
            syn::Meta::NameValue(meta) => match &meta.value {
                Expr::Lit(expr) => match &expr.lit {
                    syn::Lit::Str(value) => Some(value.value().trim().to_string()),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();

    let first = lines.iter().position(|line| !line.is_empty())?;
    let last = lines.iter().rposition(|line| !line.is_empty())?;
    lines.drain(..first);
    lines.truncate(last - first + 1);

    Some(lines.join("\n"))
}

fn parse_rename_rule_value(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<RenameRule> {
    RenameRule::parse_lit(&meta.value()?.parse::<LitStr>()?)
}

fn parse_path_value(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<Path> {
    meta.input.parse::<syn::Token![=]>()?;
    if meta.input.peek(LitStr) {
        meta.input.parse::<LitStr>()?.parse()
    } else {
        meta.input.parse()
    }
}

fn set_option<T>(
    slot: &mut Option<T>,
    value: T,
    option_name: &'static str,
    span: proc_macro2::Span,
) -> syn::Result<()> {
    if slot.is_some() {
        return Err(syn::Error::new(
            span,
            format!("duplicate `{option_name}` option"),
        ));
    }
    *slot = Some(value);
    Ok(())
}

fn set_flag(
    slot: &mut bool,
    value: bool,
    option_name: &'static str,
    span: proc_macro2::Span,
) -> syn::Result<()> {
    if *slot && value {
        return Err(syn::Error::new(
            span,
            format!("duplicate `{option_name}` option"),
        ));
    }
    *slot = value;
    Ok(())
}

fn parse_bool_flag_or_value(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<bool> {
    if meta.input.peek(syn::Token![=]) {
        Ok(meta.value()?.parse::<LitBool>()?.value)
    } else {
        Ok(true)
    }
}

fn is_option_type(ty: &Type) -> bool {
    option_type_argument(ty).is_some()
}

fn option_type_argument(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }
    let segment = path.path.segments.last()?;
    if segment.ident != "Option" {
        return None;
    }
    single_type_argument(&segment.arguments)
}

fn single_type_argument(arguments: &PathArguments) -> Option<&Type> {
    let PathArguments::AngleBracketed(arguments) = arguments else {
        return None;
    };
    let mut arguments = arguments.args.iter();
    let GenericArgument::Type(ty) = arguments.next()? else {
        return None;
    };
    if arguments.next().is_some() {
        return None;
    }
    Some(ty)
}

fn claim_wire_name(
    names: &mut BTreeMap<String, proc_macro2::Span>,
    name: &str,
    span: proc_macro2::Span,
    label: &'static str,
) -> syn::Result<()> {
    if let Some(first_span) = names.get(name).copied() {
        let mut error = syn::Error::new(span, format!("duplicate {label} name `{name}`"));
        error.combine(syn::Error::new(first_span, "first declared here"));
        return Err(error);
    }

    names.insert(name.to_string(), span);
    Ok(())
}

fn resolve_default_mcp_crate_path() -> syn::Result<Path> {
    if let Some(path) = resolve_package_path("component-shape-mcp", "crate", None) {
        return Ok(path);
    }

    let facade_paths = [
        ("gpui-form", "crate::mcp", "mcp"),
        ("gpui-table", "crate::mcp", "mcp"),
    ]
    .into_iter()
    .filter_map(|(package, itself_path, module)| {
        resolve_package_path(package, itself_path, Some(module)).map(|path| (package, path))
    })
    .collect::<Vec<_>>();

    match facade_paths.as_slice() {
        [(_, path)] => Ok(path.clone()),
        [] => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "MCP derive could not find `component-shape-mcp`, `gpui-form`, or `gpui-table` in this crate's manifest; add one of those dependencies or set `#[mcp(crate = path::to::mcp)]`",
        )),
        _ => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "MCP derive found multiple MCP facade crates; add `#[mcp(crate = path::to::mcp)]` to choose one",
        )),
    }
}

fn resolve_package_path(package: &str, itself_path: &str, module: Option<&str>) -> Option<Path> {
    let path = match crate_name(package).ok()? {
        FoundCrate::Itself => itself_path.to_string(),
        FoundCrate::Name(name) => match module {
            Some(module) => format!("::{name}::{module}"),
            None => format!("::{name}"),
        },
    };
    syn::parse_str(&path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens as _;

    #[test]
    fn derive_schema_uses_field_attributes() {
        let input: DeriveInput = syn::parse_quote! {
            struct SearchArgs {
                #[mcp(rename = "q", description = "Search text")]
                query: String,
                page: Option<u32>,
                #[serde(skip)]
                internal: String,
            }
        };

        let expanded = expand_mcp_json_schema(input)
            .expect("schema derive should expand")
            .to_token_stream()
            .to_string();

        assert!(expanded.contains("\"q\""));
        assert!(expanded.contains("Search text"));
        assert!(expanded.contains("x-mcpDecodeName"));
        assert!(expanded.contains("page"));
        assert!(!expanded.contains("internal"));
    }

    #[test]
    fn derive_schema_infers_doc_descriptions() {
        let input: DeriveInput = syn::parse_quote! {
            /// Search arguments sent to the tool.
            ///
            /// Blank doc lines are preserved between paragraphs.
            struct SearchArgs {
                /// Full text query.
                query: String,
            }
        };

        let expanded = expand_mcp_json_schema(input)
            .expect("schema derive should expand")
            .to_token_stream()
            .to_string();

        assert!(expanded.contains("Search arguments sent to the tool."));
        assert!(expanded.contains("Blank doc lines are preserved between paragraphs."));
        assert!(expanded.contains("Full text query."));
    }

    #[test]
    fn derive_schema_supports_tuple_newtypes() {
        let input: DeriveInput = syn::parse_quote! {
            struct UserId(u64);
        };

        let expanded = expand_mcp_json_schema(input)
            .expect("newtype schema derive should expand")
            .to_token_stream()
            .to_string();

        assert!(expanded.contains("< u64 as"));
        assert!(expanded.contains("json_schema"));
    }

    #[test]
    fn derive_schema_supports_named_transparent_newtypes() {
        let input: DeriveInput = syn::parse_quote! {
            #[serde(transparent)]
            struct UserId {
                value: u64,
            }
        };

        let expanded = expand_mcp_json_schema(input)
            .expect("transparent named newtype schema derive should expand")
            .to_token_stream()
            .to_string();

        assert!(expanded.contains("< u64 as"));
        assert!(expanded.contains("json_schema"));
        assert!(!expanded.contains("object_schema"));
    }

    #[test]
    fn derive_schema_rejects_multi_field_transparent_structs() {
        let input: DeriveInput = syn::parse_quote! {
            #[mcp(transparent)]
            struct Bad {
                min: u32,
                max: u32,
            }
        };

        let error = expand_mcp_json_schema(input).unwrap_err().to_string();

        assert!(error.contains("transparent structs must have exactly one field"));
    }

    #[test]
    fn derive_schema_accepts_crate_override() {
        let input: DeriveInput = syn::parse_quote! {
            #[mcp(crate = gpui_form::mcp)]
            struct UserId(u64);
        };

        let expanded = expand_mcp_json_schema(input)
            .expect("crate path override should expand")
            .to_token_stream()
            .to_string();

        assert!(expanded.contains("gpui_form :: mcp :: McpJsonSchema"));
    }

    #[test]
    fn derive_schema_rejects_legacy_crate_path_option() {
        let input: DeriveInput = syn::parse_quote! {
            #[mcp(crate_path = gpui_form::mcp)]
            struct UserId(u64);
        };

        let error = expand_mcp_json_schema(input).unwrap_err().to_string();

        assert!(error.contains("unknown `mcp` container option"));
    }

    #[test]
    fn derive_schema_rejects_field_name_alias() {
        let input: DeriveInput = syn::parse_quote! {
            struct SearchArgs {
                #[mcp(name = "q")]
                query: String,
            }
        };

        let error = expand_mcp_json_schema(input).unwrap_err().to_string();

        assert!(error.contains("unknown `mcp` field option"));
    }

    #[test]
    fn derive_schema_rejects_duplicate_field_names() {
        let input: DeriveInput = syn::parse_quote! {
            #[serde(rename_all = "camelCase")]
            struct SearchArgs {
                query_text: String,
                #[mcp(rename = "queryText")]
                query: String,
            }
        };

        let error = expand_mcp_json_schema(input).unwrap_err().to_string();

        assert!(error.contains("duplicate MCP schema field or alias name `queryText`"));
    }

    #[test]
    fn derive_schema_supports_fieldless_enums() {
        let input: DeriveInput = syn::parse_quote! {
            #[serde(rename_all = "kebab-case")]
            enum IssueState {
                Open,
                #[serde(alias = "reviewing")]
                InReview,
                #[mcp(rename = "done", alias = "resolved")]
                Closed,
                #[serde(other)]
                Unknown,
            }
        };

        let expanded = expand_mcp_json_schema(input)
            .expect("enum schema derive should expand")
            .to_token_stream()
            .to_string();

        assert!(expanded.contains("open"));
        assert!(expanded.contains("in-review"));
        assert!(expanded.contains("reviewing"));
        assert!(expanded.contains("done"));
        assert!(expanded.contains("resolved"));
        assert!(expanded.contains("x-mcpEnumDecodeAliases"));
        assert!(!expanded.contains("Unknown"));
    }

    #[test]
    fn derive_schema_rejects_duplicate_enum_values() {
        let input: DeriveInput = syn::parse_quote! {
            enum IssueState {
                Open,
                #[serde(alias = "Open")]
                InReview,
            }
        };

        let error = expand_mcp_json_schema(input).unwrap_err().to_string();

        assert!(error.contains("duplicate MCP enum value name `Open`"));
    }

    #[test]
    fn derive_tool_input_generates_schema_and_strict_decoder() {
        let input: DeriveInput = syn::parse_quote! {
            #[serde(rename_all = "camelCase")]
            struct SearchArgs {
                #[serde(rename(deserialize = "q"), alias = "queryText")]
                query: String,
                page_size: Option<u32>,
                #[serde(default = "default_limit")]
                limit: usize,
                #[serde(skip)]
                internal: String,
            }
        };

        let expanded = expand_mcp_tool_input(input)
            .expect("tool input derive should expand")
            .to_token_stream()
            .to_string();

        assert!(expanded.contains("McpToolInput for SearchArgs"));
        assert!(expanded.contains("McpJsonSchema for SearchArgs"));
        assert!(expanded.contains("McpToolValue"));
        assert!(expanded.contains("x-mcpAliases"));
        assert!(expanded.contains("take_required_tool_value_from :: < String >"));
        assert!(expanded.contains("take_present_tool_value_from :: < Option < u32 > >"));
        assert!(expanded.contains("default_limit ()"));
        assert!(expanded.contains("\"queryText\""));
    }

    #[test]
    fn derive_tool_input_treats_mcp_default_value_as_rust_expression() {
        let input: DeriveInput = syn::parse_quote! {
            struct SearchArgs {
                #[mcp(optional, default = false)]
                include_archived: bool,
            }
        };

        let expanded = expand_mcp_tool_input(input)
            .expect("tool input derive should expand")
            .to_token_stream()
            .to_string();

        assert!(expanded.contains("false"));
    }

    #[test]
    fn derive_tool_input_rejects_tuple_structs() {
        let input: DeriveInput = syn::parse_quote! {
            struct SearchArgs(String);
        };

        let error = expand_mcp_tool_input(input).unwrap_err().to_string();

        assert!(error.contains("struct with named fields"));
    }

    #[test]
    fn derive_tool_input_rejects_optional_non_option_fields_without_defaults() {
        let input: DeriveInput = syn::parse_quote! {
            struct SearchArgs {
                #[mcp(optional)]
                limit: usize,
            }
        };

        let error = expand_mcp_tool_input(input).unwrap_err().to_string();

        assert!(error.contains("optional non-Option field without a default"));
    }

    #[test]
    fn derive_tool_input_rejects_duplicate_field_aliases() {
        let input: DeriveInput = syn::parse_quote! {
            struct SearchArgs {
                #[serde(alias = "q")]
                query: String,
                #[mcp(rename = "q")]
                text: String,
            }
        };

        let error = expand_mcp_tool_input(input).unwrap_err().to_string();

        assert!(error.contains("duplicate MCP tool input field or alias name `q`"));
    }

    #[test]
    fn derive_tool_input_rejects_alias_matching_primary_name() {
        let input: DeriveInput = syn::parse_quote! {
            struct SearchArgs {
                #[serde(alias = "query")]
                query: String,
            }
        };

        let error = expand_mcp_tool_input(input).unwrap_err().to_string();

        assert!(error.contains("duplicate MCP tool input field or alias name `query`"));
    }

    #[test]
    fn derive_schema_rejects_data_bearing_enums() {
        let input: DeriveInput = syn::parse_quote! {
            enum Event {
                Click { x: u32, y: u32 },
            }
        };

        let error = expand_mcp_json_schema(input).unwrap_err().to_string();

        assert!(error.contains("fieldless enum variants"));
    }

    #[test]
    fn derive_schema_follows_serde_deserialize_metadata() {
        let input: DeriveInput = syn::parse_quote! {
            #[serde(rename_all = "camelCase")]
            struct SearchArgs {
                created_at: String,
                #[serde(rename(deserialize = "q", serialize = "query"))]
                query: String,
                #[serde(default)]
                page_size: u32,
                #[serde(skip_deserializing)]
                cache_key: String,
            }
        };

        let expanded = expand_mcp_json_schema(input)
            .expect("schema derive should expand")
            .to_token_stream()
            .to_string();

        assert!(expanded.contains("\"createdAt\""));
        assert!(expanded.contains("\"q\""));
        assert!(expanded.contains("\"pageSize\""));
        assert!(!expanded.contains("\"cache_key\""));
        assert!(!expanded.contains("\"cacheKey\""));
        assert!(expanded.contains("if false"));
    }

    #[test]
    fn derive_schema_rejects_flattened_fields() {
        let input: DeriveInput = syn::parse_quote! {
            struct SearchArgs {
                #[serde(flatten)]
                filters: Filters,
            }
        };

        let error = expand_mcp_json_schema(input).unwrap_err().to_string();

        assert!(error.contains("cannot infer `serde(flatten)`"));
    }

    #[test]
    fn derive_schema_rejects_unknown_container_mcp_options() {
        let input: DeriveInput = syn::parse_quote! {
            #[mcp(schema = "anything")]
            struct SearchArgs {
                query: String,
            }
        };

        let error = expand_mcp_json_schema(input).unwrap_err().to_string();

        assert!(error.contains("unknown `mcp` container option"));
    }
}
