use super::*;

/// In-process MCP server that owns registered tools, resources, and prompts.
#[derive(Clone)]
pub struct McpServer {
    pub(crate) server_name: Cow<'static, str>,
    pub(crate) server_version: Cow<'static, str>,
    tools: BTreeMap<String, Arc<dyn ToolExecutor>>,
    resources: BTreeMap<String, Arc<dyn ResourceReader>>,
    resource_templates: Vec<ResourceTemplate>,
    prompts: BTreeMap<String, Arc<dyn PromptExecutor>>,
}

impl McpServer {
    /// Create a dynamic MCP tool server with the advertised metadata.
    pub fn new(
        server_name: impl Into<Cow<'static, str>>,
        server_version: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self {
            server_name: server_name.into(),
            server_version: server_version.into(),
            tools: BTreeMap::new(),
            resources: BTreeMap::new(),
            resource_templates: Vec::new(),
            prompts: BTreeMap::new(),
        }
    }

    /// Start building a dynamic MCP tool server with generated registrars.
    pub fn builder(
        server_name: impl Into<Cow<'static, str>>,
        server_version: impl Into<Cow<'static, str>>,
    ) -> McpServerBuilder {
        McpServerBuilder::new(server_name, server_version)
    }

    /// Register a synchronous MCP tool handler.
    ///
    /// # Errors
    ///
    /// Returns [`McpToolError`] when `definition` is invalid or a tool with the
    /// same name is already registered.
    pub fn add_tool<Call>(
        &mut self,
        definition: ToolDefinition,
        call: Call,
    ) -> Result<(), McpToolError>
    where
        Call: Fn(McpToolCall) -> ToolCallResult + Send + Sync + 'static,
    {
        let name = definition.name.to_string();
        validate_tool_definition(&definition)?;
        if self.tools.contains_key(&name) {
            return Err(McpToolError::duplicate_tool(name));
        }

        self.tools.insert(
            name,
            Arc::new(RegisteredTool {
                definition,
                call: Arc::new(move |arguments| Box::pin(std::future::ready(call(arguments)))),
            }),
        );
        Ok(())
    }

    /// Register a synchronous typed MCP tool handler.
    ///
    /// # Errors
    ///
    /// Returns [`McpToolError`] when `definition` is invalid or a tool with the
    /// same name is already registered.
    pub fn add_typed_tool<Input, Call>(
        &mut self,
        definition: McpTypedTool<Input>,
        call: Call,
    ) -> Result<(), McpToolError>
    where
        Input: McpToolInput,
        Call: Fn(Input) -> ToolCallResult + Send + Sync + 'static,
    {
        self.add_tool(definition.into_definition(), move |tool_call| {
            let input = match Input::from_tool_call(tool_call) {
                Ok(input) => input,
                Err(error) => return tool_error_result_for(error),
            };
            call(input)
        })
    }

    /// Register an async MCP tool handler.
    ///
    /// # Errors
    ///
    /// Returns [`McpToolError`] when `definition` is invalid or a tool with the
    /// same name is already registered.
    pub fn add_tool_async<Call, Fut>(
        &mut self,
        definition: ToolDefinition,
        call: Call,
    ) -> Result<(), McpToolError>
    where
        Call: Fn(McpToolCall) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ToolCallResult> + Send + 'static,
    {
        let name = definition.name.to_string();
        validate_tool_definition(&definition)?;
        if self.tools.contains_key(&name) {
            return Err(McpToolError::duplicate_tool(name));
        }

        self.tools.insert(
            name,
            Arc::new(RegisteredTool {
                definition,
                call: Arc::new(move |arguments| Box::pin(call(arguments))),
            }),
        );
        Ok(())
    }

    /// Register an async typed MCP tool handler.
    ///
    /// # Errors
    ///
    /// Returns [`McpToolError`] when `definition` is invalid or a tool with the
    /// same name is already registered.
    pub fn add_typed_tool_async<Input, Call, Fut>(
        &mut self,
        definition: McpTypedTool<Input>,
        call: Call,
    ) -> Result<(), McpToolError>
    where
        Input: McpToolInput,
        Call: Fn(Input) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ToolCallResult> + Send + 'static,
    {
        self.add_tool_async(definition.into_definition(), move |tool_call| {
            let input = Input::from_tool_call(tool_call);
            let future = match input {
                Ok(input) => call(input),
                Err(error) => {
                    return Box::pin(std::future::ready(tool_error_result_for(error)))
                        as ToolFuture;
                },
            };
            Box::pin(future) as ToolFuture
        })
    }

