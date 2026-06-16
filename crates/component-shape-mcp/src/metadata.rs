use super::*;

/// Static icon metadata for an MCP tool definition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct McpToolIcon {
    src: &'static str,
    mime_type: Option<&'static str>,
    sizes: &'static [&'static str],
    theme: Option<McpIconTheme>,
}

impl McpToolIcon {
    /// Create icon metadata from an icon resource URI or data URI.
    pub const fn new(src: &'static str) -> Self {
        Self {
            src,
            mime_type: None,
            sizes: &[],
            theme: None,
        }
    }

    /// Override the icon MIME type.
    pub const fn with_mime_type(mut self, mime_type: &'static str) -> Self {
        self.mime_type = Some(mime_type);
        self
    }

    /// Declare supported icon sizes such as `"48x48"` or `"any"`.
    pub const fn with_sizes(mut self, sizes: &'static [&'static str]) -> Self {
        self.sizes = sizes;
        self
    }

    /// Declare the icon's intended theme.
    pub const fn with_theme(mut self, theme: McpIconTheme) -> Self {
        self.theme = Some(theme);
        self
    }

    pub const fn src(self) -> &'static str {
        self.src
    }

    pub const fn mime_type(self) -> Option<&'static str> {
        self.mime_type
    }

    pub const fn sizes(self) -> &'static [&'static str] {
        self.sizes
    }

    pub const fn theme(self) -> Option<McpIconTheme> {
        self.theme
    }

    fn into_definition_icon(self) -> McpIcon {
        let mut icon = McpIcon::new(self.src);
        if let Some(mime_type) = self.mime_type {
            icon = icon.with_mime_type(mime_type);
        }
        if !self.sizes.is_empty() {
            icon = icon.with_sizes(self.sizes.iter().map(|size| (*size).to_string()).collect());
        }
        if let Some(theme) = self.theme {
            icon = icon.with_theme(theme);
        }
        icon
    }

    fn validate(self) -> Result<(), McpToolError> {
        validate_required_metadata_text("icon src", self.src)?;
        if let Some(mime_type) = self.mime_type {
            validate_required_metadata_text("icon mime_type", mime_type)?;
        }
        for size in self.sizes {
            validate_required_metadata_text("icon size", size)?;
        }
        Ok(())
    }
}

/// Optional application-facing metadata for a generated MCP tool.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McpToolMetadata {
    name: Option<&'static str>,
    title: Option<&'static str>,
    description: Option<&'static str>,
    read_only_hint: Option<bool>,
    destructive_hint: Option<bool>,
    idempotent_hint: Option<bool>,
    open_world_hint: Option<bool>,
    icons: &'static [McpToolIcon],
    task_support: Option<McpToolTaskSupport>,
}

impl McpToolMetadata {
    pub const fn new() -> Self {
        Self {
            name: None,
            title: None,
            description: None,
            read_only_hint: None,
            destructive_hint: None,
            idempotent_hint: None,
            open_world_hint: None,
            icons: &[],
            task_support: None,
        }
    }

    pub const fn with_name(mut self, name: &'static str) -> Self {
        self.name = Some(name);
        self
    }

    pub const fn with_title(mut self, title: &'static str) -> Self {
        self.title = Some(title);
        self
    }

    pub const fn with_description(mut self, description: &'static str) -> Self {
        self.description = Some(description);
        self
    }

    pub const fn with_read_only_hint(mut self, read_only: bool) -> Self {
        self.read_only_hint = Some(read_only);
        self
    }

    pub const fn with_destructive_hint(mut self, destructive: bool) -> Self {
        self.destructive_hint = Some(destructive);
        self
    }

    pub const fn with_idempotent_hint(mut self, idempotent: bool) -> Self {
        self.idempotent_hint = Some(idempotent);
        self
    }

    pub const fn with_open_world_hint(mut self, open_world: bool) -> Self {
        self.open_world_hint = Some(open_world);
        self
    }

    pub const fn with_icons(mut self, icons: &'static [McpToolIcon]) -> Self {
        self.icons = icons;
        self
    }

    pub const fn with_task_support(mut self, task_support: McpToolTaskSupport) -> Self {
        self.task_support = Some(task_support);
        self
    }

    pub const fn name(self) -> Option<&'static str> {
        self.name
    }

    pub const fn title(self) -> Option<&'static str> {
        self.title
    }

    pub const fn description(self) -> Option<&'static str> {
        self.description
    }

    pub const fn read_only_hint(self) -> Option<bool> {
        self.read_only_hint
    }

    pub const fn destructive_hint(self) -> Option<bool> {
        self.destructive_hint
    }

    pub const fn idempotent_hint(self) -> Option<bool> {
        self.idempotent_hint
    }

    pub const fn open_world_hint(self) -> Option<bool> {
        self.open_world_hint
    }

    pub const fn icons(self) -> &'static [McpToolIcon] {
        self.icons
    }

    pub const fn task_support(self) -> Option<McpToolTaskSupport> {
        self.task_support
    }

    pub fn tool_annotations(self) -> Option<McpToolAnnotations> {
        if self.read_only_hint.is_none()
            && self.destructive_hint.is_none()
            && self.idempotent_hint.is_none()
            && self.open_world_hint.is_none()
        {
            return None;
        }

        Some(McpToolAnnotations::from_raw(
            self.title.map(str::to_string),
            self.read_only_hint,
            self.destructive_hint,
            self.idempotent_hint,
            self.open_world_hint,
        ))
    }

    pub fn tool_icons(self) -> Option<Vec<McpIcon>> {
        (!self.icons.is_empty()).then(|| {
            self.icons
                .iter()
                .map(|icon| icon.into_definition_icon())
                .collect()
        })
    }

    pub fn tool_execution(self) -> Option<McpToolExecution> {
        self.task_support
            .map(|task_support| McpToolExecution::from_raw(Some(task_support)))
    }

    pub fn validate(self) -> Result<(), McpToolError> {
        validate_tool_annotation_hints(self.read_only_hint, self.destructive_hint)?;
        if let Some(name) = self.name {
            validate_tool_name(name)?;
        }
        if let Some(title) = self.title {
            validate_tool_metadata_text("title", title)?;
        }
        if let Some(description) = self.description {
            validate_tool_metadata_text("description", description)?;
        }
        for icon in self.icons {
            icon.validate()?;
        }
        Ok(())
    }
}
