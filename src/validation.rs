// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{bail, Result};
use regex::Regex;
use std::path::Path;

pub struct Validator;

impl Validator {
    pub fn validate_version_format(version: &str) -> Result<()> {
        if version.is_empty() {
            bail!("Version cannot be empty");
        }

        // Allow common version formats:
        // - Semantic versioning: 1.2.3, 1.2.3-alpha, 1.2.3-beta.1
        // - Network prefixed: testnet-1.2.3, devnet-1.2.3, mainnet-1.2.3
        // - Special versions: latest, nightly
        let version_patterns = [
            r"^(testnet|devnet|mainnet)-\d+\.\d+\.\d+(-[a-zA-Z0-9]+(\.\d+)?)?$", // network-version
            r"^\d+\.\d+\.\d+(-[a-zA-Z0-9]+(\.\d+)?)?$",                        // semver
            r"^(latest|nightly)$",                                               // special
            r"^[a-f0-9]{7,40}$",                                                // git hash
        ];

        for pattern in &version_patterns {
            let regex = Regex::new(pattern).unwrap();
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
        let valid_networks = ["testnet", "devnet", "mainnet"];
        
        if !valid_networks.contains(&network) {
            bail!(
                "Invalid network: '{}'. Valid networks are: {}",
                network,
                valid_networks.join(", ")
            );
        }
        
        Ok(())
    }

    pub fn validate_binary_name(binary: &str) -> Result<()> {
        let valid_binaries = ["sui", "mvr", "walrus", "site-builder"];
        
        if !valid_binaries.contains(&binary) {
            bail!(
                "Invalid binary: '{}'. Valid binaries are: {}",
                binary,
                valid_binaries.join(", ")
            );
        }
        
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

    pub fn validate_url(url: &str) -> Result<()> {
        match url::Url::parse(url) {
            Ok(parsed_url) => {
                if !["http", "https"].contains(&parsed_url.scheme()) {
                    bail!("URL must use http or https scheme");
                }
                
                if parsed_url.host().is_none() {
                    bail!("URL must have a valid host");
                }
                
                Ok(())
            }
            Err(_) => bail!("Invalid URL format: {}", url),
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