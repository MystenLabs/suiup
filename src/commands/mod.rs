// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

mod cleanup;
mod config;
mod default;
mod doctor;
mod install;
mod list;
mod remove;
mod self_;
mod show;
mod switch;
mod update;
mod which;

use crate::{
    handlers::config::ConfigHandler, handlers::self_::check_for_updates, types::BinaryVersion,
};

use anyhow::{anyhow, bail, Result};
use clap::{Parser, Subcommand, ValueEnum};
use comfy_table::Table;
pub const TABLE_FORMAT: &str = "  ── ══      ──    ";
#[derive(Parser)]
#[command(arg_required_else_help = true, disable_help_subcommand = true)]
#[command(version, about)]
pub struct Command {
    #[command(subcommand)]
    command: Commands,

    /// GitHub API token for authenticated requests (helps avoid rate limits).
    #[arg(long, env = "GITHUB_TOKEN", global = true)]
    pub github_token: Option<String>,

    /// Disable update warnings for suiup itself.
    #[arg(long, env = "SUIUP_DISABLE_UPDATE_WARNINGS", global = true)]
    pub disable_update_warnings: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    Cleanup(cleanup::Command),

    #[command(about = "Manage suiup configuration settings")]
    Config(config::Command),

    Default(default::Command),
    Doctor(doctor::Command),
    Install(install::Command),
    Remove(remove::Command),
    List(list::Command),

    #[command(name = "self")]
    Self_(self_::Command),

    Show(show::Command),
    Switch(switch::Command),
    Update(update::Command),
    Which(which::Command),
}

impl Command {
    pub async fn exec(&self) -> Result<()> {
        // Check for updates before executing any command (except self update to avoid recursion)
        // Priority: command line flag > config file setting
        let should_check_updates = if !matches!(self.command, Commands::Self_(_)) {
            if self.disable_update_warnings {
                false // Command line flag takes precedence
            } else {
                // Check config file setting
                match ConfigHandler::new() {
                    Ok(config_handler) => {
                        let config = config_handler.get_config();
                        !config.disable_update_warnings
                    }
                    Err(_) => true, // Default to enabled if config can't be loaded
                }
            }
        } else {
            false // Never check for updates when running self commands
        };

        if should_check_updates {
            check_for_updates();
        }

        // Resolve GitHub token: command line > config file > environment
        let github_token = if self.github_token.is_some() {
            self.github_token.clone()
        } else {
            match ConfigHandler::new() {
                Ok(config_handler) => {
                    let config = config_handler.get_config();
                    config.github_token.clone()
                }
                Err(_) => None, // Fallback to None if config can't be loaded
            }
        };

        match &self.command {
            Commands::Cleanup(cmd) => cmd.exec(&github_token).await,
            Commands::Config(cmd) => cmd.exec().await,
            Commands::Default(cmd) => cmd.exec(),
            Commands::Doctor(cmd) => cmd.exec(&github_token).await,
            Commands::Install(cmd) => cmd.exec(&github_token).await,
            Commands::Remove(cmd) => cmd.exec(&github_token).await,
            Commands::List(cmd) => cmd.exec(&github_token).await,
            Commands::Self_(cmd) => cmd.exec().await,
            Commands::Show(cmd) => cmd.exec(),
            Commands::Switch(cmd) => cmd.exec(),
            Commands::Update(cmd) => cmd.exec(&github_token).await,
            Commands::Which(cmd) => cmd.exec(),
        }
    }
}

