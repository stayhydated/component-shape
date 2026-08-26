# Servers and stdio

Use one `McpServer` to compose generated registrars with application-owned
tools, resources, resource templates, and prompts. Duplicate tool names,
resource URIs, and prompt names fail registration instead of silently replacing
an existing entry.

`McpToolRegistry` owns reusable MCP definitions and handlers. Use
`McpServer::from_tool_registry` when the application builds that tool surface
independently. Registry clones share handler allocations, while each server
retains its own identity, resources, prompts, and transport lifecycle.

## Compose registrations

Start with `McpServer::builder(name, version)`. Chain generated registrars
through `register`, or add custom entries through the matching builder or
mutable-server methods.

The common definition helpers cover static integrations:

- `resource_definition` and `json_resource_result` for JSON resources.
- `resource_template_definition` for resource templates.
- `prompt_definition` and `text_prompt_result` for text prompts.

Use async resource, prompt, or tool handlers when the application must await
I/O.

## Choose a serving boundary

- Call `build()?` when the caller owns the transport.
- Call `serve_stdio().await` from an async application.
- Call `serve_stdio_blocking()` at a synchronous binary boundary.

The stdio server uses newline-delimited JSON-RPC and delegates MCP lifecycle
handling to `rmcp`. Treat it as a long-lived host: the registry and stateful
handlers remain alive across calls until EOF, cancellation, or another
application-owned shutdown signal ends the transport. Serving one call and
then stopping is an explicit host policy.

## Smoke-test a binary

Use `McpStdioSmokeClient` for process-level tests that launch a real server
with piped stdin, stdout, and stderr. Exercise discovery, listing, resource
reads, and tool calls through the client. Use
`tool_call_structured_content` to read structured output without depending on
the protocol field spelling.

Keep these tests at the application boundary. Use focused schema, decoding, and
registration tests for library-level behavior.
