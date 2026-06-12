use proc_macro_crate::{FoundCrate, crate_name};
use syn::Path;

#[derive(Clone)]
pub struct CratePaths {
    pub component_shape: Path,
    pub component_shape_gpui: Path,
    pub gpui: Path,
}

impl CratePaths {
    pub fn resolve() -> Self {
        let component_shape_gpui =
            resolve_crate_path("component-shape-gpui", "::component_shape_gpui");
        Self {
            component_shape: component_shape_gpui.clone(),
            component_shape_gpui,
            gpui: resolve_crate_path("gpui", "::gpui"),
        }
    }
}

pub fn resolve_crate_path(package_name: &str, fallback: &str) -> Path {
    let path = match crate_name(package_name) {
        Ok(FoundCrate::Itself) => "crate".to_string(),
        Ok(FoundCrate::Name(name)) => format!("::{name}"),
        Err(_) => fallback.to_string(),
    };

    syn::parse_str(&path).expect("crate path resolver produced a valid Rust path")
}
