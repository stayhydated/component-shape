use super::*;

/// Typed MCP tool call payload passed to registered handlers.
///
/// The shared server normalizes protocol-level arguments into this object
/// before dispatch. Domain integrations can then decode fields without
/// repeatedly accepting arbitrary JSON values at every handler boundary.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct McpToolCall {
    arguments: McpToolArguments,
}

impl McpToolCall {
    /// Creates a tool call from normalized MCP arguments.
    pub fn new(arguments: McpToolArguments) -> Self {
        Self { arguments }
    }

    /// Creates a tool call with no arguments.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Converts protocol arguments into a normalized tool call.
    ///
    /// # Errors
    ///
    /// Returns [`McpToolError::ArgumentsMustBeObject`] when `arguments` is
    /// present but is not a JSON object.
    pub fn from_value(arguments: Option<Value>) -> Result<Self, McpToolError> {
        match arguments {
            None => Ok(Self::empty()),
            Some(Value::Object(arguments)) => Ok(Self::new(arguments)),
            Some(_) => Err(McpToolError::ArgumentsMustBeObject),
        }
    }

    /// Returns the normalized argument object.
    pub fn arguments(&self) -> &McpToolArguments {
        &self.arguments
    }

    /// Converts this call into an owning argument decoder.
    pub fn into_arguments(self) -> McpArguments {
        McpArguments::new(self.arguments)
    }
}

/// Owning decoder for MCP tool argument objects.
///
/// Generated form and table integrations consume arguments through this type
/// instead of open-coding JSON map removal. Manual tool integrations can use
/// the same helpers and call [`McpArguments::finish`] when every expected
/// argument has been consumed.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct McpArguments {
    arguments: McpToolArguments,
}

impl McpArguments {
    /// Creates an owning decoder from normalized MCP arguments.
    pub fn new(arguments: McpToolArguments) -> Self {
        Self { arguments }
    }

    /// Returns whether any arguments remain unconsumed.
    pub fn is_empty(&self) -> bool {
        self.arguments.is_empty()
    }

    /// Returns the remaining raw argument object.
    pub fn as_inner(&self) -> &McpToolArguments {
        &self.arguments
    }

    /// Returns the remaining raw argument object.
    pub fn into_inner(self) -> McpToolArguments {
        self.arguments
    }

    /// Removes and returns one raw argument by wire field name.
    pub fn take_raw(&mut self, field: &str) -> Option<Value> {
        self.arguments.remove(field)
    }

    /// Removes and returns one raw argument by canonical field name or alias.
    ///
    /// # Errors
    ///
    /// Returns [`McpToolError::DuplicateField`] when both the canonical field
    /// name and an alias are present, or multiple aliases are present.
    pub fn take_raw_one_of(
        &mut self,
        field: &str,
        aliases: &[&str],
    ) -> Result<Option<Value>, McpToolError> {
        let mut found = self.take_raw(field);
        for alias in aliases {
            if let Some(alias_value) = self.take_raw(alias) {
                if found.is_some() {
                    return Err(McpToolError::DuplicateField {
                        field: field.to_string(),
                    });
                }
                found = Some(alias_value);
            }
        }
        Ok(found)
    }

    /// Removes, requires, and decodes one typed argument by field name.
    ///
    /// # Errors
    ///
    /// Returns [`McpToolError`] when the field is missing or when `T` rejects the
    /// raw JSON value.
    pub fn take_required_tool_value<T>(
        &mut self,
        field: impl Into<String>,
    ) -> Result<T, McpToolError>
    where
        T: McpToolValue,
    {
        let field = field.into();
        let value = self
            .take_raw(&field)
            .ok_or_else(|| McpToolError::missing_field(field.clone()))?;
        T::from_tool_value(&field, value)
    }

    /// Removes, requires, and decodes one typed argument by field name or alias.
    ///
    /// # Errors
    ///
    /// Returns [`McpToolError`] when no accepted field name is present, duplicate
    /// field spellings are present, or `T` rejects the raw JSON value.
    pub fn take_required_tool_value_from<T>(
        &mut self,
        field: &'static str,
        aliases: &'static [&'static str],
    ) -> Result<T, McpToolError>
    where
        T: McpToolValue,
    {
        let value = self
            .take_raw_one_of(field, aliases)?
            .ok_or_else(|| McpToolError::missing_field(field))?;
        T::from_tool_value(field, value)
    }

    /// Removes and decodes an optional typed argument by field name.
    ///
    /// # Errors
    ///
    /// Returns [`McpToolError`] when `T` rejects the raw JSON value.
    pub fn take_present_tool_value<T>(
        &mut self,
        field: impl Into<String>,
    ) -> Result<Option<T>, McpToolError>
    where
        T: McpToolValue,
    {
        let field = field.into();
        self.take_raw(&field)
            .map(|value| T::from_tool_value(&field, value))
            .transpose()
    }

    /// Removes and decodes an optional typed argument by field name or alias.
    ///
    /// # Errors
    ///
    /// Returns [`McpToolError`] when duplicate field spellings are present or
    /// `T` rejects the raw JSON value.
    pub fn take_present_tool_value_from<T>(
        &mut self,
        field: &'static str,
        aliases: &'static [&'static str],
    ) -> Result<Option<T>, McpToolError>
    where
        T: McpToolValue,
    {
        self.take_raw_one_of(field, aliases)?
            .map(|value| T::from_tool_value(field, value))
            .transpose()
    }

    /// Verifies that no unrecognized arguments remain.
    ///
    /// # Errors
    ///
    /// Returns [`McpToolError::UnknownField`] when any raw arguments remain.
    pub fn finish(self) -> Result<(), McpToolError> {
        reject_unknown_arguments(self.arguments)
    }
}

impl From<McpToolCall> for McpArguments {
    fn from(call: McpToolCall) -> Self {
        call.into_arguments()
    }
}

impl From<McpToolArguments> for McpArguments {
    fn from(arguments: McpToolArguments) -> Self {
        Self::new(arguments)
    }
}
