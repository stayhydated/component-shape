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
    #[cfg(feature = "mcp-jiff")]
    {
        use component_shape_mcp::serde_json::{self, Value, json};
        use jiff::{
            Timestamp,
            civil::{Date, DateTime},
        };
        assert_eq!(Date::json_schema()["format"], "date");
        assert_eq!(Timestamp::json_schema()["format"], "date-time");
        assert_eq!(
            DateTime::json_schema().as_value(),
            &json!({ "type": "string" })
        );

        let date: Date = serde_json::from_value(json!("2024-02-29")).unwrap();
        let local: DateTime = serde_json::from_value(json!("2024-02-29T12:34:56")).unwrap();
        let stamp: Timestamp = serde_json::from_value(json!("2024-02-29T12:34:56+05:30")).unwrap();

        let date_wire: Value = serde_json::to_value(date).unwrap();
        let local_wire = serde_json::to_value(local).unwrap();
        let stamp_wire = serde_json::to_value(stamp).unwrap();
        assert_eq!(date_wire, "2024-02-29");
        assert_eq!(local_wire, "2024-02-29T12:34:56");
        assert_eq!(stamp_wire, "2024-02-29T07:04:56Z");
        assert_eq!(serde_json::from_value::<Date>(date_wire).unwrap(), date);
        assert_eq!(
            serde_json::from_value::<DateTime>(local_wire).unwrap(),
            local
        );
        assert_eq!(
            serde_json::from_value::<Timestamp>(stamp_wire).unwrap(),
            stamp
        );
        assert!(serde_json::from_value::<Timestamp>(json!("2024-02-29T12:34:56")).is_err());
    }
    #[cfg(feature = "mcp-decimal")]
    {
        assert_eq!(
            rust_decimal::Decimal::json_schema()["anyOf"][0]["type"],
            "number"
        );
    }
}
