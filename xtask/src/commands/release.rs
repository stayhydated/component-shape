use crate::cli::ReleasePublishArgs;
use stayhydated_xtask::release::PublishOptions;

pub fn plan() -> anyhow::Result<()> {
    let root = stayhydated_xtask::workspace_root_from_xtask_manifest()?;
    stayhydated_xtask::release::plan(&root)
}
pub fn publish(args: &ReleasePublishArgs) -> anyhow::Result<()> {
    let root = stayhydated_xtask::workspace_root_from_xtask_manifest()?;
    let options = PublishOptions::new(args.execute)
        .resume_from(args.from.clone())?
        .registry(args.registry.clone())?
        .allow_dirty(args.allow_dirty)
        .no_verify(args.no_verify)
        .include_dev_deps(args.include_dev_deps)
        .skip_existing(args.skip_existing)
        .retries(args.retries)
        .retry_delay_seconds(args.retry_delay_seconds);
    stayhydated_xtask::release::publish(&root, &options)
}
