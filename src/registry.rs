// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Result, anyhow};
use serde::Deserialize;
use std::sync::OnceLock;

include!(concat!(env!("OUT_DIR"), "/binary_configs.rs"));

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InstallationType {
    Archive,
    Standalone,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BinaryConfig {
    pub name: String,
    pub description: String,
    pub repository: String,
    #[serde(default = "default_main_branch")]
    pub main_branch: String,
    pub installation_type: InstallationType,
    #[serde(default)]
    pub network_based: bool,
    #[serde(default)]
    pub supported_networks: Vec<String>,
    #[serde(default = "default_network")]
    pub default_network: String,
    #[serde(default)]
    pub supports_debug: bool,
    pub cargo_package: Option<String>,
    pub nightly_toolchain: Option<String>,
    #[serde(default)]
    pub shared_repo_binary: bool,
}

fn default_main_branch() -> String {
    "main".to_string()
}

fn default_network() -> String {
    "testnet".to_string()
}

fn normalize_optional_string(value: &mut Option<String>) {
    if value.as_ref().is_some_and(|v| v.trim().is_empty()) {
        *value = None;
    }
}

impl BinaryConfig {
    pub fn repo_url(&self) -> String {
        format!("https://github.com/{}", self.repository)
    }
}

pub struct BinaryRegistry {
    configs: Vec<BinaryConfig>,
}

static REGISTRY: OnceLock<BinaryRegistry> = OnceLock::new();

impl BinaryRegistry {
    pub fn global() -> &'static BinaryRegistry {
        REGISTRY.get_or_init(|| {
            let mut configs = Vec::new();
            for toml_str in BINARY_CONFIGS {
                let mut config: BinaryConfig =
                    toml::from_str(toml_str).expect("Failed to parse embedded binary TOML config");
                normalize_optional_string(&mut config.cargo_package);
                normalize_optional_string(&mut config.nightly_toolchain);
                configs.push(config);
            }
            configs.sort_by(|a, b| a.name.cmp(&b.name));
            BinaryRegistry { configs }
        })
    }

    pub fn get(&self, name: &str) -> Option<&BinaryConfig> {
        self.configs.iter().find(|c| c.name == name)
    }

    pub fn all(&self) -> &[BinaryConfig] {
        &self.configs
    }

    pub fn all_names(&self) -> Vec<&str> {
        self.configs.iter().map(|c| c.name.as_str()).collect()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.configs.iter().any(|c| c.name == name)
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct BinaryName(String);

impl BinaryName {
    pub fn new(name: &str) -> Result<Self> {
        if BinaryRegistry::global().contains(name) {
            Ok(BinaryName(name.to_string()))
        } else {
            Err(anyhow!("Unknown binary: {}", name))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn config(&self) -> &BinaryConfig {
        BinaryRegistry::global()
            .get(&self.0)
            .expect("BinaryName should always have a valid config")
    }

    pub fn repo_url(&self) -> String {
        self.config().repo_url()
    }
}

impl std::fmt::Display for BinaryName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for BinaryName {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        BinaryName::new(s).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_toml_files_parse() {
        let registry = BinaryRegistry::global();
        assert!(
            !registry.all().is_empty(),
            "Registry should have at least one binary"
        );
    }

    #[test]
    fn all_tomls_include_all_fields() {
        let required_fields = [
            "name",
            "description",
            "repository",
            "main_branch",
            "installation_type",
            "network_based",
            "supported_networks",
            "default_network",
            "supports_debug",
            "cargo_package",
            "nightly_toolchain",
            "shared_repo_binary",
        ];

        for toml_str in BINARY_CONFIGS {
            for field in &required_fields {
                assert!(
                    toml_str.contains(&format!("{field} =")),
                    "TOML config is missing required field '{field}'\nConfig:\n{toml_str}"
                );
            }
        }
    }

    #[test]
    fn known_binaries_present() {
        let registry = BinaryRegistry::global();
        for name in &[
            "sui",
            "mvr",
            "walrus",
            "site-builder",
            "move-analyzer",
            "ledger-signer",
            "yubikey-signer",
        ] {
            assert!(
                registry.contains(name),
                "Registry should contain '{}'",
                name
            );
        }
    }

    #[test]
    fn sui_config_values() {
        let config = BinaryRegistry::global().get("sui").unwrap();
        assert_eq!(config.repository, "MystenLabs/sui");
        assert_eq!(config.installation_type, InstallationType::Archive);
        assert!(config.network_based);
        assert!(config.supports_debug);
        assert_eq!(config.default_network, "testnet");
        assert!(config.supported_networks.contains(&"testnet".to_string()));
        assert!(config.supported_networks.contains(&"devnet".to_string()));
        assert!(config.supported_networks.contains(&"mainnet".to_string()));
    }

    #[test]
    fn mvr_config_values() {
        let config = BinaryRegistry::global().get("mvr").unwrap();
        assert_eq!(config.repository, "MystenLabs/mvr");
        assert_eq!(config.installation_type, InstallationType::Standalone);
        assert!(!config.network_based);
    }

    #[test]
    fn ledger_signer_config_values() {
        let config = BinaryRegistry::global().get("ledger-signer").unwrap();
        assert_eq!(config.repository, "MystenLabs/rust-signers");
        assert_eq!(config.nightly_toolchain.as_deref(), Some("nightly"));
        assert!(config.shared_repo_binary);
    }

    #[test]
    fn walrus_config_values() {
        let config = BinaryRegistry::global().get("walrus").unwrap();
        assert_eq!(config.cargo_package.as_deref(), Some("walrus-service"));
        assert!(config.network_based);
    }

    #[test]
    fn site_builder_config_values() {
        let config = BinaryRegistry::global().get("site-builder").unwrap();
        assert_eq!(config.supported_networks, vec!["mainnet"]);
        assert_eq!(config.default_network, "mainnet");
    }

    #[test]
    fn invalid_binary_name_rejected() {
        assert!(BinaryName::new("nonexistent").is_err());
    }

    #[test]
    fn valid_binary_name_accepted() {
        assert!(BinaryName::new("sui").is_ok());
        assert_eq!(BinaryName::new("sui").unwrap().as_str(), "sui");
    }

    #[test]
    fn binary_name_display() {
        let name = BinaryName::new("sui").unwrap();
        assert_eq!(format!("{}", name), "sui");
    }

    #[test]
    fn all_names_returns_sorted() {
        let names = BinaryRegistry::global().all_names();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }
}