    /// Return registered MCP tool definitions.
    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|executor| executor.definition())
            .collect()
    }

    /// Whether a tool name is already registered.
    pub fn contains_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Number of registered tools.
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Register a static MCP resource reader.
    ///
    /// # Errors
    ///
    /// Returns [`McpToolError`] when `definition` is invalid or a resource with
    /// the same URI is already registered.
    pub fn add_resource<Read>(
        &mut self,
        definition: ResourceDefinition,
        read: Read,
    ) -> Result<(), McpToolError>
    where
        Read: Fn() -> ReadResourceResult + Send + Sync + 'static,
    {
        self.add_resource_async(definition, move || {
            let result = read();
            std::future::ready(Ok(result))
        })
    }

    /// Register an async MCP resource reader.
    ///
    /// # Errors
    ///
    /// Returns [`McpToolError`] when `definition` is invalid or a resource with
    /// the same URI is already registered.
    pub fn add_resource_async<Read, Fut>(
        &mut self,
        definition: ResourceDefinition,
        read: Read,
    ) -> Result<(), McpToolError>
    where
        Read: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ReadResourceResult, ErrorData>> + Send + 'static,
    {
        let uri = definition.uri.clone();
        validate_resource_definition(&definition)?;
        if self.resources.contains_key(&uri) {
            return Err(McpToolError::duplicate_resource(uri));
        }

        self.resources.insert(
            uri,
            Arc::new(RegisteredResource {
                definition,
                read: Arc::new(move || Box::pin(read())),
            }),
        );
        Ok(())
    }

    /// Register an MCP resource template advertised by `resources/templates/list`.
    ///
    /// # Errors
    ///
    /// Returns [`McpToolError`] when `definition` is invalid.
    pub fn add_resource_template(
        &mut self,
        definition: ResourceTemplateDefinition,
    ) -> Result<(), McpToolError> {
        validate_resource_template(&definition)?;
        self.resource_templates.push(definition);
        Ok(())
    }

    /// Return registered MCP resource definitions.
    pub fn list_resources(&self) -> Vec<ResourceDefinition> {
        self.resources
            .values()
            .map(|resource| resource.definition())
            .collect()
    }

    /// Return registered MCP resource template definitions.
    pub fn list_resource_templates(&self) -> Vec<ResourceTemplateDefinition> {
        self.resource_templates.clone()
    }

    /// Whether a resource URI is already registered.
    pub fn contains_resource(&self, uri: &str) -> bool {
        self.resources.contains_key(uri)
    }

    /// Number of registered concrete resources.
    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }

    /// Register a static MCP prompt.
    ///
    /// # Errors
    ///
    /// Returns [`McpToolError`] when `definition` is invalid or a prompt with
    /// the same name is already registered.
    pub fn add_prompt<Get>(
        &mut self,
        definition: PromptDefinition,
        get: Get,
    ) -> Result<(), McpToolError>
    where
        Get: Fn(Option<JsonObject>) -> GetPromptResult + Send + Sync + 'static,
    {
        self.add_prompt_async(definition, move |arguments| {
            let result = get(arguments);
            std::future::ready(Ok(result))
        })
    }

    /// Register an async MCP prompt.
    ///
    /// # Errors
    ///
    /// Returns [`McpToolError`] when `definition` is invalid or a prompt with
    /// the same name is already registered.
    pub fn add_prompt_async<Get, Fut>(
        &mut self,
        definition: PromptDefinition,
        get: Get,
    ) -> Result<(), McpToolError>
    where
        Get: Fn(Option<JsonObject>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<GetPromptResult, ErrorData>> + Send + 'static,
    {
        let name = definition.name.clone();
        validate_prompt_definition(&definition)?;
        if self.prompts.contains_key(&name) {
            return Err(McpToolError::duplicate_prompt(name));
        }

        self.prompts.insert(
            name,
            Arc::new(RegisteredPrompt {
                definition,
                get: Arc::new(move |arguments| Box::pin(get(arguments))),
            }),
        );
        Ok(())
    }

    /// Return registered MCP prompt definitions.
    pub fn list_prompts(&self) -> Vec<PromptDefinition> {
        self.prompts
            .values()
            .map(|prompt| prompt.definition())
            .collect()
    }

    /// Whether a prompt name is already registered.
    pub fn contains_prompt(&self, name: &str) -> bool {
        self.prompts.contains_key(name)
    }

    /// Number of registered prompts.
    pub fn prompt_count(&self) -> usize {
        self.prompts.len()
    }

    /// Calls a registered tool and converts validation or handler failures into
    /// a protocol-level tool result.
    pub fn call_tool(&self, name: &str, arguments: Option<Value>) -> ToolCallResult {
        match self.tools.get(name) {
            Some(executor) => {
                let call = match McpToolCall::from_value(arguments) {
                    Ok(call) => call,
                    Err(error) => return tool_error_result_for(error),
                };
                let output_schema = executor.definition().output_schema;
                validate_tool_call_result(
                    name,
                    output_schema.as_deref(),
                    block_on_tool_future(executor.call(call)),
                )
            },
            None => tool_error_result_for(McpToolError::UnknownTool {
                name: name.to_string(),
            }),
        }
    }

    /// Serve this server over stdin/stdout using the MCP stdio transport.
    ///
    /// # Errors
    ///
    /// Returns an error when stdio serving fails or the service task fails.
    pub async fn serve_stdio(self) -> ServeStdioResult {
        let service = self.serve(stdio()).await?;
        service.waiting().await?;
        Ok(())
    }

    /// Serve this server over stdin/stdout on a new Tokio runtime.
    ///
    /// # Errors
    ///
    /// Returns an error when the Tokio runtime cannot be created, stdio serving
    /// fails, or the service task fails.
    pub fn serve_stdio_blocking(self) -> ServeStdioResult {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        runtime.block_on(self.serve_stdio())
    }
}

