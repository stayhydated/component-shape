use dioxus::prelude::*;
use stayhydated_dioxus::{Project, ProjectSite, StayhydatedSinglePageProjectApp};

const PROJECT: Project = Project::new(
    "component-shape",
    "Framework-neutral component metadata, GPUI contracts, and shared MCP integrations for Rust.",
)
.with_skill_command("npx skills add stayhydated/component-shape");
const SITE_URL: &str = "https://stayhydated.github.io/component-shape/";
const RUSTDOC_URL: &str = "https://docs.rs/component-shape/";
const SOURCE_URL: &str = "https://github.com/stayhydated/component-shape";
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn site() -> ProjectSite {
    ProjectSite::builder()
        .project(PROJECT)
        .site_url(SITE_URL)
        .rustdoc_url(RUSTDOC_URL)
        .source_url(SOURCE_URL)
        .version(VERSION)
        .site_stylesheet_path("assets/site.css")
        .build()
}

#[component]
pub fn App() -> Element {
    rsx! { StayhydatedSinglePageProjectApp { site: site() } }
}

pub fn route_manifest() -> stayhydated_site::SiteRouteManifest {
    site().single_page_route_manifest()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_page_site_keeps_the_project_theme_and_no_demo() {
        let site = site();

        assert_eq!(site.site_stylesheet_path(), Some("assets/site.css"));
        assert_eq!(site.demo_path(), None);
        assert_eq!(site.rustdoc_url(), RUSTDOC_URL);
        assert_eq!(site.source_url(), SOURCE_URL);
        assert_eq!(
            site.project().skill_command(),
            Some("npx skills add stayhydated/component-shape")
        );
    }
}
