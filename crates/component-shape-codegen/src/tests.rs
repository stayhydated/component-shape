use super::*;
use quote::ToTokens as _;
use syn::parse::Parser as _;
use syn::parse_quote;

fn compact_type(ty: &Type) -> String {
    ty.to_token_stream()
        .to_string()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}

fn compact_path(path: &Path) -> String {
    path.to_token_stream()
        .to_string()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}

fn compact_tokens(tokens: TokenStream) -> String {
    tokens
        .to_string()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}

#[test]
fn shape_options_resolve_identifier_shape_and_suffix() {
    let options = ShapeOptions::from_shape(syn::parse_quote!(crate::EmailInputShape<_>));
    let field_type: Type = syn::parse_quote!(String);

    let resolved = options.resolve("email".to_string(), field_type);

    assert_eq!(
        compact_path(resolved.shape()),
        "crate::EmailInputShape<String>"
    );
    assert_eq!(resolved.component_suffix(), "input");
}

#[test]
fn shape_options_resolve_plain_constructor_expr_as_default_shape() {
    let expr: Expr = parse_quote!(crate::Input::<_>);
    let options = ShapeOptions::from_constructor_expr(expr, "expected component shape")
        .expect("plain shape expression should parse");
    let field_type: Type = syn::parse_quote!(crate::types::AccountCode);

    let resolved = options.resolve("account".to_string(), field_type);

    assert_eq!(
        compact_path(resolved.shape()),
        "crate::Input<crate::types::AccountCode>"
    );
    assert!(resolved.constructor_expr().is_none());
}

#[test]
fn shape_options_resolve_dot_chain_constructor_expr() {
    let expr: Expr = parse_quote!(
        crate::select::Select::<_>
            .searchable(true)
            .placeholder("Country")
    );
    let options = ShapeOptions::from_constructor_expr(expr, "expected component shape")
        .expect("dot-chain shape expression should parse");
    let field_type: Type = syn::parse_quote!(String);

    let resolved = options.resolve("country".to_string(), field_type);

    assert_eq!(
        compact_path(resolved.shape()),
        "crate::select::Select<String>"
    );
    assert_eq!(
        compact_tokens(
            resolved
                .constructor_expr()
                .expect("configured constructor should be retained")
                .to_token_stream()
        ),
        "crate::select::Select::<String>::searchable(true).placeholder(\"Country\")"
    );
}

#[test]
fn shape_options_resolve_from_constructor_expr() {
    let expr: Expr = parse_quote!(
        crate::select::Select::<_>.from(
            crate::select::SelectArgs::builder()
                .searchable(true)
                .placeholder("Country")
                .build()
        )
    );
    let options = ShapeOptions::from_constructor_expr(expr, "expected component shape")
        .expect("configured shape expression should parse");
    let field_type: Type = syn::parse_quote!(String);

    let resolved = options.resolve("country".to_string(), field_type);

    assert_eq!(
        compact_path(resolved.shape()),
        "crate::select::Select<String>"
    );
    assert_eq!(
        compact_tokens(
            resolved
                .constructor_expr()
                .expect("configured constructor should be retained")
                .to_token_stream()
        ),
        "crate::select::Select::<String>::from(crate::select::SelectArgs::builder().searchable(true).placeholder(\"Country\").build())"
    );
}

#[test]
fn shape_path_from_constructor_expr_rejects_associated_constructor_exprs() {
    let expr: Expr = parse_quote!(crate::select::Select::<_>::searchable(true));

    let error = shape_path_from_constructor_expr(&expr, "expected component shape")
        .expect_err("associated constructor should fail");

    assert_eq!(error.to_string(), "expected component shape");
}

#[test]
fn shape_path_from_constructor_expr_rejects_method_chains() {
    let expr: Expr = parse_quote!(crate::select::Select::<_>::args().searchable(true));

    let error = shape_path_from_constructor_expr(&expr, "expected component shape")
        .expect_err("method chain should fail");

    assert_eq!(error.to_string(), "expected component shape");
}

#[test]
fn shape_path_from_constructor_expr_rejects_unanchored_call() {
    let expr: Expr = parse_quote!(searchable(true));

    let error = shape_path_from_constructor_expr(&expr, "expected component shape")
        .expect_err("unanchored call should fail");

    assert_eq!(error.to_string(), "expected component shape");
}

