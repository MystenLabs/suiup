// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, bail, Error};
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "suiup")]
#[command(about = "Sui Tooling Version Manager.")]
#[command(version = env!("CARGO_PKG_VERSION"))]
pub(crate) struct Suiup {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(
        long = "github-token",
        help = "GitHub API token for authenticated requests (helps avoid rate limits)",
        env = "GITHUB_TOKEN",
        global = true
    )]
    pub github_token: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(subcommand, about = "Get or set the default tool version")]
    Default(DefaultCommands),
    #[command(about = "Install a binary")]
    Install {
        #[arg(
            help = "Binary to install with optional version (e.g. 'sui', 'sui@1.40.1', 'sui@testnet', 'sui@testnet-1.39.3')"
        )]
        component: String,
        #[arg(
            long,
            required = false,
            value_name = "branch",
            default_missing_value = "main",
            num_args = 0..=1,
            help = "Install from a branch in release mode (use --debug for debug mode). \
            If none provided, main is used. Note that this requires Rust & cargo to be installed."
        )]
        nightly: Option<String>,
        #[arg(
            long,
            help = "This flag can be used in two ways: 1) to install the debug version of the \
            binary (only available for sui, default is false; 2) together with `--nightly` \
            to specify to install from branch in debug mode!"
        )]
        debug: bool,
        #[arg(short, long, help = "Accept defaults without prompting")]
        yes: bool,
    },
    #[command(about = "Remove one or more binaries")]
    Remove {
        #[arg(value_enum)]
        binary: BinaryName,
    },
    #[command(about = "List available binaries to install")]
    List,
    #[command(subcommand, about = "Commands for suiup itself", name = "self")]
    Self_(SelfCommands),
    #[command(about = "Show installed and active binaries")]
    Show,
    #[command(about = "Update binary")]
    Update {
        #[arg(
            help = "Binary to update (e.g. 'sui', 'mvr', 'walrus'). By default, it will update the default \
            binary version. For updating a specific release, use the `sui@testnet` form."
        )]
        name: String,
        #[arg(short, long, help = "Accept defaults without prompting")]
        yes: bool,
    },
    #[command(about = "Show the path where default binaries are installed")]
    Which,
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

#[derive(Debug, Subcommand)]
pub enum DefaultCommands {
    #[command(about = "Get the default Sui CLI version")]
    Get,
    #[command(about = "Set the default Sui CLI version")]
    Set {
        #[arg(
            help = "Binary to be set as default and the version (e.g. 'sui@testnet-1.39.3', 'sui@testnet' -- this will use an installed binary that has the highest testnet version)"
        )]
        name: String,
        #[arg(
            long,
            help = "Whether to set the debug version of the binary as default (only available for sui)."
        )]
        debug: bool,

        #[arg(
            long,
            required = false,
            value_name = "branch",
            default_missing_value = "main",
            num_args = 0..=1,
            help = "Use the nightly version by optionally specifying the branch name (uses main by default). Use `suiup show` to find all installed binaries"
        )]
        nightly: Option<String>,
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

#[derive(Debug, Subcommand)]
pub enum SelfCommands {
    #[command(about = "Update suiup itself")]
    Update,
    #[command(about = "Uninstall suiup")]
    Uninstall,
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

pub fn parse_version_spec(spec: Option<String>) -> Result<(String, Option<String>), Error> {
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
