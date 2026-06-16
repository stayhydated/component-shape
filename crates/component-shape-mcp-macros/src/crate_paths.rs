use super::*;

#[cfg(test)]
pub(crate) fn resolve_default_mcp_crate_path() -> syn::Result<Path> {
    Ok(parse_quote!(component_shape_mcp))
}

#[cfg(not(test))]
pub(crate) fn resolve_default_mcp_crate_path() -> syn::Result<Path> {
    if let Some(path) = resolve_package_path("component-shape-mcp", "crate", None) {
        return Ok(path);
    }

    let facade_paths = [
        ("gpui-form", "crate::mcp", "mcp"),
        ("gpui-table", "crate::mcp", "mcp"),
    ]
    .into_iter()
    .filter_map(|(package, itself_path, module)| {
        resolve_package_path(package, itself_path, Some(module)).map(|path| (package, path))
    })
    .collect::<Vec<_>>();

    match facade_paths.as_slice() {
        [(_, path)] => Ok(path.clone()),
        [] => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "MCP derive could not find `component-shape-mcp`, `gpui-form`, or `gpui-table` in this crate's manifest; add one of those dependencies or set `#[mcp(crate = path::to::mcp)]`",
        )),
        _ => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            multiple_facade_error_message(&facade_paths),
        )),
    }
}

pub(crate) fn multiple_facade_error_message(facade_paths: &[(&str, Path)]) -> String {
    let packages = facade_paths
        .iter()
        .map(|(package, _)| format!("`{package}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let suggestions = facade_paths
        .iter()
        .map(|(_, path)| format!("`#[mcp(crate = {})]`", display_path(path)))
        .collect::<Vec<_>>()
        .join(" or ");

    format!(
        "MCP derive found multiple MCP facade crates: {packages}; add {suggestions} to choose one"
    )
}

fn display_path(path: &Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

#[cfg(not(test))]
fn resolve_package_path(package: &str, itself_path: &str, module: Option<&str>) -> Option<Path> {
    let path = match crate_name(package).ok()? {
        FoundCrate::Itself => itself_path.to_string(),
        FoundCrate::Name(name) => match module {
            Some(module) => format!("::{name}::{module}"),
            None => format!("::{name}"),
        },
    };
    syn::parse_str(&path).ok()
}
