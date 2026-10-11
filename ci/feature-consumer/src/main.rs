use component_shape_mcp::McpJsonSchema as _;

fn main() {
    assert_eq!(u32::json_schema()["type"], "integer");
    #[cfg(feature = "mcp-default")]
    {
        #[derive(component_shape_mcp::McpJsonSchema)]
        struct Input {
            #[allow(dead_code)]
            value: u32,
        }
        assert_eq!(
            Input::json_schema()["properties"]["value"]["type"],
            "integer"
        );
    }
    #[cfg(feature = "mcp-chrono")]
    {
        assert_eq!(chrono::NaiveDate::json_schema()["format"], "date");
        assert_eq!(
            chrono::DateTime::<chrono::Utc>::json_schema()["format"],
            "date-time"
        );
    }
    #[cfg(feature = "mcp-decimal")]
    {
        assert_eq!(
            rust_decimal::Decimal::json_schema()["anyOf"][0]["type"],
            "number"
        );
    }
}
