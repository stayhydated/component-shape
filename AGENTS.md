# Working in component-shape

Start with `just --list` for repository commands. Use the owning crate's
public rustdoc and tests to verify behavior before changing its documentation.

## Where changes belong

| Surface | Ownership |
| --- | --- |
| `crates/component-shape` | Public framework-neutral metadata, capabilities, suffixes, Rust syntax wrappers, coarse `McpInput`, and `ValueChange`. |
| `crates/component-shape-gpui` | Public GPUI state, rendering, construction, value binding, and macro re-exports. |
| `crates/component-shape-mcp` | Public schemas, strict decoding, tool registries, results, validation metadata, servers, resources, prompts, and stdio helpers. |
| `crates/component-shape-codegen` | Shared generator contracts: paths, suffixes, `_` substitution, imports, spans, and emitted metadata. |
| `crates/component-shape-gpui-macros` | GPUI declaration macro implementation; users enter through `component-shape-gpui`. |
| `crates/component-shape-mcp-macros` | Schema and input derive implementation; users enter through the MCP crate's `derive` feature. |
| `book/src` and `skills` | User workflows and reusable guidance for framework-neutral, GPUI, and MCP consumers. |
| `web/src/lib.rs` | Pages project identity, destinations, and route manifest. |
| `xtask/src/commands` | Shared book, llms.txt, Pages, preview, and release command integration. |

Keep shared metadata independent of GPUI. GPUI crates depend on
`component-shape`; the framework-neutral crate must not depend on GPUI.
Protocol behavior belongs in the MCP crate; applications own authorization,
domain validation execution, and handler policy.

## What changes together

- When a public trait, macro syntax, generated implementation, or runtime
  behavior changes, update its rustdoc and the README, book chapter, and skill
  that describe it. Check downstream macro consumers when shared codegen
  changes paths, identifiers, `_` substitution, or metadata output.
- GPUI compile contracts live in `crates/component-shape-gpui/tests/ui` and
  are registered in `crates/component-shape-gpui/tests/trybuild.rs`. Keep
  pass/fail fixtures aligned with changed syntax, generated implementations,
  and trait requirements.
- Preserve diagnostic spans and generated naming contracts in macro edits.
  Regenerate `.stderr` files only for intentional diagnostic changes, then
  inspect their diff.
- MCP runtime contracts are exercised in
  `crates/component-shape-mcp/src/tests`; schema and input expansion tests live
  in `crates/component-shape-mcp-macros/src/tests.rs`. Pair schema and decoder
  changes with their focused tests and affected MCP guidance.
- Edit book and site sources, then regenerate their outputs through `xtask`.
  Keep project destinations aligned with `web/src/lib.rs` and its route test.
- Update this guide when ownership, synchronization, or validation routes
  change. Keep implementation rationale near the owning code and tests.

## Validation

Run the narrowest check that covers the change:

- Crate behavior: `cargo test -p <crate> --all-features --locked`.
- GPUI macro contracts:
  `cargo test -p component-shape-gpui --test trybuild --locked`.
  Use `TRYBUILD=overwrite` with that command only to regenerate intentional
  diagnostic expectations.
- Rustdoc: `cargo doc --workspace --all-features --no-deps --locked`.
- Markdown: `rumdl check` with the edited files or directories.
- Book and llms.txt: `cargo xtask build book` and
  `cargo xtask build llms-txt`.
- Skills: run the skill-creator `quick_validate.py` on each edited skill.
- Pages: `cargo test -p web --lib --locked`, `just web-build`, and the
  stayhydated Pages consumer audit against `web/dist`.
- Workspace-wide Rust changes: select `just check`, `just clippy`, or
  `just test`. Use `just cov` for coverage work and `just test-publish` for
  changes to public crate packaging.

Report commands that passed separately from failed or unexecuted checks.
