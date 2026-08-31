use serde_json::Value;

pub(super) fn schema(value: Value) -> crate::McpSchema {
    crate::McpSchema::new(value)
}
