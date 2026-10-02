use super::*;
use crate::McpToolValue as _;

macro_rules! serde_rename_contract {
    ($name:ident, $rule:literal) => {
        #[test]
        fn $name() {
            #[derive(
                Debug, PartialEq, serde::Serialize, serde::Deserialize, crate::McpJsonSchema,
            )]
            #[serde(rename_all = $rule)]
            struct Record {
                first_name: String,
                http_server2: String,
                _leading: String,
                trailing_: String,
                r#type: String,
            }

            #[derive(Debug, PartialEq, serde::Deserialize, crate::McpToolInput)]
            #[serde(rename_all = $rule)]
            struct Input {
                first_name: String,
                http_server2: String,
                _leading: String,
                trailing_: String,
                r#type: String,
            }

            let record = Record {
                first_name: "one".into(),
                http_server2: "two".into(),
                _leading: "three".into(),
                trailing_: "four".into(),
                r#type: "five".into(),
            };
            let wire = serde_json::to_value(&record).expect("serialize field names");
            for schema in [Record::json_schema(), Input::input_schema()] {
                assert_eq!(
                    schema["properties"]
                        .as_object()
                        .unwrap()
                        .keys()
                        .collect::<Vec<_>>(),
                    wire.as_object().unwrap().keys().collect::<Vec<_>>(),
                    "schema fields must follow Serde for {}",
                    $rule,
                );
            }
            assert_eq!(
                Record::from_tool_value("record", wire.clone()).unwrap(),
                record
            );
            let expected: Input = serde_json::from_value(wire.clone()).unwrap();
            let call = crate::McpToolCall::from_value(Some(wire)).unwrap();
            assert_eq!(Input::from_tool_call(call).unwrap(), expected);

            #[derive(
                Debug, PartialEq, serde::Serialize, serde::Deserialize, crate::McpJsonSchema,
            )]
            #[allow(non_camel_case_types)]
            #[serde(rename_all = $rule)]
            enum Variant {
                HTTPServer2,
                V1Beta,
                XML_HTTP2,
                r#type,
            }

            let variants = [
                Variant::HTTPServer2,
                Variant::V1Beta,
                Variant::XML_HTTP2,
                Variant::r#type,
            ];
            let expected_names = variants
                .iter()
                .map(|variant| serde_json::to_value(variant).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(Variant::json_schema()["enum"], json!(expected_names));
            for (variant, wire) in variants.into_iter().zip(expected_names) {
                assert_eq!(Variant::from_tool_value("variant", wire).unwrap(), variant);
            }
        }
    };
}

serde_rename_contract!(serde_rename_lowercase, "lowercase");
serde_rename_contract!(serde_rename_uppercase, "UPPERCASE");
serde_rename_contract!(serde_rename_pascalcase, "PascalCase");
serde_rename_contract!(serde_rename_camelcase, "camelCase");
serde_rename_contract!(serde_rename_snakecase, "snake_case");
serde_rename_contract!(serde_rename_screaming_snakecase, "SCREAMING_SNAKE_CASE");
serde_rename_contract!(serde_rename_kebabcase, "kebab-case");
serde_rename_contract!(serde_rename_screaming_kebabcase, "SCREAMING-KEBAB-CASE");

#[test]
fn serde_rename_deserialize_overrides_and_mcp_names_are_separate() {
    #[derive(Debug, PartialEq, serde::Deserialize, crate::McpJsonSchema)]
    #[serde(rename_all(serialize = "UPPERCASE", deserialize = "lowercase"))]
    #[mcp(rename_all = "camelCase")]
    struct Record {
        first_name: String,
        #[serde(
            rename(serialize = "OUTPUT", deserialize = "input_name"),
            alias = "old_name"
        )]
        #[mcp(rename = "mcp_name")]
        custom_name: String,
    }

    let schema = Record::json_schema();
    assert_eq!(
        schema["properties"]["firstName"]["x-mcpDecodeName"],
        "first_name"
    );
    assert_eq!(
        schema["properties"]["mcp_name"]["x-mcpDecodeName"],
        "input_name"
    );
    let wire = json!({"firstName": "one", "old_name": "two"});
    assert_eq!(
        Record::from_tool_value("record", wire).unwrap(),
        Record {
            first_name: "one".into(),
            custom_name: "two".into()
        }
    );

    #[derive(Debug, PartialEq, serde::Deserialize, crate::McpJsonSchema)]
    #[serde(rename_all(deserialize = "snake_case", serialize = "UPPERCASE"))]
    #[mcp(rename_all = "snake_case")]
    enum Variant {
        HTTPServer2,
        #[serde(
            rename(deserialize = "serde_name", serialize = "OUTPUT"),
            alias = "old_name"
        )]
        #[mcp(rename = "mcp_name")]
        CustomName,
    }

    let schema = Variant::json_schema();
    assert_eq!(
        schema["enum"],
        json!(["http_server_2", "mcp_name", "old_name"])
    );
    assert_eq!(
        schema["x-mcpEnumDecodeAliases"]["http_server_2"],
        "h_t_t_p_server2"
    );
    assert_eq!(
        Variant::from_tool_value("variant", json!("http_server_2")).unwrap(),
        Variant::HTTPServer2
    );
    assert_eq!(
        Variant::from_tool_value("variant", json!("mcp_name")).unwrap(),
        Variant::CustomName
    );
    assert_eq!(
        Variant::from_tool_value("variant", json!("old_name")).unwrap(),
        Variant::CustomName
    );
}
