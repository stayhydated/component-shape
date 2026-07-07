use strum::{Display, IntoStaticStr};

/// Rust type syntax stored as static metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RustType(&'static str);

/// Rust path syntax stored as static metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RustPath(&'static str);

/// Rust expression syntax stored as static metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RustExpr(&'static str);

/// Kind of Rust syntax stored as static metadata.
#[derive(Clone, Copy, Debug, Display, Eq, IntoStaticStr, PartialEq)]
#[strum(serialize_all = "lowercase", const_into_str)]
pub enum RustSyntaxKind {
    /// A Rust type.
    Type,
    /// A Rust path.
    Path,
    /// A Rust expression.
    #[strum(to_string = "expression")]
    Expr,
}

impl RustSyntaxKind {
    /// Returns the stable English label for this syntax kind.
    pub const fn label(self) -> &'static str {
        self.into_str()
    }
}

/// Error returned when Rust syntax metadata fails to parse.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
#[error("invalid Rust {kind} metadata `{value}`: {error}")]
pub struct RustSyntaxError {
    kind: RustSyntaxKind,
    value: String,
    error: String,
}

impl RustSyntaxError {
    /// Returns the syntax kind that failed to parse.
    pub const fn kind(&self) -> RustSyntaxKind {
        self.kind
    }

    /// Returns the original metadata string.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the parser error message from `syn`.
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
    /// Validates Rust type syntax stored as static metadata.
    ///
    /// # Errors
    ///
    /// Returns [`RustSyntaxError`] when `value` is not valid Rust type syntax.
    pub fn new(value: &'static str) -> Result<Self, RustSyntaxError> {
        parse_rust_syntax::<syn::Type>(RustSyntaxKind::Type, value)?;
        Ok(Self(value))
    }

    /// Validates optional Rust type syntax stored as static metadata.
    ///
    /// # Errors
    ///
    /// Returns [`RustSyntaxError`] when `value` is `Some` and the contained
    /// string is not valid Rust type syntax.
    pub fn new_opt(value: Option<&'static str>) -> Result<Option<Self>, RustSyntaxError> {
        value.map(Self::new).transpose()
    }

    /// Stores Rust type syntax emitted by a trusted macro expansion.
    pub const fn from_macro_tokens_unchecked(value: &'static str) -> Self {
        Self(value)
    }

    /// Stores optional Rust type syntax emitted by a trusted macro expansion.
    pub const fn from_macro_tokens_opt_unchecked(value: Option<&'static str>) -> Option<Self> {
        match value {
            Some(value) => Some(Self::from_macro_tokens_unchecked(value)),
            None => None,
        }
    }

    /// Returns the stored Rust type syntax.
    pub const fn as_str(self) -> &'static str {
        self.0
    }

    /// Parses the stored Rust type syntax.
    ///
    /// # Errors
    ///
    /// Returns [`RustSyntaxError`] when the stored string is not valid Rust type
    /// syntax.
    pub fn parse(self) -> Result<syn::Type, RustSyntaxError> {
        parse_rust_syntax(RustSyntaxKind::Type, self.0)
    }
}

impl RustPath {
    /// Validates Rust path syntax stored as static metadata.
    ///
    /// # Errors
    ///
    /// Returns [`RustSyntaxError`] when `value` is not valid Rust path syntax.
    pub fn new(value: &'static str) -> Result<Self, RustSyntaxError> {
        parse_rust_syntax::<syn::Path>(RustSyntaxKind::Path, value)?;
        Ok(Self(value))
    }

    /// Stores Rust path syntax emitted by a trusted macro expansion.
    pub const fn from_macro_tokens_unchecked(value: &'static str) -> Self {
        Self(value)
    }

    /// Returns the stored Rust path syntax.
    pub const fn as_str(self) -> &'static str {
        self.0
    }

    /// Parses the stored Rust path syntax.
    ///
    /// # Errors
    ///
    /// Returns [`RustSyntaxError`] when the stored string is not valid Rust path
    /// syntax.
    pub fn parse(self) -> Result<syn::Path, RustSyntaxError> {
        parse_rust_syntax(RustSyntaxKind::Path, self.0)
    }
}

impl RustExpr {
    /// Validates Rust expression syntax stored as static metadata.
    ///
    /// # Errors
    ///
    /// Returns [`RustSyntaxError`] when `value` is not valid Rust expression
    /// syntax.
    pub fn new(value: &'static str) -> Result<Self, RustSyntaxError> {
        parse_rust_syntax::<syn::Expr>(RustSyntaxKind::Expr, value)?;
        Ok(Self(value))
    }

    /// Stores Rust expression syntax emitted by a trusted macro expansion.
    pub const fn from_macro_tokens_unchecked(value: &'static str) -> Self {
        Self(value)
    }

    /// Returns the stored Rust expression syntax.
    pub const fn as_str(self) -> &'static str {
        self.0
    }

    /// Parses the stored Rust expression syntax.
    ///
    /// # Errors
    ///
    /// Returns [`RustSyntaxError`] when the stored string is not valid Rust
    /// expression syntax.
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
