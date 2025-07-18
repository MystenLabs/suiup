// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

mod default;
mod install;
mod list;
mod remove;
mod self_;
mod show;
mod switch;
mod update;
mod which;

use crate::types::BinaryVersion;
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
}

#[derive(Subcommand)]
pub enum Commands {
    Default(default::Command),
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
        match &self.command {
            Commands::Default(cmd) => cmd.exec(),
            Commands::Install(cmd) => cmd.exec(&self.github_token).await,
            Commands::Remove(cmd) => cmd.exec(&self.github_token).await,
            Commands::List(cmd) => cmd.exec(&self.github_token).await,
            Commands::Self_(cmd) => cmd.exec().await,
            Commands::Show(cmd) => cmd.exec(),
            Commands::Switch(cmd) => cmd.exec(),
            Commands::Update(cmd) => cmd.exec(&self.github_token).await,
            Commands::Which(cmd) => cmd.exec(),
        }
    }
}

#[derive(Subcommand)]
pub enum ComponentCommands {
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
    },
    #[command(
        about = "Remove one. By default, the binary from each release will be removed. Use --version to specify which exact version to remove"
    )]
    Remove {
        #[arg(value_enum)]
        binary: BinaryName,
    },
}

#[derive(Clone, Debug, PartialEq, Hash, Eq, ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum BinaryName {
    #[value(name = "sui")]
    Sui,
    #[value(name = "walrus")]
    Walrus,
    #[value(name = "mvr")]
    Mvr,
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
            _ => "https://github.com/MystenLabs/sui",
        }
    }

    pub fn to_str(&self) -> &str {
        match self {
            BinaryName::Sui => "sui",
            BinaryName::Walrus => "walrus",
            BinaryName::Mvr => "mvr",
        }
    }
}

impl std::fmt::Display for BinaryName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinaryName::Sui => write!(f, "sui"),
            BinaryName::Walrus => write!(f, "walrus"),
            BinaryName::Mvr => write!(f, "mvr"),
        }
    }
}

impl std::str::FromStr for BinaryName {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "sui" => Ok(BinaryName::Sui),
            "walrus" => Ok(BinaryName::Walrus),
            "mvr" => Ok(BinaryName::Mvr),
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
