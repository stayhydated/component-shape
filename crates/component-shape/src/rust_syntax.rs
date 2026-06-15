use strum::{Display, IntoStaticStr};

/// Rust type syntax stored as static metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RustType(&'static str);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RustPath(&'static str);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RustExpr(&'static str);

#[derive(Clone, Copy, Debug, Display, Eq, IntoStaticStr, PartialEq)]
#[strum(serialize_all = "lowercase", const_into_str)]
pub enum RustSyntaxKind {
    Type,
    Path,
    #[strum(to_string = "expression")]
    Expr,
}

impl RustSyntaxKind {
    pub const fn label(self) -> &'static str {
        self.into_str()
    }
}

#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
#[error("invalid Rust {kind} metadata `{value}`: {error}")]
pub struct RustSyntaxError {
    kind: RustSyntaxKind,
    value: String,
    error: String,
}

impl RustSyntaxError {
    pub const fn kind(&self) -> RustSyntaxKind {
        self.kind
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn source_error(&self) -> &str {
        &self.error
    }
}

fn parse_rust_syntax<T: syn::parse::Parse>(
    kind: RustSyntaxKind,
    value: &'static str,
) -> Result<T, RustSyntaxError> {
    syn::parse_str(value).map_err(|err| RustSyntaxError {
        kind,
        value: value.to_string(),
        error: err.to_string(),
    })
}

impl RustType {
    pub fn new(value: &'static str) -> Result<Self, RustSyntaxError> {
        parse_rust_syntax::<syn::Type>(RustSyntaxKind::Type, value)?;
        Ok(Self(value))
    }

    pub fn new_opt(value: Option<&'static str>) -> Result<Option<Self>, RustSyntaxError> {
        value.map(Self::new).transpose()
    }

    pub const fn from_macro_tokens_unchecked(value: &'static str) -> Self {
        Self(value)
    }

    pub const fn from_macro_tokens_opt_unchecked(value: Option<&'static str>) -> Option<Self> {
        match value {
            Some(value) => Some(Self::from_macro_tokens_unchecked(value)),
            None => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }

    pub fn parse(self) -> Result<syn::Type, RustSyntaxError> {
        parse_rust_syntax(RustSyntaxKind::Type, self.0)
    }
}

impl RustPath {
    pub fn new(value: &'static str) -> Result<Self, RustSyntaxError> {
        parse_rust_syntax::<syn::Path>(RustSyntaxKind::Path, value)?;
        Ok(Self(value))
    }

    pub const fn from_macro_tokens_unchecked(value: &'static str) -> Self {
        Self(value)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }

    pub fn parse(self) -> Result<syn::Path, RustSyntaxError> {
        parse_rust_syntax(RustSyntaxKind::Path, self.0)
    }
}

impl RustExpr {
    pub fn new(value: &'static str) -> Result<Self, RustSyntaxError> {
        parse_rust_syntax::<syn::Expr>(RustSyntaxKind::Expr, value)?;
        Ok(Self(value))
    }

    pub const fn from_macro_tokens_unchecked(value: &'static str) -> Self {
        Self(value)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }

    pub fn parse(self) -> Result<syn::Expr, RustSyntaxError> {
        parse_rust_syntax(RustSyntaxKind::Expr, self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{RustExpr, RustPath, RustSyntaxKind, RustType};

    #[test]
    fn rust_type_validates_and_parses_type_syntax() {
        let ty =
            RustType::new("std::collections::HashMap<String, usize>").expect("type should parse");

        assert_eq!(ty.as_str(), "std::collections::HashMap<String, usize>");
        assert!(matches!(ty.parse(), Ok(syn::Type::Path(_))));
    }

    #[test]
    fn rust_type_new_opt_validates_some_values() {
        assert_eq!(
            RustType::new_opt(Some("Option<String>"))
                .expect("type should parse")
                .map(RustType::as_str),
            Some("Option<String>")
        );
        assert_eq!(RustType::new_opt(None).expect("none should pass"), None);
    }

    #[test]
    fn rust_path_validates_and_parses_path_syntax() {
        let path = RustPath::new("crate::widgets::TextInput<String>").expect("path should parse");

        assert_eq!(path.as_str(), "crate::widgets::TextInput<String>");
        assert_eq!(
            path.parse()
                .expect("path should parse")
                .segments
                .last()
                .expect("path should have final segment")
                .ident,
            "TextInput"
        );
    }

    #[test]
    fn rust_expr_validates_and_parses_expression_syntax() {
        let expr = RustExpr::new("Some(Default::default())").expect("expr should parse");

        assert_eq!(expr.as_str(), "Some(Default::default())");
        assert!(matches!(expr.parse(), Ok(syn::Expr::Call(_))));
    }

    #[test]
    fn rust_syntax_errors_record_kind_value_and_source_error() {
        let error = RustType::new("Vec<").expect_err("invalid type should fail");

        assert_eq!(error.kind(), RustSyntaxKind::Type);
        assert_eq!(error.value(), "Vec<");
        assert!(!error.source_error().is_empty());
        assert!(
            error
                .to_string()
                .starts_with("invalid Rust type metadata `Vec<`: ")
        );
    }

    #[test]
    fn unchecked_macro_constructors_do_not_validate() {
        assert_eq!(
            RustType::from_macro_tokens_unchecked("Vec<").as_str(),
            "Vec<"
        );
        assert_eq!(
            RustType::from_macro_tokens_opt_unchecked(Some("Vec<")).map(RustType::as_str),
            Some("Vec<")
        );
    }
}
