// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use clap::{Args, Subcommand};

use crate::handlers::config::{ConfigHandler, ConfigValue};

#[derive(Args, Debug)]
#[command(about = "Manage suiup configuration settings")]
#[command(long_about = "Configure suiup behavior including mirror URLs, cache settings, default network, install paths, GitHub tokens, and update warnings.

Available configuration keys:
  mirror_url           - GitHub mirror URL for downloading releases (default: https://github.com)
  cache_days           - Number of days to keep cached files (default: 30)
  auto_cleanup         - Enable automatic cache cleanup (default: false)  
  max_cache_size       - Maximum cache size in bytes (default: 1073741824)
  default_network      - Default network for installations (default: testnet)
  install_path         - Custom installation path for binaries (default: system default)
  disable_update_warnings - Disable update notifications (default: false)
  github_token         - GitHub API token for authenticated requests (default: not set)

Examples:
  suiup config list                           # Show all configuration
  suiup config get mirror_url                # Get specific setting
  suiup config set mirror_url https://mirror.example.com
  suiup config set cache_days 7              # Set cache retention
  suiup config set auto_cleanup true         # Enable auto cleanup
  suiup config set disable_update_warnings true  # Disable update warnings
  suiup config set github_token ghp_xxxxxxxxxxxxxxxxxxxx  # Set GitHub token
  suiup config unset install_path            # Reset to default
  suiup config reset                         # Reset all to defaults")]
pub struct Command {
    #[command(subcommand)]
    command: ConfigCommands,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    #[command(about = "Get a configuration value")]
    #[command(long_about = "Display the current value of a configuration key.
    
Examples:
  suiup config get mirror_url
  suiup config get cache_days")]
    Get {
        #[arg(help = "Configuration key to get (e.g., mirror_url, cache_days, auto_cleanup)")]
        key: String,
    },
    
    #[command(about = "Set a configuration value")]
    #[command(long_about = "Set a configuration key to a new value.
    
Examples:
  suiup config set mirror_url https://mirror.example.com
  suiup config set cache_days 7
  suiup config set auto_cleanup true
  suiup config set default_network mainnet
  suiup config set disable_update_warnings true
  suiup config set github_token ghp_xxxxxxxxxxxxxxxxxxxx")]
    Set {
        #[arg(help = "Configuration key to set")]
        key: String,
        #[arg(help = "Configuration value to set")]
        value: String,
    },
    
    #[command(about = "Remove a configuration setting (reset to default)")]
    #[command(long_about = "Remove a configuration setting and revert it to its default value.
    
Examples:
  suiup config unset install_path    # Use system default path
  suiup config unset mirror_url      # Use default GitHub")]
    Unset {
        #[arg(help = "Configuration key to reset to default")]
        key: String,
    },
    
    #[command(about = "List all configuration values")]
    #[command(long_about = "Display all current configuration settings with their values.")]
    List,
    
    #[command(about = "Reset all configuration to defaults")]
    #[command(long_about = "Reset all configuration settings to their default values.
This will remove any custom settings you have configured.")]
    Reset {
        #[arg(long, help = "Reset without confirmation prompt")]
        yes: bool,
    },
    
    #[command(about = "Validate current configuration")]
    #[command(long_about = "Check if all configuration values are valid and properly formatted.
This will verify URLs, file paths, and value ranges.")]
    Validate,
}

impl Command {
    pub async fn exec(&self) -> Result<()> {
        let mut handler = ConfigHandler::new()?;
        match &self.command {
            ConfigCommands::Get { key } => handler.get(key).await,
            ConfigCommands::Set { key, value } => {
                let config_value = ConfigValue::from_string(key, value)?;
                handler.set(key, config_value).await
            },
            ConfigCommands::List => handler.list().await,
            ConfigCommands::Unset { key } => handler.unset(key).await,
            ConfigCommands::Reset { yes } => handler.reset(*yes).await,
            ConfigCommands::Validate => handler.validate().await,
        }
    }
}