/// Builder for composing generated MCP server registrars.
#[derive(Clone)]
pub struct McpServerBuilder {
    server: Result<McpServer, McpToolError>,
}

impl McpServerBuilder {
    /// Create a builder with advertised server metadata.
    pub fn new(
        server_name: impl Into<Cow<'static, str>>,
        server_version: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self {
            server: Ok(McpServer::new(server_name, server_version)),
        }
    }

    /// Runs a registrar against the server being built.
    pub fn register<Register>(mut self, register: Register) -> Self
    where
        Register: FnOnce(&mut McpServer) -> Result<(), McpToolError>,
    {
        if let Ok(server) = self.server.as_mut()
            && let Err(error) = register(server)
        {
            self.server = Err(error);
        }
        self
    }

    /// Add a synchronous MCP tool to the server being built.
    pub fn tool<Call>(self, definition: ToolDefinition, call: Call) -> Self
    where
        Call: Fn(McpToolCall) -> ToolCallResult + Send + Sync + 'static,
    {
        self.register(move |server| server.add_tool(definition, call))
    }

    /// Add a synchronous typed MCP tool to the server being built.
    pub fn typed_tool<Input, Call>(self, definition: McpTypedTool<Input>, call: Call) -> Self
    where
        Input: McpToolInput,
        Call: Fn(Input) -> ToolCallResult + Send + Sync + 'static,
    {
        self.register(move |server| server.add_typed_tool(definition, call))
    }

    /// Add an async MCP tool to the server being built.
    pub fn tool_async<Call, Fut>(self, definition: ToolDefinition, call: Call) -> Self
    where
        Call: Fn(McpToolCall) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ToolCallResult> + Send + 'static,
    {
        self.register(move |server| server.add_tool_async(definition, call))
    }

    /// Add an async typed MCP tool to the server being built.
    pub fn typed_tool_async<Input, Call, Fut>(
        self,
        definition: McpTypedTool<Input>,
        call: Call,
    ) -> Self
    where
        Input: McpToolInput,
        Call: Fn(Input) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ToolCallResult> + Send + 'static,
    {
        self.register(move |server| server.add_typed_tool_async(definition, call))
    }

