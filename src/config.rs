// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Result};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryConfig {
    pub name: String,
    pub binary_name: String,
    pub repository: String,
    pub description: String,
    pub supported_networks: Vec<String>,
    pub default_network: String,
    pub supports_debug: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinariesConfig {
    pub binaries: Vec<BinaryConfig>,
}

impl BinariesConfig {
    pub fn get_binary(&self, name: &str) -> Option<&BinaryConfig> {
        self.binaries.iter().find(|b| b.name == name || b.binary_name == name)
    }

    pub fn get_binary_by_repo(&self, repo: &str) -> Option<&BinaryConfig> {
        self.binaries.iter().find(|b| b.repository == repo)
    }

    pub fn available_components(&self) -> Vec<&str> {
        self.binaries.iter().map(|b| b.name.as_str()).collect()
    }

    pub fn is_network_supported(&self, binary: &str, network: &str) -> bool {
        if let Some(config) = self.get_binary(binary) {
            if config.supported_networks.is_empty() {
                // No network restrictions (like MVR)
                true
            } else {
                config.supported_networks.contains(&network.to_string())
            }
        } else {
            false
        }
    }

    pub fn get_default_network(&self, binary: &str) -> Option<String> {
        self.get_binary(binary).map(|c| c.default_network.clone())
    }

    pub fn supports_debug(&self, binary: &str) -> bool {
        self.get_binary(binary).map(|c| c.supports_debug).unwrap_or(false)
    }

    pub fn get_repo_url(&self, binary: &str) -> Option<String> {
        self.get_binary(binary).map(|c| format!("https://github.com/{}", c.repository))
    }
}

lazy_static! {
    static ref CONFIG: RwLock<Option<BinariesConfig>> = RwLock::new(None);
}

/// Get the path to the configuration file
fn get_config_path() -> PathBuf {
    use crate::paths::get_config_home;
    let mut config_path = get_config_home();
    config_path.push("suiup");
    config_path.push("binaries.json");
    config_path
}

pub fn load_config() -> Result<()> {
    let config_path = get_config_path();
    
    // Try to load from user config directory first
    let config: BinariesConfig = if config_path.exists() {
        let config_str = fs::read_to_string(&config_path)
            .map_err(|e| anyhow!("Failed to read config file at {:?}: {}", config_path, e))?;
        serde_json::from_str(&config_str)
            .map_err(|e| anyhow!("Failed to parse config file at {:?}: {}", config_path, e))?
    } else {
        // Fall back to embedded config
        let embedded_config = include_str!("../binaries.json");
        let config: BinariesConfig = serde_json::from_str(embedded_config)
            .map_err(|e| anyhow!("Failed to parse embedded binaries.json: {}", e))?;
        
        // Try to create the config file for future use
        if let Some(parent) = config_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).ok();
            }
            // Write the embedded config to the user's config directory
            fs::write(&config_path, embedded_config).ok();
        }
        
        config
    };
    
    let mut global_config = CONFIG.write()
        .map_err(|_| anyhow!("Failed to acquire write lock on configuration"))?;
    *global_config = Some(config);
    Ok(())
}

pub fn get_config() -> Result<BinariesConfig> {
    let config = CONFIG.read()
        .map_err(|_| anyhow!("Failed to acquire read lock on configuration"))?;
    
    config.clone().ok_or_else(|| anyhow!("Configuration not loaded. Call load_config() first."))
}

pub fn with_config<T, F>(f: F) -> Result<T>
where
    F: FnOnce(&BinariesConfig) -> Result<T>,
{
    let config = get_config()?;
    f(&config)
}

// Helper function to get binary config by name
pub fn get_binary_config(name: &str) -> Result<BinaryConfig> {
    let config = get_config()?;
    config.get_binary(name)
        .cloned()
        .ok_or_else(|| anyhow!("Binary '{}' not found in configuration", name))
}

// Backward compatibility helper
pub fn available_components() -> Vec<String> {
    get_config()
        .map(|c| c.binaries.iter().map(|b| b.name.clone()).collect())
        .unwrap_or_else(|_| vec!["sui".to_string(), "mvr".to_string(), "walrus".to_string(), "site-builder".to_string()])
}

/// Reload configuration from disk
pub fn reload_config() -> Result<()> {
    load_config()
}

/// Get the configuration file path for display purposes
pub fn get_config_file_path() -> PathBuf {
    get_config_path()
}