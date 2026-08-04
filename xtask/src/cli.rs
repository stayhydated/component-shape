use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "xtask",
    about = "Workspace maintenance tasks.",
    disable_help_subcommand = true,
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Build generated workspace artifacts
    Build {
        #[command(subcommand)]
        target: BuildCommand,
    },
    /// Preview generated workspace artifacts
    Preview {
        #[command(subcommand)]
        target: PreviewCommand,
    },
    /// Release workspace crates in registry dependency order
    Release {
        #[command(subcommand)]
        action: ReleaseCommand,
    },
}
#[derive(Debug, Subcommand)]
pub enum BuildCommand {
    /// Build mdBook documentation to web/public/book
    Book,
    /// Build llms.txt from mdBook sources to web/public/llms.txt
    LlmsTxt,
    /// Build the Dioxus site into web/dist for GitHub Pages
    Web,
}
#[derive(Debug, Subcommand)]
pub enum PreviewCommand {
    /// Preview the generated static site with its GitHub Pages base path
    Web,
}
#[derive(Debug, Subcommand)]
pub enum ReleaseCommand {
    /// Print the publish order for workspace crates
    Plan,
    /// Publish workspace crates in registry dependency order
    Publish(ReleasePublishArgs),
}
#[derive(Args, Debug)]
pub struct ReleasePublishArgs {
    #[arg(long)]
    pub execute: bool,
    #[arg(long)]
    pub from: Option<String>,
    #[arg(long)]
    pub registry: Option<String>,
    #[arg(long)]
    pub allow_dirty: bool,
    #[arg(long)]
    pub no_verify: bool,
    #[arg(long)]
    pub include_dev_deps: bool,
    #[arg(long)]
    pub skip_existing: bool,
    #[arg(long, default_value_t = 3)]
    pub retries: u32,
    #[arg(long, default_value_t = 20)]
    pub retry_delay_seconds: u64,
}