    /// Add a static MCP resource to the server being built.
    pub fn resource<Read>(self, definition: ResourceDefinition, read: Read) -> Self
    where
        Read: Fn() -> ReadResourceResult + Send + Sync + 'static,
    {
        self.register(move |server| server.add_resource(definition, read))
    }

    /// Add an async MCP resource to the server being built.
    pub fn resource_async<Read, Fut>(self, definition: ResourceDefinition, read: Read) -> Self
    where
        Read: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ReadResourceResult, ErrorData>> + Send + 'static,
    {
        self.register(move |server| server.add_resource_async(definition, read))
    }

    /// Add an MCP resource template to the server being built.
    pub fn resource_template(self, definition: ResourceTemplateDefinition) -> Self {
        self.register(move |server| server.add_resource_template(definition))
    }

    /// Add a static MCP prompt to the server being built.
    pub fn prompt<Get>(self, definition: PromptDefinition, get: Get) -> Self
    where
        Get: Fn(Option<JsonObject>) -> GetPromptResult + Send + Sync + 'static,
    {
        self.register(move |server| server.add_prompt(definition, get))
    }

    /// Add an async MCP prompt to the server being built.
    pub fn prompt_async<Get, Fut>(self, definition: PromptDefinition, get: Get) -> Self
    where
        Get: Fn(Option<JsonObject>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<GetPromptResult, ErrorData>> + Send + 'static,
    {
        self.register(move |server| server.add_prompt_async(definition, get))
    }

    /// Finish the builder and return the composed server.
    ///
    /// # Errors
    ///
    /// Returns the first [`McpToolError`] produced by a builder registrar.
    pub fn build(self) -> Result<McpServer, McpToolError> {
        self.server
    }

    /// Build and serve this server over stdin/stdout using the MCP stdio transport.
    ///
    /// # Errors
    ///
    /// Returns an error when a builder registrar fails, stdio serving fails, or
    /// the service task fails.
    pub async fn serve_stdio(self) -> ServeStdioResult {
        self.build()?.serve_stdio().await
    }

    /// Build and serve this server over stdin/stdout on a new Tokio runtime.
    ///
    /// # Errors
    ///
    /// Returns an error when a builder registrar fails, the Tokio runtime cannot
    /// be created, stdio serving fails, or the service task fails.
    pub fn serve_stdio_blocking(self) -> ServeStdioResult {
        self.build()?.serve_stdio_blocking()
    }
}

impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        let mut capabilities = ServerCapabilities::builder().enable_tools().build();
        capabilities.resources = (!self.resources.is_empty()
            || !self.resource_templates.is_empty())
        .then(Default::default);
        capabilities.prompts = (!self.prompts.is_empty()).then(Default::default);
        ServerInfo::new(capabilities)
            .with_protocol_version(ProtocolVersion::V_2025_11_25)
            .with_server_info(Implementation::new(
                self.server_name.clone(),
                self.server_version.clone(),
            ))
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + MaybeSendFuture + '_ {
        std::future::ready(Ok(ListToolsResult {
            tools: self.list_tools(),
            next_cursor: None,
            meta: None,
        }))
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tools.get(name).map(|executor| executor.definition())
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ToolCallResult, ErrorData>> + MaybeSendFuture + '_ {
        let name = request.name.to_string();
        let call = McpToolCall::new(request.arguments.unwrap_or_default());
        let call = match self.tools.get(&name) {
            Some(executor) => {
                let output_schema = executor.definition().output_schema;
                let name = name.clone();
                let call = executor.call(call);
                Box::pin(async move {
                    validate_tool_call_result(&name, output_schema.as_deref(), call.await)
                }) as ToolFuture
            },
            None => {
                let result = tool_error_result_for(McpToolError::UnknownTool {
                    name: name.to_string(),
                });
                Box::pin(std::future::ready(result))
            },
        };
        async move { Ok(call.await) }
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, ErrorData>> + MaybeSendFuture + '_ {
        std::future::ready(Ok(ListResourcesResult {
            resources: self.list_resources(),
            next_cursor: None,
            meta: None,
        }))
    }

    fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourceTemplatesResult, ErrorData>> + MaybeSendFuture + '_
    {
        std::future::ready(Ok(ListResourceTemplatesResult {
            resource_templates: self.list_resource_templates(),
            next_cursor: None,
            meta: None,
        }))
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ReadResourceResult, ErrorData>> + MaybeSendFuture + '_ {
        let uri = request.uri;
        let read = self.resources.get(&uri).map(|resource| resource.read());
        async move {
            match read {
                Some(read) => read.await,
                None => Err(ErrorData::resource_not_found(
                    format!("resource `{uri}` not found"),
                    Some(McpToolError::unknown_resource(uri).to_structured_value()),
                )),
            }
        }
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListPromptsResult, ErrorData>> + MaybeSendFuture + '_ {
        std::future::ready(Ok(ListPromptsResult {
            prompts: self.list_prompts(),
            next_cursor: None,
            meta: None,
        }))
    }

    fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<GetPromptResult, ErrorData>> + MaybeSendFuture + '_ {
        let name = request.name;
        let get = self
            .prompts
            .get(&name)
            .map(|prompt| prompt.get(request.arguments));
        async move {
            match get {
                Some(get) => get.await,
                None => Err(ErrorData::invalid_params(
                    format!("prompt `{name}` not found"),
                    Some(McpToolError::unknown_prompt(name).to_structured_value()),
                )),
            }
        }
    }
}

