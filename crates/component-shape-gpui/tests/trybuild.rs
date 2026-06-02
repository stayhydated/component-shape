#[test]
fn component_shape_macro_compile_tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/basic_shape.rs");
    t.pass("tests/ui/builder_shape.rs");
    t.pass("tests/ui/generic_shape.rs");
    t.pass("tests/ui/render_component.rs");
    t.pass("tests/ui/value_binding_shape.rs");
    t.compile_fail("tests/ui/incompatible_value.rs");
    t.compile_fail("tests/ui/missing_state.rs");
    t.compile_fail("tests/ui/invalid_suffix.rs");
    t.compile_fail("tests/ui/missing_value.rs");
    t.compile_fail("tests/ui/value_binding_without_impl.rs");
}
