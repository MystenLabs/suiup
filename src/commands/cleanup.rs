use anyhow::Result;
use clap::Args;

use crate::handle_commands::handle_cmd;
use crate::handlers::config::ConfigHandler;

use super::ComponentCommands;

/// Remove old release archives from the cache directory.
#[derive(Args, Debug)]
pub struct Command {
    /// Days to keep files in cache (overrides config setting)
    #[clap(long, short = 'd')]
    days: Option<u32>,

    /// Remove all cache files
    #[clap(long, conflicts_with = "days")]
    all: bool,

    /// Show what would be removed without actually removing anything
    #[clap(long, short = 'n')]
    dry_run: bool,

    /// Show cache statistics only
    #[clap(long, short = 's')]
    stats: bool,

    /// Use smart cleanup strategy (removes oldest files first when size limit exceeded)
    #[clap(long)]
    smart: bool,
}

impl Command {
    pub async fn exec(&self, github_token: &Option<String>) -> Result<()> {
        // Resolve days setting: command line > config file > default
        let days = if let Some(cmd_days) = self.days {
            cmd_days
        } else {
            match ConfigHandler::new() {
                Ok(config_handler) => {
                    let config = config_handler.get_config();
                    config.cache_days
                }
                Err(_) => 30, // Fallback to default if config can't be loaded
            }
        };

        handle_cmd(
            ComponentCommands::Cleanup {
                all: self.all,
                days,
                dry_run: self.dry_run,
                stats: self.stats,
                smart: self.smart,
            },
            github_token.to_owned(),
        )
        .await
    }
}