trait ToolExecutor: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    fn call(&self, call: McpToolCall) -> ToolFuture;
}

struct RegisteredTool {
    definition: ToolDefinition,
    call: Arc<dyn Fn(McpToolCall) -> ToolFuture + Send + Sync>,
}

impl ToolExecutor for RegisteredTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    fn call(&self, call: McpToolCall) -> ToolFuture {
        (self.call)(call)
    }
}

trait ResourceReader: Send + Sync {
    fn definition(&self) -> ResourceDefinition;
    fn read(&self) -> ResourceFuture;
}

struct RegisteredResource {
    definition: ResourceDefinition,
    read: Arc<dyn Fn() -> ResourceFuture + Send + Sync>,
}

impl ResourceReader for RegisteredResource {
    fn definition(&self) -> ResourceDefinition {
        self.definition.clone()
    }

    fn read(&self) -> ResourceFuture {
        (self.read)()
    }
}

trait PromptExecutor: Send + Sync {
    fn definition(&self) -> PromptDefinition;
    fn get(&self, arguments: Option<JsonObject>) -> PromptFuture;
}

struct RegisteredPrompt {
    definition: PromptDefinition,
    get: Arc<dyn Fn(Option<JsonObject>) -> PromptFuture + Send + Sync>,
}

impl PromptExecutor for RegisteredPrompt {
    fn definition(&self) -> PromptDefinition {
        self.definition.clone()
    }

    fn get(&self, arguments: Option<JsonObject>) -> PromptFuture {
        (self.get)(arguments)
    }
}

fn validate_tool_call_result(
    tool_name: &str,
    output_schema: Option<&JsonObject>,
    result: ToolCallResult,
) -> ToolCallResult {
    let Some(output_schema) = output_schema else {
        return result;
    };
    if result.is_error == Some(true) {
        return result;
    }

    let Some(structured_content) = result.structured_content.as_ref() else {
        return tool_error_result_for(McpToolError::invalid_tool_output(
            tool_name,
            "tool declares output_schema but returned no structured_content",
        ));
    };

    if !structured_content.is_object() {
        return tool_error_result_for(McpToolError::invalid_tool_output(
            tool_name,
            "tool declares output_schema with object root but returned non-object structured_content",
        ));
    }

    let output_schema = Value::Object(output_schema.clone());
    if let Err(error) = validate_value_against_closed_schema(
        "structured_content",
        &output_schema,
        structured_content,
    ) {
        return tool_error_result_for(McpToolError::invalid_tool_output(
            tool_name,
            error.to_string(),
        ));
    }

    result
}

fn block_on_tool_future(future: ToolFuture) -> ToolCallResult {
    let join = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(runtime.block_on(future))
    })
    .join();

    match join {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => tool_error_result_for(McpToolError::handler(format!(
            "failed to run async tool handler: {error}"
        ))),
        Err(_) => {
            tool_error_result_for(McpToolError::handler("async tool handler runtime panicked"))
        },
    }
}
