use super::*;

#[test]
fn json_resource_specs_register_and_reuse_generated_resources() {
    let specs = vec![
        crate::McpJsonResourceSpec::new(
            "gpui-form://forms/contact/descriptor",
            "contact_descriptor",
            Some("Contact descriptor".to_string()),
            Some("Descriptor for the contact form.".to_string()),
            json!({ "fields": ["name", "email"] }),
        )
        .expect("resource spec should build"),
    ];
    let definitions = crate::json_resource_definitions(&specs).expect("definitions should build");
    assert_eq!(definitions[0].uri, "gpui-form://forms/contact/descriptor");
    assert_eq!(
        definitions[0].mime_type.as_deref(),
        Some("application/json")
    );

    let mut server = crate::McpServer::new("test", "0.0.0");
    crate::register_json_resource_specs_if_missing(&mut server, specs.clone())
        .expect("resources should register");
    crate::register_json_resource_specs_if_missing(&mut server, specs)
        .expect("existing complete resource set should be reused");

    assert_eq!(server.resource_count(), 1);
    assert!(server.contains_resource("gpui-form://forms/contact/descriptor"));
}

#[test]
fn json_resource_specs_reject_duplicate_uris() {
    let specs = vec![
        crate::McpJsonResourceSpec::new(
            "gpui-form://forms/contact/descriptor",
            "contact_descriptor",
            None,
            None,
            json!({}),
        )
        .expect("resource spec should build"),
        crate::McpJsonResourceSpec::new(
            "gpui-form://forms/contact/descriptor",
            "duplicate_contact_descriptor",
            None,
            None,
            json!({}),
        )
        .expect("resource spec should build"),
    ];

    assert_eq!(
        crate::ensure_json_resource_specs_distinct(&specs).expect_err("duplicate URI should fail"),
        crate::McpToolError::duplicate_resource("gpui-form://forms/contact/descriptor")
    );
}

#[test]
fn server_exposes_resources_and_prompts_through_rmcp_protocol() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime should start");

    runtime.block_on(async {
        use rmcp::{
            ClientLifecycleMode, ClientServiceExt as _, ServerHandler as _, ServiceExt as _,
            model::{
                CacheScope, GetPromptRequestParams, ProtocolVersion, ReadResourceRequestParams,
                ResultType,
            },
        };

        let descriptor_uri = "gpui-form://forms/contact/descriptor";
        let descriptor = json!({
            "tool": "form_contact",
            "fields": ["name", "email"]
        });
        let descriptor_resource = descriptor.clone();
        let mut server = McpServer::new("test-server", "0.0.0");
        server
            .add_resource(
                crate::resource_definition(
                    descriptor_uri,
                    "contact_descriptor",
                    Some("Contact descriptor".to_string()),
                    Some("Descriptor for the contact form.".to_string()),
                    Some("application/json".to_string()),
                )
                .expect("resource definition should build"),
                move || {
                    crate::json_resource_result(descriptor_uri, &descriptor_resource)
                        .expect("descriptor resource should encode")
                },
            )
            .expect("resource should register");
        server
            .add_resource_template(
                crate::resource_template_definition(
                    "gpui-form://forms/{form}/descriptor",
                    "gpui_form_descriptor",
                    Some("GPUI form descriptor".to_string()),
                    Some("Descriptor for a generated GPUI form.".to_string()),
                    Some("application/json".to_string()),
                )
                .expect("resource template should build"),
            )
            .expect("resource template should register");
        server
            .add_prompt(
                crate::prompt_definition(
                    "draft_contact",
                    Some("Draft contact".to_string()),
                    Some("Draft values for the contact form.".to_string()),
                    None,
                )
                .expect("prompt definition should build"),
                |_| {
                    crate::text_prompt_result(
                        Some("Draft values for the contact form.".to_string()),
                        "Use the contact descriptor resource and return valid form fields.",
                    )
                },
            )
            .expect("prompt should register");

        let info = server.get_info();
        assert!(info.capabilities.resources.is_some());
        assert!(info.capabilities.prompts.is_some());

        let (server_transport, client_transport) = tokio::io::duplex(4096);
        let server_handle = tokio::spawn(async move {
            let service = server
                .serve(server_transport)
                .await
                .expect("server should start");
            service.waiting().await.expect("server task should join");
        });

        let client = ()
            .serve_with_lifecycle(
                client_transport,
                ClientLifecycleMode::Discover {
                    preferred_versions: vec![ProtocolVersion::V_2026_07_28],
                },
            )
            .await
            .expect("client should start");

        let resources = client
            .peer()
            .list_resources(Default::default())
            .await
            .expect("resources/list should succeed");
        assert_eq!(resources.resources.len(), 1);
        assert_eq!(resources.result_type, Some(ResultType::COMPLETE));
        assert_eq!(resources.ttl_ms, Some(0));
        assert_eq!(resources.cache_scope, Some(CacheScope::Private));
        assert_eq!(resources.resources[0].uri, descriptor_uri);
        assert_eq!(
            resources.resources[0].title.as_deref(),
            Some("Contact descriptor")
        );

        let templates = client
            .peer()
            .list_resource_templates(Default::default())
            .await
            .expect("resources/templates/list should succeed");
        assert_eq!(templates.resource_templates.len(), 1);
        assert_eq!(templates.result_type, Some(ResultType::COMPLETE));
        assert_eq!(templates.ttl_ms, Some(0));
        assert_eq!(templates.cache_scope, Some(CacheScope::Private));
        assert_eq!(
            templates.resource_templates[0].uri_template,
            "gpui-form://forms/{form}/descriptor"
        );

        let resource = client
            .peer()
            .read_resource(ReadResourceRequestParams::new(descriptor_uri))
            .await
            .expect("resources/read should succeed");
        assert_eq!(resource.contents.len(), 1);
        assert_eq!(resource.result_type, Some(ResultType::COMPLETE));
        assert_eq!(resource.ttl_ms, Some(0));
        assert_eq!(resource.cache_scope, Some(CacheScope::Private));
        match &resource.contents[0] {
            crate::McpResourceContents::TextResourceContents {
                uri,
                mime_type,
                text,
                ..
            } => {
                assert_eq!(uri, descriptor_uri);
                assert_eq!(mime_type.as_deref(), Some("application/json"));
                let value: Value =
                    serde_json::from_str(text).expect("resource should contain JSON");
                assert_eq!(value, descriptor);
            },
            other => panic!("expected text resource contents, got {other:?}"),
        }

        let prompts = client
            .peer()
            .list_prompts(Default::default())
            .await
            .expect("prompts/list should succeed");
        assert_eq!(prompts.prompts.len(), 1);
        assert_eq!(prompts.result_type, Some(ResultType::COMPLETE));
        assert_eq!(prompts.ttl_ms, Some(0));
        assert_eq!(prompts.cache_scope, Some(CacheScope::Private));
        assert_eq!(prompts.prompts[0].name, "draft_contact");
        assert_eq!(prompts.prompts[0].title.as_deref(), Some("Draft contact"));

        let prompt = client
            .peer()
            .get_prompt(GetPromptRequestParams::new("draft_contact"))
            .await
            .expect("prompts/get should succeed");
        assert_eq!(
            prompt.description.as_deref(),
            Some("Draft values for the contact form.")
        );
        assert_eq!(prompt.result_type, Some(ResultType::COMPLETE));
        assert_eq!(prompt.messages.len(), 1);
        assert_eq!(prompt.messages[0].role, crate::McpRole::User);
        match &prompt.messages[0].content {
            crate::ContentBlock::Text(content) => {
                assert!(content.text.contains("Use the contact descriptor resource"))
            },
            other => panic!("expected text prompt message, got {other:?}"),
        }

        client.cancel().await.expect("client should close");
        server_handle.await.expect("server should finish");
    });
}

