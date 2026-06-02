/// Normalized value change derived from a component event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValueChange<T> {
    /// The component event did not change the value.
    Unchanged,
    /// Replace the value with the supplied value.
    Set(T),
    /// Clear an optional value.
    Clear,
}

impl<T> ValueChange<T> {
    pub const fn set(value: T) -> Self {
        Self::Set(value)
    }

    pub const fn clear() -> Self {
        Self::Clear
    }

    pub const fn unchanged() -> Self {
        Self::Unchanged
    }
}

#[cfg(test)]
mod tests {
    use super::ValueChange;

    #[test]
    fn value_change_helpers_construct_expected_variants() {
        assert_eq!(ValueChange::set("x"), ValueChange::Set("x"));
        assert_eq!(ValueChange::<String>::clear(), ValueChange::Clear);
        assert_eq!(ValueChange::<String>::unchanged(), ValueChange::Unchanged);
    }
}