#[test]
fn substitutes_infer_in_arrays_slices_pointers_and_bare_fns() {
    let ty: Type = syn::parse_quote! {
        fn([_; 2], &[_], *const _, *mut _) -> Option<_>
    };
    let replacement: Type = syn::parse_quote!(String);

    let substituted = substitute_infer_in_type(&ty, &replacement);

    assert_eq!(
        compact_type(&substituted),
        "fn([String;2],&[String],*constString,*mutString)->Option<String>"
    );
}

#[test]
fn substitutes_infer_in_trait_objects_and_generic_constraints() {
    let ty: Type = syn::parse_quote! {
        dyn crate::Shape<Assoc: Iterator<Item = _>> + FnOnce(_) -> _
    };
    let replacement: Type = syn::parse_quote!(String);

    let substituted = substitute_infer_in_type(&ty, &replacement);

    assert_eq!(
        compact_type(&substituted),
        "dyncrate::Shape<Assoc:Iterator<Item=String>>+FnOnce(String)->String"
    );
}

#[test]
fn rust_metadata_helpers_emit_unchecked_wrapper_construction() {
    let rust_type =
        rust_type_metadata_tokens(quote!(component_shape::RustType), &parse_quote!(String));
    let rust_path = rust_path_metadata_tokens(
        quote!(component_shape::RustPath),
        &parse_quote!(crate::Input),
    );
    let rust_expr = rust_expr_metadata_tokens(
        quote!(component_shape::RustExpr),
        &parse_quote!(Some(Default::default())),
    );
    let optional_rust_expr = optional_rust_expr_metadata_tokens(
        quote!(component_shape::RustExpr),
        Some(&parse_quote!(42)),
    );
    let no_rust_expr = optional_rust_expr_metadata_tokens(quote!(component_shape::RustExpr), None);

    assert_eq!(
        compact_tokens(rust_type),
        "component_shape::RustType::from_macro_tokens_unchecked(\"String\")"
    );
    assert_eq!(
        compact_tokens(rust_path),
        "component_shape::RustPath::from_macro_tokens_unchecked(\"crate::Input\")"
    );
    assert_eq!(
        compact_tokens(rust_expr),
        "component_shape::RustExpr::from_macro_tokens_unchecked(\"Some(Default::default())\")"
    );
    assert_eq!(
        compact_tokens(optional_rust_expr),
        "Some(component_shape::RustExpr::from_macro_tokens_unchecked(\"42\"))"
    );
    assert_eq!(compact_tokens(no_rust_expr), "None");
}

#[test]
fn shape_path_from_expr_accepts_path_expressions() {
    let expr: Expr = parse_quote!(crate::Input::<_>);

    let path = shape_path_from_expr(&expr, "expected shape path").expect("path expression parses");

    assert_eq!(compact_path(&path), "crate::Input<_>");
}

#[test]
fn shape_path_from_expr_rejects_non_path_expressions() {
    let expr: Expr = parse_quote!(make_shape());

    let error = shape_path_from_expr(&expr, "expected shape path")
        .expect_err("call expression should fail");

    assert_eq!(error.to_string(), "expected shape path");
}

#[test]
fn parse_single_shape_path_rejects_extra_tokens() {
    let parser = |input: syn::parse::ParseStream<'_>| {
        parse_single_shape_path(input, "expected exactly one shape path")
    };

    let error = parser
        .parse2(quote!(crate::Input, value(type = String)))
        .expect_err("extra tokens should fail");

    assert_eq!(error.to_string(), "expected exactly one shape path");
}

#[test]
fn shape_type_assertion_tokens_include_field_specific_bounds() {
    let tokens = shape_type_assertion_tokens(
        "gpui_form",
        "email-address",
        &quote!(EmailInputShape),
        &quote!(String),
        Span::call_site(),
        [
            quote!(component_shape_gpui::DeclaredGpuiComponentShape),
            quote!(component_shape_gpui::GpuiComponentShape),
        ],
        quote!(component_shape_gpui::GpuiComponentShapeFor),
    );

    let tokens = compact_tokens(tokens);
    assert!(tokens.contains("constfn__gpui_form_assert_email_address_declared_shape<Shape>()"));
    assert!(tokens.contains("Shape:component_shape_gpui::DeclaredGpuiComponentShape"));
    assert!(tokens.contains("Shape:component_shape_gpui::GpuiComponentShape"));
    assert!(
        tokens
            .contains("constfn__gpui_form_assert_email_address_shape_compatibility<Shape,Field>()")
    );
    assert!(tokens.contains("Shape:component_shape_gpui::GpuiComponentShapeFor<Field>"));
    assert!(tokens.contains(
        "__gpui_form_assert_email_address_shape_compatibility::<EmailInputShape,String>()"
    ));
}