#[test]
fn server_builder_registers_resources_templates_and_prompts() {
    let resource = || {
        crate::resource_definition(
            "shape://sync",
            "sync_resource",
            None,
            None,
            Some("text/plain".to_string()),
        )
        .expect("resource should build")
    };
    let async_resource = crate::resource_definition(
        "shape://async",
        "async_resource",
        None,
        None,
        Some("text/plain".to_string()),
    )
    .expect("resource should build");
    let template = crate::resource_template_definition(
        "shape://{name}",
        "shape_template",
        None,
        None,
        Some("text/plain".to_string()),
    )
    .expect("template should build");
    let prompt =
        crate::prompt_definition("sync_prompt", None, None, None).expect("prompt should build");
    let async_prompt =
        crate::prompt_definition("async_prompt", None, None, None).expect("prompt should build");

    let server = crate::McpServer::builder("builder", "0.0.0")
        .resource(resource(), || {
            crate::text_resource_result("shape://sync", "sync", "text/plain")
        })
        .resource_async(async_resource, || async {
            Ok(crate::text_resource_result(
                "shape://async",
                "async",
                "text/plain",
            ))
        })
        .resource_template(template)
        .prompt(prompt, |_| crate::text_prompt_result(None, "sync"))
        .prompt_async(async_prompt, |_| async {
            Ok(crate::text_prompt_result(None, "async"))
        })
        .build()
        .expect("builder should register resources and prompts");

    assert_eq!(server.resource_count(), 2);
    assert_eq!(server.list_resource_templates().len(), 1);
    assert_eq!(server.prompt_count(), 2);
    assert!(server.contains_prompt("sync_prompt"));

    let mut duplicates = crate::McpServer::new("duplicates", "0.0.0");
    duplicates
        .add_resource(resource(), || {
            crate::text_resource_result("shape://sync", "sync", "text/plain")
        })
        .expect("first resource should register");
    assert!(matches!(
        duplicates.add_resource(resource(), || {
            crate::text_resource_result("shape://sync", "sync", "text/plain")
        }),
        Err(crate::McpToolError::DuplicateResource { .. })
    ));

    let prompt = || {
        crate::prompt_definition("duplicate_prompt", None, None, None).expect("prompt should build")
    };
    duplicates
        .add_prompt(prompt(), |_| crate::text_prompt_result(None, "prompt"))
        .expect("first prompt should register");
    assert!(matches!(
        duplicates.add_prompt(prompt(), |_| crate::text_prompt_result(None, "prompt")),
        Err(crate::McpToolError::DuplicatePrompt { .. })
    ));
}
