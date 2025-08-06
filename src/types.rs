// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Error};
use std::io::Write;
use std::{
    collections::BTreeMap,
    fmt::{self, Display, Formatter},
    path::PathBuf,
    str::FromStr,
};

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::paths::{default_file_path, installed_binaries_file};

pub type Version = String;

#[derive(Debug)]
pub enum Repo {
    Sui,
    Mvr,
    Walrus,
    WalrusSites,
    LedgerSigner,
}

impl Repo {
    /// Returns the binary name for this repository
    pub fn binary_name(&self) -> &'static str {
        match self {
            Repo::Mvr => "mvr",
            Repo::Sui => "sui",
            Repo::Walrus => "walrus",
            Repo::WalrusSites => "site-builder",
            Repo::LedgerSigner => "sui-ledger-signer",
        }
    }
}

impl Display for Repo {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mvr => write!(f, "MystenLabs/mvr"),
            Self::Sui => write!(f, "MystenLabs/sui"),
            Self::Walrus => write!(f, "MystenLabs/walrus"),
            Self::WalrusSites => write!(f, "MystenLabs/walrus-sites"),
            Self::LedgerSigner => write!(f, "MystenLabs/ledger-signer-rust"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Release {
    pub assets: Vec<Asset>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Asset {
    pub browser_download_url: String,
    pub name: String,
}

pub struct Binaries {
    pub binaries: Vec<BinaryVersion>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DefaultBinaries {
    pub binaries: Vec<BinaryVersion>,
}

/// Struct to store the installed binaries
#[derive(Serialize, Deserialize, Debug)]
pub struct InstalledBinaries {
    binaries: Vec<BinaryVersion>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct BinaryVersion {
    /// The name of the Sui tool binary
    pub binary_name: String,
    /// The network release of the binary
    pub network_release: String,
    /// The version of the binary in the corresponding release
    pub version: String,
    /// Debug build of the binary
    pub debug: bool,
    /// Path to the binary
    pub path: Option<String>,
}

#[derive(
    Copy, Deserialize, Serialize, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, ValueEnum,
)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    #[serde(alias = "testnet")]
    Testnet,
    #[serde(alias = "devnet")]
    Devnet,
    #[serde(alias = "mainnet")]
    Mainnet,
}

impl InstalledBinaries {
    pub fn create_file(path: &PathBuf) -> Result<(), Error> {
        let binaries = InstalledBinaries { binaries: vec![] };
        let s = serde_json::to_string_pretty(&binaries)
            .map_err(|e| anyhow!("Cannot serialize the installed binaries to file: {e}"))?;
        let mut file = std::fs::File::create(path)
            .map_err(|e| anyhow!("Cannot create this file {}: {e}", path.display()))?;
        file.write_all(s.as_bytes())
            .map_err(|e| anyhow!("Cannot write to {}: {e}", path.display()))?;
        Ok(())
    }

    pub fn new() -> Result<Self, Error> {
        Self::read_from_file()
    }

    /// Save the installed binaries data to the installed binaries JSON file
    pub fn save_to_file(&self) -> Result<(), Error> {
        let s = serde_json::to_string_pretty(self)
            .map_err(|e| anyhow!("Cannot read the installed binaries file: {e}"))?;
        std::fs::write(installed_binaries_file()?, s)
            .map_err(|e| anyhow!("Cannot serialize the installed binaries to file: {e}"))?;
        Ok(())
    }

    /// Read the installed binaries JSON file
    pub fn read_from_file() -> Result<Self, Error> {
        let s = std::fs::read_to_string(installed_binaries_file()?)
            .map_err(|e| anyhow!("Cannot read from the installed binaries file: {e}"))?;
        let binaries: InstalledBinaries = serde_json::from_str(&s)
            .map_err(|e| anyhow!("Cannot deserialize from installed binaries file: {e}"))?;
        Ok(binaries)
    }

    /// Add a binary to the installed binaries JSON file
    pub fn add_binary(&mut self, binary: BinaryVersion) {
        if !self.binaries.iter().any(|b| b == &binary) {
            self.binaries.push(binary);
        }
    }

    /// Remove a binary from the installed binaries JSON file
    pub fn remove_binary(&mut self, binary: &str) {
        self.binaries.retain(|b| b.binary_name != binary);
    }

    /// List the binaries in the installed binaries JSON file
    pub fn binaries(&self) -> &[BinaryVersion] {
        &self.binaries
    }
}

impl DefaultBinaries {
    pub fn _load() -> Result<DefaultBinaries, Error> {
        let default_file_path = default_file_path()?;
        let file_content = std::fs::read_to_string(default_file_path)?;
        let default_binaries: DefaultBinaries =
            serde_json::from_str(&file_content).expect("Cannot deserialize default binaries file");

        Ok(default_binaries)
    }
}

impl Display for Binaries {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let mut s: BTreeMap<String, Vec<(String, String, bool)>> = BTreeMap::new();

        for b in self.binaries.clone() {
            if let Some(binaries) = s.get_mut(&b.network_release) {
                binaries.push((b.binary_name, b.version, b.debug));
            } else {
                s.insert(b.network_release, vec![(b.binary_name, b.version, b.debug)]);
            }
        }

        for (network, binaries) in s {
            writeln!(f, "[{network} release/branch]")?;
            for (binary, version, debug) in binaries {
                if binary == "sui" && debug {
                    writeln!(f, "    {binary}-{version} (debug build)")?;
                } else {
                    writeln!(f, "    {binary}-{version}")?;
                }
            }
        }
        Ok(())
    }
}

impl Display for BinaryVersion {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        if self.debug {
            write!(f, "{}-{} (debug build)", self.binary_name, self.version)
        } else {
            write!(f, "{}-{}", self.binary_name, self.version)
        }
    }
}

impl Display for Network {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Network::Testnet => write!(f, "testnet"),
            Network::Devnet => write!(f, "devnet"),
            Network::Mainnet => write!(f, "mainnet"),
        }
    }
}

impl From<BTreeMap<String, (String, Version, bool)>> for Binaries {
    fn from(map: BTreeMap<String, (String, Version, bool)>) -> Self {
        let binaries = map
            .iter()
            .map(|(k, v)| BinaryVersion {
                binary_name: k.to_string(),
                network_release: v.0.clone(),
                version: v.1.to_string(),
                debug: v.2,
                path: None,
            })
            .collect();
        Binaries { binaries }
    }
}

impl FromStr for Network {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "testnet" => Ok(Network::Testnet),
            "devnet" => Ok(Network::Devnet),
            "mainnet" => Ok(Network::Mainnet),
            _ => Err(anyhow!("Invalid network")),
        }
    }
}
