use super::*;

#[derive(Default)]
pub(crate) struct SchemaOptions {
    pub(crate) crate_path: Option<Path>,
    pub(crate) description: Option<String>,
    pub(crate) mcp_rename_all: Option<RenameRule>,
    pub(crate) serde_rename_all: Option<RenameRule>,
    pub(crate) defaulted: bool,
    pub(crate) transparent: bool,
}

impl SchemaOptions {
    pub(crate) fn parse(input: &DeriveInput) -> syn::Result<Self> {
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
pub(crate) enum RenameRule {
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

    pub(crate) fn apply(self, ident: &str) -> String {
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
pub(crate) struct FieldOptions {
    pub(crate) mcp_rename: Option<String>,
    pub(crate) serde_rename: Option<String>,
    pub(crate) aliases: Vec<String>,
    pub(crate) description: Option<String>,
    pub(crate) required: Option<bool>,
    pub(crate) default_value: Option<FieldDefault>,
    flatten: Option<proc_macro2::Span>,
    pub(crate) skip: bool,
}

#[derive(Clone)]
pub(crate) enum FieldDefault {
    Default,
    Expr(Expr),
    CallPath(Path),
}

impl FieldDefault {
    pub(crate) fn tokens(&self) -> proc_macro2::TokenStream {
        match self {
            Self::Default => quote! { ::core::default::Default::default() },
            Self::Expr(expr) => quote! { #expr },
            Self::CallPath(path) => quote! { #path() },
        }
    }
}

impl FieldOptions {
    pub(crate) fn parse(field: &syn::Field) -> syn::Result<Self> {
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

    pub(crate) fn defaulted(&self) -> bool {
        self.default_value.is_some()
    }
}

#[derive(Default)]
pub(crate) struct VariantOptions {
    pub(crate) mcp_rename: Option<String>,
    pub(crate) serde_rename: Option<String>,
    pub(crate) aliases: Vec<String>,
    pub(crate) skip: bool,
}

impl VariantOptions {
    pub(crate) fn parse(variant: &syn::Variant) -> syn::Result<Self> {
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

pub(crate) fn doc_description(attrs: &[Attribute]) -> Option<String> {
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

pub(crate) fn is_option_type(ty: &Type) -> bool {
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

pub(crate) fn claim_wire_name(
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
