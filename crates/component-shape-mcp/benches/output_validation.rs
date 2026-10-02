use component_shape_mcp::{McpSchema, McpToolRegistry, tool_definition, tool_structured_result};
use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use serde_json::{Map, Value, json};
use std::{hint::black_box, time::Duration};

fn output_fixture(fields: usize, string_bytes: usize) -> (McpSchema, Value) {
    let properties = (0..fields)
        .map(|index| (format!("field_{index}"), McpSchema::string()))
        .collect();
    let required = (0..fields).map(|index| format!("field_{index}"));
    let schema = component_shape_mcp::object_schema(properties, required);
    let output = (0..fields)
        .map(|index| (format!("field_{index}"), json!("x".repeat(string_bytes))))
        .collect::<Map<_, _>>();
    (schema, Value::Object(output))
}

fn registry(output_schema: Option<McpSchema>, output: Value) -> McpToolRegistry {
    let definition = tool_definition("output", None, None, McpSchema::object(), output_schema)
        .expect("fixture schema should be valid");
    let mut registry = McpToolRegistry::new();
    registry
        .add_tool(definition, move |_| tool_structured_result(output.clone()))
        .expect("fixture should register once");
    registry
}

fn cached_validation(c: &mut Criterion) {
    // The synchronous bridge creates a thread/runtime per call. Reuse one
    // runtime here so these workloads measure in-process async dispatch.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("benchmark runtime should build");
    let mut group = c.benchmark_group("cached_output_validation");
    group.sample_size(30);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(2));

    for fields in [1, 16, 64] {
        for string_bytes in [16, 256] {
            let (schema, output) = output_fixture(fields, string_bytes);
            let bytes = serde_json::to_vec(&output).unwrap().len() as u64;
            let no_schema = registry(None, output.clone());
            let cached_valid = registry(Some(schema.clone()), output.clone());
            let mut invalid = output;
            invalid[format!("field_{}", fields - 1)] = json!(42);
            let invalid_bytes = serde_json::to_vec(&invalid).unwrap().len() as u64;
            let cached_invalid = registry(Some(schema), invalid);

            for (mode, registry, expected_error, fixture_bytes) in [
                ("no_schema", no_schema, false, bytes),
                ("cached_valid", cached_valid, false, bytes),
                ("cached_invalid", cached_invalid, true, invalid_bytes),
            ] {
                // Smoke mode checks outcomes as well as executing each workload.
                let result = runtime.block_on(registry.call_tool_async("output", None));
                assert_eq!(result.is_error, Some(expected_error));
                group.throughput(Throughput::Bytes(fixture_bytes));
                group.bench_with_input(
                    BenchmarkId::new(mode, format!("{fields}_fields_{string_bytes}_bytes")),
                    &registry,
                    |b, registry| {
                        b.to_async(&runtime).iter(|| async {
                            black_box(registry.call_tool_async(black_box("output"), None).await)
                        });
                    },
                );
            }
        }
    }
    group.finish();
}

fn cold_schema(c: &mut Criterion) {
    let mut group = c.benchmark_group("cold_output_schema");
    group.sample_size(30);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(2));
    for fields in [1, 16, 64] {
        let (schema, _) = output_fixture(fields, 16);
        group.bench_with_input(
            BenchmarkId::new("definition", fields),
            &schema,
            |b, schema| {
                b.iter(|| {
                    black_box(
                        tool_definition(
                            "output",
                            None,
                            None,
                            McpSchema::object(),
                            Some(black_box(schema.clone())),
                        )
                        .unwrap(),
                    )
                });
            },
        );
        let definition = tool_definition("output", None, None, McpSchema::object(), Some(schema))
            .expect("fixture definition should validate");
        group.bench_with_input(
            BenchmarkId::new("registration", fields),
            &definition,
            |b, definition| {
                // Definition validation and registry setup are outside this
                // measurement; registration compiles the retained validator.
                b.iter_batched(
                    || (McpToolRegistry::new(), definition.clone()),
                    |(mut registry, definition)| {
                        registry
                            .add_tool(definition, |_| tool_structured_result(json!({})))
                            .expect("fixture should register once");
                        black_box(registry)
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

criterion_group!(benches, cached_validation, cold_schema);
criterion_main!(benches);
