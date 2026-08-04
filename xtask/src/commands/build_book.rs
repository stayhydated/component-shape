pub fn run() -> anyhow::Result<()> {
    let root = stayhydated_xtask::workspace_root_from_xtask_manifest()?;
    stayhydated_xtask::book::build_workspace_book(&root)
}