#[derive(Subcommand)]
pub enum ComponentCommands {
    #[command(about = "Run diagnostic checks on the environment")]
    Doctor,
    #[command(about = "List available binaries to install")]
    List,
    #[command(about = "Add a binary")]
    Add {
        #[arg(
            num_args = 1..=2,
            help = "Binary to install with optional version (e.g. 'sui', 'sui@testnet-1.39.3', 'sui@testnet')"
        )]
        component: String,
        #[arg(
            long,
            help = "Whether to install the debug version of the binary (only available for sui). Default is false."
        )]
        debug: bool,
        #[arg(
            long,
            required = false,
            value_name = "branch",
            default_missing_value = "main",
            num_args = 0..=1,
            help = "Install from a branch in release mode. If none provided, main is used. Note that this requires Rust & cargo to be installed."
        )]
        nightly: Option<String>,
        #[arg(short, long, help = "Accept defaults without prompting")]
        yes: bool,
        #[arg(
            long,
            value_name = "path",
            help = "Custom installation path for the binary"
        )]
        path: Option<String>,
        #[arg(long, help = "Enable the tool after installation")]
        enable: bool,
        #[arg(
            long,
            conflicts_with = "enable",
            help = "Disable the tool after installation"
        )]
        disable: bool,
        #[arg(long, help = "Auto-detect and use existing version if none specified")]
        auto_detect: bool,
    },
    #[command(
        about = "Remove one. By default, the binary from each release will be removed. Use --version to specify which exact version to remove"
    )]
    Remove {
        #[arg(value_enum)]
        binary: BinaryName,
    },
    #[command(about = "Cleanup cache files")]
    Cleanup {
        /// Remove all cache files
        /// If not specified, only cache files older than `days` will be removed
        #[arg(long, conflicts_with = "days")]
        all: bool,
        /// Days to keep files in cache (default: 30)
        #[arg(long, short = 'd', default_value = "30")]
        days: u32,
        /// Show what would be removed without actually removing anything
        #[arg(long, short = 'n')]
        dry_run: bool,
        /// Show cache statistics only
        #[arg(long, short = 's')]
        stats: bool,
        /// Use smart cleanup strategy (removes oldest files first when size limit exceeded)
        #[arg(long)]
        smart: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Hash, Eq, ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum BinaryName {
    #[value(name = "mvr")]
    Mvr,
    #[value(name = "sui")]
    Sui,
    #[value(name = "walrus")]
    Walrus,
    #[value(name = "site-builder")]
    WalrusSites,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct CommandMetadata {
    pub name: BinaryName,
    pub network: String,
    pub version: Option<String>,
}

impl BinaryName {
    pub fn repo_url(&self) -> &str {
        match self {
            BinaryName::Mvr => "https://github.com/MystenLabs/mvr",
            BinaryName::Walrus => "https://github.com/MystenLabs/walrus",
            BinaryName::WalrusSites => "https://github.com/MystenLabs/walrus-sites",
            _ => "https://github.com/MystenLabs/sui",
        }
    }

    pub fn to_str(&self) -> &str {
        match self {
            BinaryName::Mvr => "mvr",
            BinaryName::Sui => "sui",
            BinaryName::Walrus => "walrus",
            BinaryName::WalrusSites => "site-builder",
        }
    }
}

impl std::fmt::Display for BinaryName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinaryName::Mvr => write!(f, "mvr"),
            BinaryName::Sui => write!(f, "sui"),
            BinaryName::Walrus => write!(f, "walrus"),
            BinaryName::WalrusSites => write!(f, "site-builder"),
        }
    }
}

impl std::str::FromStr for BinaryName {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "sui" => Ok(BinaryName::Sui),
            "mvr" => Ok(BinaryName::Mvr),
            "walrus" => Ok(BinaryName::Walrus),
            "site-builder" => Ok(BinaryName::WalrusSites),
            _ => Err(format!("Unknown binary: {}", s)),
        }
    }
}

pub fn parse_component_with_version(s: &str) -> Result<CommandMetadata, anyhow::Error> {
    let split_char = if s.contains("@") {
        "@"
    } else if s.contains("==") {
        "=="
    } else if s.contains("=") {
        "="
    } else {
        // TODO this is a hack because we don't have a better way to split
        " "
    };

    let parts: Vec<&str> = s.split(split_char).collect();

    match parts.len() {
        1 => {
            let component = BinaryName::from_str(parts[0], true)
                .map_err(|_| anyhow!("Invalid binary name: {}. Use `suiup list` to find available binaries to install.", parts[0]))?;
            let (network, version) = parse_version_spec(None)?;
            let component_metadata = CommandMetadata {
                name: component,
                network,
                version,
            };
            Ok(component_metadata)
        }
        2 => {
            let component = BinaryName::from_str(parts[0], true)
                .map_err(|_| anyhow!("Invalid binary name: {}. Use `suiup list` to find available binaries to install.", parts[0]))?;
            let (network, version) = parse_version_spec(Some(parts[1].to_string()))?;
            let component_metadata = CommandMetadata {
                name: component,
                network,
                version,
            };
            Ok(component_metadata)
        }
        _ => bail!("Invalid format. Use 'binary' or 'binary version'".to_string()),
    }
}

pub fn parse_version_spec(spec: Option<String>) -> Result<(String, Option<String>)> {
    match spec {
        None => Ok(("testnet".to_string(), None)),
        Some(spec) => {
            if spec.starts_with("testnet-")
                || spec.starts_with("devnet-")
                || spec.starts_with("mainnet-")
            {
                let parts: Vec<&str> = spec.splitn(2, '-').collect();
                Ok((parts[0].to_string(), Some(parts[1].to_string())))
            } else if spec == "testnet" || spec == "devnet" || spec == "mainnet" {
                Ok((spec, None))
            } else {
                // Assume it's a version for testnet
                Ok(("testnet".to_string(), Some(spec)))
            }
        }
    }
}

pub fn print_table(binaries: &Vec<BinaryVersion>) {
    let mut binaries_vec = binaries.clone();
    // sort by Binary column
    binaries_vec.sort_by_key(|b| b.binary_name.clone());
    let mut table = Table::new();
    table
        .load_preset(TABLE_FORMAT)
        .set_header(vec!["Binary", "Release/Branch", "Version", "Debug"])
        .add_rows(
            binaries_vec
                .into_iter()
                .map(|binary| {
                    vec![
                        binary.binary_name,
                        binary.network_release,
                        binary.version,
                        if binary.debug {
                            "Yes".to_string()
                        } else {
                            "No".to_string()
                        },
                    ]
                })
                .collect::<Vec<Vec<String>>>(),
        );
    println!("{table}");
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    #[test]
    fn verify_command() {
        super::Command::command().debug_assert();
    }
}
