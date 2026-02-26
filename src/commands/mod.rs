// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

mod cleanup;
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

pub use crate::registry::BinaryName;
use crate::{handlers::self_::check_for_updates, types::BinaryVersion};

use anyhow::{Result, anyhow, bail};
use clap::{Parser, Subcommand};
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
    Cleanup(cleanup::Command),
}

impl Command {
    fn normalized_github_token(&self) -> Option<String> {
        self.github_token
            .as_deref()
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(ToOwned::to_owned)
    }

    pub async fn exec(&self) -> Result<()> {
        // Check for updates before executing any command (except self update to avoid recursion)
        if !matches!(self.command, Commands::Self_(_)) && !self.disable_update_warnings {
            check_for_updates();
        }

        let github_token = self.normalized_github_token();
        let github_token_ref = github_token.as_deref();

        match &self.command {
            Commands::Default(cmd) => cmd.exec(),
            Commands::Doctor(cmd) => cmd.exec(github_token_ref).await,
            Commands::Install(cmd) => cmd.exec(github_token_ref).await,
            Commands::Remove(cmd) => cmd.exec(github_token_ref).await,
            Commands::List(cmd) => cmd.exec(github_token_ref).await,
            Commands::Self_(cmd) => cmd.exec().await,
            Commands::Show(cmd) => cmd.exec(),
            Commands::Switch(cmd) => cmd.exec(),
            Commands::Update(cmd) => cmd.exec(github_token_ref).await,
            Commands::Which(cmd) => cmd.exec(),
            Commands::Cleanup(cmd) => cmd.exec(github_token_ref).await,
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
    },
    #[command(
        about = "Remove one. By default, the binary from each release will be removed. Use --version to specify which exact version to remove"
    )]
    Remove { binary: String },
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
    },
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct CommandMetadata {
    pub name: BinaryName,
    pub network: String,
    pub version: Option<String>,
}

fn parse_binary_name(name: &str) -> Result<BinaryName> {
    BinaryName::new(name).map_err(|_| {
        anyhow!(
            "Invalid binary name: {name}. Use `suiup list` to find available binaries to install or `suiup show` to see which binaries are already installed.\nWhen specifying versions, use `@`, e.g.: sui@v1.60.0\n\nMore information in the docs: https://github.com/mystenLabs/suiup?tab=readme-ov-file#switch-between-versions-note-that-default-set-requires-to-specify-a-version"
        )
    })
}

fn split_component_spec(s: &str) -> (&str, Option<&str>) {
    for delimiter in ["@", "==", "="] {
        if let Some((name, spec)) = s.split_once(delimiter) {
            return (name, Some(spec));
        }
    }

    if let Some((name, spec)) = s.split_once(' ') {
        return (name, Some(spec));
    }

    (s, None)
}

pub fn parse_component_with_version(s: &str) -> Result<CommandMetadata, anyhow::Error> {
    let (name, version_spec) = split_component_spec(s);
    let component = parse_binary_name(name)?;

    if let Some(spec) = version_spec
        && spec.is_empty()
    {
        bail!("Version cannot be empty. Use 'binary' or 'binary@version' (e.g., sui@v1.60.0)");
    }

    let (network, version) = parse_version_spec(version_spec)?;
    Ok(CommandMetadata {
        name: component,
        network,
        version,
    })
}

pub fn parse_version_spec(spec: Option<&str>) -> Result<(String, Option<String>)> {
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
                Ok((spec.to_string(), None))
            } else {
                // Validate that it looks like a version (starts with 'v' + digit or digit, and contains a dot)
                let starts_valid = spec.chars().next().is_some_and(|c| {
                    c.is_ascii_digit()
                        || (c == 'v' && spec.chars().nth(1).is_some_and(|c2| c2.is_ascii_digit()))
                });
                let has_dot = spec.contains('.');
                if !starts_valid || !has_dot {
                    bail!(
                        "Invalid version format: '{spec}'. Expected a version like 'v1.60.0' or '1.60.0', or when applicable, 'testnet', 'devnet', 'mainnet'.",
                    );
                }
                Ok(("testnet".to_string(), Some(spec.to_string())))
            }
        }
    }
}

pub fn print_table(binaries: &[BinaryVersion]) {
    let mut binaries_vec = binaries.to_owned();
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
    use clap::Parser;

    #[test]
    fn verify_command() {
        super::Command::command().debug_assert();
    }

    #[test]
    fn normalize_empty_github_token_to_none() {
        let cmd = super::Command::parse_from(["suiup", "--github-token", "", "list"]);
        assert_eq!(cmd.normalized_github_token(), None);
    }

    #[test]
    fn preserve_non_empty_github_token() {
        let cmd = super::Command::parse_from(["suiup", "--github-token", "abc123", "list"]);
        assert_eq!(cmd.normalized_github_token(), Some("abc123".to_string()));
    }
}
