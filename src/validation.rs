// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{bail, Result};
use lazy_static::lazy_static;
use regex::Regex;
use std::path::Path;

trait Enumerable {
    fn as_str(&self) -> &'static str;
    fn all() -> Vec<Self>
    where
        Self: Sized;
    fn try_from_str(s: &str) -> Result<Self>
    where
        Self: Sized;
}

// Network
enum Network {
    Testnet,
    Devnet,
    Mainnet,
}

impl Enumerable for Network {
    fn as_str(&self) -> &'static str {
        match self {
            Network::Testnet => "testnet",
            Network::Devnet => "devnet",
            Network::Mainnet => "mainnet",
        }
    }
    fn all() -> Vec<Network> {
        vec![Network::Testnet, Network::Devnet, Network::Mainnet]
    }

    fn try_from_str(s: &str) -> Result<Self> {
        match s {
            "testnet" => Ok(Network::Testnet),
            "devnet" => Ok(Network::Devnet),
            "mainnet" => Ok(Network::Mainnet),
            _ => bail!(
                "Invalid network: '{}'. Valid networks are: {}",
                s,
                Self::all()
                    .into_iter()
                    .map(|n| n.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

// Binary
enum Binary {
    Sui,
    Mvr,
    Walrus,
    SiteBuilder,
}

impl Enumerable for Binary {
    fn as_str(&self) -> &'static str {
        match self {
            Binary::Sui => "sui",
            Binary::Mvr => "mvr",
            Binary::Walrus => "walrus",
            Binary::SiteBuilder => "site-builder",
        }
    }

    fn all() -> Vec<Binary> {
        vec![
            Binary::Sui,
            Binary::Mvr,
            Binary::Walrus,
            Binary::SiteBuilder,
        ]
    }
    fn try_from_str(s: &str) -> Result<Self> {
        match s {
            "sui" => Ok(Binary::Sui),
            "mvr" => Ok(Binary::Mvr),
            "walrus" => Ok(Binary::Walrus),
            "site-builder" => Ok(Binary::SiteBuilder),
            _ => bail!(
                "Invalid binary: '{}'. Valid binaries are: {}",
                s,
                Self::all()
                    .into_iter()
                    .map(|b| b.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

lazy_static! {
    static ref VERSION_REGEXS:Vec<Regex> = vec![
        Regex::new(r"^(testnet|devnet|mainnet)-\d+\.\d+\.\d+(-[a-zA-Z0-9]+(\.\d+)?)?$").unwrap(), // network-version
        Regex::new(r"^\d+\.\d+\.\d+(-[a-zA-Z0-9]+(\.\d+)?)?$").unwrap(),                        // semver
        Regex::new(r"^(latest|nightly)$").unwrap(),                                               // special
        Regex::new(r"^[a-f0-9]{7,40}$").unwrap(),                                                // git hash
    ];
}

pub struct Validator;

impl Validator {
    pub fn validate_version_format(version: &str) -> Result<()> {
        if version.is_empty() {
            bail!("Version cannot be empty");
        }

        for regex in VERSION_REGEXS.iter() {
            if regex.is_match(version) {
                return Ok(());
            }
        }

        bail!(
            "Invalid version format: '{}'. Expected formats:\n\
             - Semantic version: 1.2.3, 1.2.3-alpha\n\
             - Network version: testnet-1.2.3, devnet-1.2.3\n\
             - Special: latest, nightly\n\
             - Git hash: a1b2c3d",
            version
        );
    }

    pub fn validate_network(network: &str) -> Result<()> {
        Network::try_from_str(network)?;
        Ok(())
    }

    pub fn validate_binary_name(binary: &str) -> Result<()> {
        Binary::try_from_str(binary)?;
        Ok(())
    }

    pub fn validate_path_exists(path: &str) -> Result<()> {
        let path = Path::new(path);

        if !path.exists() {
            bail!("Path does not exist: {}", path.display());
        }

        Ok(())
    }

    pub fn validate_path_writable(path: &str) -> Result<()> {
        let path = Path::new(path);

        if let Some(parent) = path.parent() {
            if !parent.exists() {
                bail!("Parent directory does not exist: {}", parent.display());
            }

            // Try to create a temporary file to test writability
            let temp_file = parent.join(".suiup_write_test");
            match std::fs::write(&temp_file, "") {
                Ok(_) => {
                    let _ = std::fs::remove_file(&temp_file);
                    Ok(())
                }
                Err(_) => bail!("Directory is not writable: {}", parent.display()),
            }
        } else {
            bail!("Invalid path: {}", path.display());
        }
    }

    pub fn validate_number_range(value: u64, min: u64, max: u64, field_name: &str) -> Result<()> {
        if value < min || value > max {
            bail!(
                "{} must be between {} and {} (got: {})",
                field_name,
                min,
                max,
                value
            );
        }
        Ok(())
    }

    pub fn validate_cache_size(size_bytes: u64) -> Result<()> {
        const MIN_SIZE: u64 = 100 * 1024 * 1024; // 100MB
        const MAX_SIZE: u64 = 100 * 1024 * 1024 * 1024; // 100GB

        Self::validate_number_range(size_bytes, MIN_SIZE, MAX_SIZE, "Cache size")
    }

    pub fn validate_cache_days(days: u32) -> Result<()> {
        const MIN_DAYS: u64 = 1;
        const MAX_DAYS: u64 = 365;

        Self::validate_number_range(days as u64, MIN_DAYS, MAX_DAYS, "Cache days")
    }
}