#[test]
fn doc_description_trims_outer_blank_doc_lines() {
    let input: syn::ItemStruct = syn::parse_quote! {
        ///
        /// Search arguments.
        ///
        /// Includes pagination.
        ///
        struct Search;
    };

    assert_eq!(
        doc_description(&input.attrs),
        Some("Search arguments.\n\nIncludes pagination.".to_string())
    );
}

#[test]
fn mcp_tool_metadata_tokens_prefers_explicit_description_over_docs() {
    let input: syn::ItemStruct = syn::parse_quote! {
        /// Inferred description.
        struct Search;
    };
    let mcp_crate: Path = syn::parse_quote!(gpui_form::mcp);

    let tokens = mcp_tool_metadata_tokens(
        &mcp_crate,
        &input.attrs,
        McpToolMetadataParts {
            name: Some("search"),
            title: Some("Search"),
            description: Some("Explicit description."),
            read_only: Some(true),
            destructive: Some(false),
            idempotent: Some(true),
            open_world: Some(false),
        },
        Span::call_site(),
    )
    .expect("metadata should be valid");
    let compact = compact_tokens(tokens);

    assert!(compact.contains("gpui_form::mcp::McpToolMetadata::new()"));
    assert!(compact.contains(".with_name(\"search\")"));
    assert!(compact.contains(".with_title(\"Search\")"));
    assert!(compact.contains(".with_description(\"Explicitdescription.\")"));
    assert!(compact.contains(".with_read_only_hint(true)"));
    assert!(compact.contains(".with_destructive_hint(false)"));
    assert!(compact.contains(".with_idempotent_hint(true)"));
    assert!(compact.contains(".with_open_world_hint(false)"));
    assert!(!compact.contains("Inferreddescription."));
}

#[test]
fn mcp_tool_metadata_tokens_validates_metadata() {
    let mcp_crate: Path = syn::parse_quote!(gpui_form::mcp);
    let error = mcp_tool_metadata_tokens(
        &mcp_crate,
        &[],
        McpToolMetadataParts {
            name: Some("bad name"),
            title: None,
            description: None,
            read_only: None,
            destructive: None,
            idempotent: None,
            open_world: None,
        },
        Span::call_site(),
    )
    .expect_err("invalid name should fail");

    assert!(error.to_string().contains("tool name"));

    let error = mcp_tool_metadata_tokens(
        &mcp_crate,
        &[],
        McpToolMetadataParts {
            name: None,
            title: None,
            description: None,
            read_only: Some(true),
            destructive: Some(true),
            idempotent: None,
            open_world: None,
        },
        Span::call_site(),
    )
    .expect_err("conflicting annotation hints should fail");

    assert!(
        error
            .to_string()
            .contains("cannot be both read-only and destructive"),
        "unexpected error: {error}"
    );
}

