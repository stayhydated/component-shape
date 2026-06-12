use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use quote::quote;
use syn::{
    Data, DeriveInput, Expr, Fields, GenericArgument, Ident, LitBool, LitStr, Path, PathArguments,
    Type, parse_macro_input, parse_quote, spanned::Spanned,
};

#[proc_macro_derive(McpJsonSchema, attributes(mcp, serde))]
pub fn derive_mcp_json_schema(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_mcp_json_schema(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_mcp_json_schema(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let schema_options = SchemaOptions::parse(&input)?;
    let ident = input.ident;
    let mut generics = input.generics;
    let mcp_crate = schema_options
        .crate_path
        .clone()
        .unwrap_or_else(|| resolve_crate_path("component-shape-mcp", "::component_shape_mcp"));
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
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote!(#ty: #mcp_crate::McpJsonSchema));
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
        return Ok(quote! {
            impl #impl_generics #mcp_crate::McpJsonSchema for #ident #ty_generics
                #where_clause
            {
                fn json_schema() -> #mcp_crate::serde_json::Value {
                    <#ty as #mcp_crate::McpJsonSchema>::json_schema()
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
            generics
                .make_where_clause()
                .predicates
                .push(parse_quote!(#ty: #mcp_crate::McpJsonSchema));
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
            return Ok(quote! {
                impl #impl_generics #mcp_crate::McpJsonSchema for #ident #ty_generics
                    #where_clause
                {
                    fn json_schema() -> #mcp_crate::serde_json::Value {
                        <#ty as #mcp_crate::McpJsonSchema>::json_schema()
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

    for field in fields.named {
        let Some(field_ident) = field.ident.clone() else {
            continue;
        };
        let options = FieldOptions::parse(&field)?;
        if options.skip {
            continue;
        }

        let field_name = options.rename.unwrap_or_else(|| {
            schema_options
                .rename_all
                .map(|rule| rule.apply(&field_ident.to_string()))
                .unwrap_or_else(|| field_ident.to_string())
        });
        let field_name = LitStr::new(&field_name, field_ident.span());
        let description = options
            .description
            .map(|description| LitStr::new(&description, field_ident.span()));
        let required = options.required.unwrap_or_else(|| {
            !is_option_type(&field.ty) && !options.defaulted && !schema_options.defaulted
        });
        let ty = field.ty;
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote!(#ty: #mcp_crate::McpJsonSchema));
        let description_tokens = description
            .map(|description| {
                quote! {
                    if let Some(__component_shape_mcp_field_schema_object) =
                        __component_shape_mcp_field_schema.as_object_mut()
                    {
                        __component_shape_mcp_field_schema_object.insert(
                            "description".to_string(),
                            #mcp_crate::serde_json::Value::String(#description.to_string()),
                        );
                    }
                }
            })
            .unwrap_or_default();

        field_tokens.push(quote! {
            {
                let mut __component_shape_mcp_field_schema =
                    <#ty as #mcp_crate::McpJsonSchema>::json_schema();
                #description_tokens
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

    Ok(quote! {
        impl #impl_generics #mcp_crate::McpJsonSchema for #ident #ty_generics
            #where_clause
        {
            fn json_schema() -> #mcp_crate::serde_json::Value {
                let mut __component_shape_mcp_properties =
                    #mcp_crate::serde_json::Map::new();
                let mut __component_shape_mcp_required = Vec::new();

                #(#field_tokens)*

                #mcp_crate::object_schema(
                    __component_shape_mcp_properties,
                    __component_shape_mcp_required,
                )
            }
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

        let primary_name = options.rename.unwrap_or_else(|| {
            schema_options
                .rename_all
                .map(|rule| rule.apply(&variant.ident.to_string()))
                .unwrap_or_else(|| variant.ident.to_string())
        });
        value_tokens.push(enum_value_push_tokens(
            &mcp_crate,
            &primary_name,
            variant.ident.span(),
        ));
        for alias in options.aliases {
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

    Ok(quote! {
        impl #impl_generics #mcp_crate::McpJsonSchema for #ident #ty_generics
            #where_clause
        {
            fn json_schema() -> #mcp_crate::serde_json::Value {
                let mut __component_shape_mcp_enum_values = Vec::new();
                #(#value_tokens)*
                #mcp_crate::serde_json::json!({
                    "type": "string",
                    "enum": __component_shape_mcp_enum_values
                })
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

#[derive(Default)]
struct SchemaOptions {
    crate_path: Option<Path>,
    rename_all: Option<RenameRule>,
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
                if meta.path.is_ident("crate") || meta.path.is_ident("crate_path") {
                    set_option(
                        &mut options.crate_path,
                        parse_path_value(&meta)?,
                        "crate",
                        meta.path.span(),
                    )
                } else if meta.path.is_ident("rename_all") {
                    set_option(
                        &mut options.rename_all,
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
        Ok(options)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RenameRule {
    LowerCase,
    UpperCase,
    PascalCase,
    CamelCase,
    SnakeCase,
    ScreamingSnakeCase,
    KebabCase,
    ScreamingKebabCase,
}

impl RenameRule {
    fn parse_lit(value: &LitStr) -> syn::Result<Self> {
        match value.value().as_str() {
            "lowercase" => Ok(Self::LowerCase),
            "UPPERCASE" => Ok(Self::UpperCase),
            "PascalCase" => Ok(Self::PascalCase),
            "camelCase" => Ok(Self::CamelCase),
            "snake_case" => Ok(Self::SnakeCase),
            "SCREAMING_SNAKE_CASE" => Ok(Self::ScreamingSnakeCase),
            "kebab-case" => Ok(Self::KebabCase),
            "SCREAMING-KEBAB-CASE" => Ok(Self::ScreamingKebabCase),
            _ => Err(syn::Error::new(
                value.span(),
                format!("unsupported serde rename_all rule `{}`", value.value()),
            )),
        }
    }

    fn apply(self, ident: &str) -> String {
        let words = split_words(ident);
        match self {
            Self::LowerCase => words.concat().to_ascii_lowercase(),
            Self::UpperCase => words.concat().to_ascii_uppercase(),
            Self::PascalCase => words.iter().map(|word| capitalize(word)).collect(),
            Self::CamelCase => {
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
            Self::SnakeCase => words.join("_").to_ascii_lowercase(),
            Self::ScreamingSnakeCase => words.join("_").to_ascii_uppercase(),
            Self::KebabCase => words.join("-").to_ascii_lowercase(),
            Self::ScreamingKebabCase => words.join("-").to_ascii_uppercase(),
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
            && match (previous_kind, kind, next_kind) {
                (Some(CharKind::Lower), CharKind::Upper, _) => true,
                (Some(CharKind::Digit), CharKind::Upper | CharKind::Lower, _) => true,
                (Some(CharKind::Upper | CharKind::Lower), CharKind::Digit, _) => true,
                (Some(CharKind::Upper), CharKind::Upper, Some(CharKind::Lower)) => true,
                _ => false,
            };

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
    rename: Option<String>,
    description: Option<String>,
    required: Option<bool>,
    defaulted: bool,
    flatten: Option<proc_macro2::Span>,
    skip: bool,
}

impl FieldOptions {
    fn parse(field: &syn::Field) -> syn::Result<Self> {
        let mut options = Self::default();
        for attr in &field.attrs {
            if attr.path().is_ident("mcp") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("rename") || meta.path.is_ident("name") {
                        set_option(
                            &mut options.rename,
                            meta.value()?.parse::<LitStr>()?.value(),
                            "rename",
                            meta.path.span(),
                        )
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
                        set_flag(
                            &mut options.defaulted,
                            parse_bool_flag_or_value(&meta)?,
                            "default",
                            meta.path.span(),
                        )
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
}

#[derive(Default)]
struct VariantOptions {
    rename: Option<String>,
    aliases: Vec<String>,
    skip: bool,
}

impl VariantOptions {
    fn parse(variant: &syn::Variant) -> syn::Result<Self> {
        let mut options = Self::default();
        for attr in &variant.attrs {
            if attr.path().is_ident("mcp") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("rename") || meta.path.is_ident("name") {
                        set_option(
                            &mut options.rename,
                            meta.value()?.parse::<LitStr>()?.value(),
                            "rename",
                            meta.path.span(),
                        )
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
            if let Some(rename) = parse_serde_rename(&meta)? {
                if options.rename.is_none() {
                    options.rename = Some(rename);
                }
            }
            Ok(())
        } else if meta.path.is_ident("skip") {
            options.skip = true;
            Ok(())
        } else if meta.path.is_ident("skip_deserializing") {
            options.skip = true;
            consume_optional_serde_meta_value(&meta)
        } else if meta.path.is_ident("default") {
            options.defaulted = true;
            consume_optional_serde_meta_value(&meta)
        } else if meta.path.is_ident("flatten") {
            options.flatten = Some(meta.path.span());
            Ok(())
        } else if meta.path.is_ident("skip_serializing")
            || meta.path.is_ident("with")
            || meta.path.is_ident("serialize_with")
            || meta.path.is_ident("deserialize_with")
            || meta.path.is_ident("alias")
            || meta.path.is_ident("rename_all")
            || meta.path.is_ident("bound")
        {
            consume_optional_serde_meta_value(&meta)
        } else {
            Ok(())
        }
    })
}

fn parse_variant_serde_attr(
    attr: &syn::Attribute,
    options: &mut VariantOptions,
) -> syn::Result<()> {
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("rename") {
            if let Some(rename) = parse_serde_rename(&meta)? {
                if options.rename.is_none() {
                    options.rename = Some(rename);
                }
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
            if let Some(rename_all) = parse_serde_rename_rule(&meta)? {
                if options.rename_all.is_none() {
                    options.rename_all = Some(rename_all);
                }
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
    let Type::Path(path) = ty else {
        return false;
    };
    if path.qself.is_some() {
        return false;
    }
    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "Option" && has_one_type_argument(segment))
}

fn has_one_type_argument(segment: &syn::PathSegment) -> bool {
    matches!(
        &segment.arguments,
        PathArguments::AngleBracketed(arguments)
            if arguments.args.len() == 1
                && matches!(arguments.args.first(), Some(GenericArgument::Type(_)))
    )
}

fn resolve_crate_path(package: &str, fallback: &str) -> Path {
    let path = match crate_name(package) {
        Ok(FoundCrate::Itself) => "crate".to_string(),
        Ok(FoundCrate::Name(name)) => format!("::{name}"),
        Err(_) => fallback.to_string(),
    };
    syn::parse_str(&path).unwrap_or_else(|_| {
        let fallback_ident = Ident::new(
            fallback.trim_start_matches("::"),
            proc_macro2::Span::call_site(),
        );
        syn::parse_quote!(::#fallback_ident)
    })
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
        assert!(expanded.contains("page"));
        assert!(!expanded.contains("internal"));
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
    fn derive_schema_accepts_crate_path_override() {
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
    fn derive_schema_supports_fieldless_enums() {
        let input: DeriveInput = syn::parse_quote! {
            #[serde(rename_all = "kebab-case")]
            enum IssueState {
                Open,
                #[serde(alias = "reviewing")]
                InReview,
                #[mcp(rename = "done")]
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
        assert!(!expanded.contains("Unknown"));
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
