const BASE_URL: &str = "https://stayhydated.github.io/component-shape";
pub fn run() -> anyhow::Result<()> {
    let root = stayhydated_xtask::workspace_root_from_xtask_manifest()?;
    stayhydated_xtask::llms::build_workspace_llms(&root, BASE_URL, None)
}
