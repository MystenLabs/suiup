use anyhow::Result;
use clap::Args;

use crate::handle_commands::handle_cmd;

use super::ComponentCommands;

/// Remove old release archives from the cache directory.
#[derive(Args, Debug)]
pub struct Command {
    /// Days to keep files in cache
    #[clap(long, short = 'd', default_value = "30")]
    days: u32,

    /// Remove all cache files
    #[clap(long, conflicts_with = "days")]
    all: bool,

    /// Show what would be removed without actually removing anything
    #[clap(long, short = 'n')]
    dry_run: bool,
}

impl Command {
    pub async fn exec(&self, github_token: Option<&str>) -> Result<()> {
        handle_cmd(
            ComponentCommands::Cleanup {
                all: self.all,
                days: self.days,
                dry_run: self.dry_run,
            },
            github_token,
        )
        .await
    }
}