#[test]
fn inferred_mcp_input_shape_handles_scalar_wrappers() {
    let string_ty: Type = parse_quote!(std::borrow::Cow<'static, str>);
    let boxed_date_ty: Type = parse_quote!(Box<chrono::NaiveDate>);
    let optional_decimal_ty: Type = parse_quote!(Option<std::sync::Arc<rust_decimal::Decimal>>);

    assert_eq!(
        inferred_mcp_input_shape_for_type(&string_ty),
        Some(component_shape::McpInputShape::Scalar(
            component_shape::McpPrimitiveKind::String
        ))
    );
    assert_eq!(
        inferred_mcp_input_shape_for_type(&boxed_date_ty),
        Some(component_shape::McpInputShape::Scalar(
            component_shape::McpPrimitiveKind::Date
        ))
    );
    assert_eq!(
        inferred_mcp_input_shape_for_type(&optional_decimal_ty),
        Some(component_shape::McpInputShape::Scalar(
            component_shape::McpPrimitiveKind::Decimal
        ))
    );
}

#[test]
fn inferred_mcp_input_shape_handles_collections_and_ranges() {
    let vec_ty: Type = parse_quote!(std::collections::VecDeque<String>);
    let wrapped_vec_ty: Type = parse_quote!(Vec<Box<String>>);
    let set_ty: Type = parse_quote!(std::collections::BTreeSet<chrono::NaiveDate>);
    let map_ty: Type = parse_quote!(std::collections::HashMap<String, gpui_form::mcp::McpAny>);
    let json_map_ty: Type = parse_quote!(serde_json::Map<String, gpui_form::mcp::McpAny>);
    let any_value_ty: Type = parse_quote!(gpui_form::mcp::McpAny);
    let array_ty: Type = parse_quote!([u64; 3]);
    let slice_ty: Type = parse_quote!(&[bool]);
    let tuple_range_ty: Type = parse_quote!((Option<chrono::NaiveDate>, Option<chrono::NaiveDate>));
    let mcp_range_ty: Type = parse_quote!(gpui_form::mcp::McpRange<rust_decimal::Decimal>);

    assert_eq!(
        inferred_mcp_input_shape_for_type(&vec_ty),
        Some(component_shape::McpInputShape::List(
            component_shape::McpPrimitiveKind::String
        ))
    );
    assert_eq!(
        inferred_mcp_input_shape_for_type(&wrapped_vec_ty),
        Some(component_shape::McpInputShape::List(
            component_shape::McpPrimitiveKind::String
        ))
    );
    assert_eq!(
        inferred_mcp_input_shape_for_type(&set_ty),
        Some(component_shape::McpInputShape::Set(
            component_shape::McpPrimitiveKind::Date
        ))
    );
    assert_eq!(
        inferred_mcp_input_shape_for_type(&map_ty),
        Some(component_shape::McpInputShape::Object)
    );
    assert_eq!(
        inferred_mcp_input_shape_for_type(&json_map_ty),
        Some(component_shape::McpInputShape::Object)
    );
    assert_eq!(
        inferred_mcp_input_shape_for_type(&any_value_ty),
        Some(component_shape::McpInputShape::Scalar(
            component_shape::McpPrimitiveKind::Any
        ))
    );
    assert_eq!(
        inferred_mcp_input_shape_for_type(&array_ty),
        Some(component_shape::McpInputShape::List(
            component_shape::McpPrimitiveKind::Integer
        ))
    );
    assert_eq!(
        inferred_mcp_input_shape_for_type(&slice_ty),
        Some(component_shape::McpInputShape::List(
            component_shape::McpPrimitiveKind::Boolean
        ))
    );
    assert_eq!(
        inferred_mcp_input_shape_for_type(&tuple_range_ty),
        Some(component_shape::McpInputShape::Range(
            component_shape::McpRangeBoundKind::Date
        ))
    );
    assert_eq!(
        inferred_mcp_input_shape_for_type(&mcp_range_ty),
        Some(component_shape::McpInputShape::Range(
            component_shape::McpRangeBoundKind::Decimal
        ))
    );
}

#[test]
fn common_inferred_mcp_input_shape_requires_agreement() {
    let string_ty: Type = parse_quote!(String);
    let string_ref_ty: Type = parse_quote!(&str);
    let number_ty: Type = parse_quote!(u64);

    assert_eq!(
        common_inferred_mcp_input_shape_for_types([&string_ty, &string_ref_ty]),
        Some(component_shape::McpInputShape::Scalar(
            component_shape::McpPrimitiveKind::String
        ))
    );
    assert_eq!(
        common_inferred_mcp_input_shape_for_types([&string_ty, &number_ty]),
        None
    );
}

#[test]
fn mcp_input_expr_tokens_support_constructor_shorthand() {
    let component_shape_crate: Path = parse_quote!(component_shape_gpui);
    let shorthand: Expr = parse_quote!(string_list);
    let explicit: Expr = parse_quote!(component_shape_gpui::McpInput::object());
    let invalid: Expr = parse_quote!(strings);

    assert_eq!(
        compact_tokens(mcp_input_expr_tokens(&component_shape_crate, &shorthand)),
        "component_shape_gpui::McpInput::string_list()"
    );
    assert_eq!(
        compact_tokens(mcp_input_expr_tokens(&component_shape_crate, &explicit)),
        "component_shape_gpui::McpInput::object()"
    );
    assert!(
        validate_mcp_input_expr(&invalid)
            .expect_err("unknown shorthand should fail")
            .to_string()
            .contains("unknown `mcp_input` shorthand `strings`")
    );
}
