use super::*;

#[derive(Clone, Debug)]
pub struct ShapeOptions {
    pub shape: Path,
    constructor: ComponentShapeConstructor,
    span: Span,
}

/// Optional configured construction expression attached to a component shape.
#[derive(Clone, Debug)]
pub enum ComponentShapeConstructor {
    /// Construct the shape with the consumer's normal/default constructor.
    Default,
    /// Construct the shape with a user-supplied expression, such as
    /// `Select::<_>::searchable(true)`.
    Expr(Expr),
}

impl ComponentShapeConstructor {
    pub fn expr(&self) -> Option<&Expr> {
        match self {
            Self::Default => None,
            Self::Expr(expr) => Some(expr),
        }
    }

    fn resolved(&self, field_type: &Type) -> Self {
        match self {
            Self::Default => Self::Default,
            Self::Expr(expr) => Self::Expr(substitute_infer_in_expr(expr, field_type)),
        }
    }
}

impl ShapeOptions {
    pub fn from_shape(shape: Path) -> Self {
        let span = shape.span();
        Self::from_shape_with_span(shape, span)
    }

    pub fn from_shape_with_span(shape: Path, span: Span) -> Self {
        let shape = normalize_shape_path(shape);
        Self {
            shape,
            constructor: ComponentShapeConstructor::Default,
            span,
        }
    }

    /// Build shape options from either a plain shape path expression or a
    /// configured constructor expression.
    pub fn from_constructor_expr(expr: Expr, expected: &'static str) -> syn::Result<Self> {
        let span = expr.span();
        Self::from_constructor_expr_with_span(expr, span, expected)
    }

    /// Build shape options from either a plain shape path expression or a
    /// configured constructor expression, using `span` for later diagnostics.
    pub fn from_constructor_expr_with_span(
        expr: Expr,
        span: Span,
        expected: &'static str,
    ) -> syn::Result<Self> {
        let parts = component_shape_expression_parts(&expr, expected)?;
        let constructor = if parts.configured {
            ComponentShapeConstructor::Expr(expr)
        } else {
            ComponentShapeConstructor::Default
        };

        Ok(Self {
            shape: parts.shape,
            constructor,
            span,
        })
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn resolved_shape(&self, field_type: &Type) -> Path {
        substitute_infer_in_path(&self.shape, field_type)
    }

    pub fn resolve(&self, field_name: String, field_type: Type) -> ResolvedComponentShape {
        let shape = self.resolved_shape(&field_type);
        let constructor = self.constructor.resolved(&field_type);
        let component_suffix = component_suffix_for_shape(&shape, &field_name);

        ResolvedComponentShape {
            shape,
            constructor,
            field_name,
            field_type,
            component_suffix,
            span: self.span,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedComponentShape {
    pub shape: Path,
    constructor: ComponentShapeConstructor,
    pub field_name: String,
    pub field_type: Type,
    component_suffix: String,
    span: Span,
}

impl ResolvedComponentShape {
    pub fn shape(&self) -> &Path {
        &self.shape
    }

    pub fn constructor(&self) -> &ComponentShapeConstructor {
        &self.constructor
    }

    pub fn constructor_expr(&self) -> Option<&Expr> {
        self.constructor.expr()
    }

    pub fn field_name(&self) -> &str {
        &self.field_name
    }

    pub fn field_type(&self) -> &Type {
        &self.field_type
    }

    pub fn component_suffix(&self) -> &str {
        &self.component_suffix
    }

    pub fn span(&self) -> Span {
        self.span
    }
}
