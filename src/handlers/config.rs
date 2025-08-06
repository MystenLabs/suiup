// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, bail, Result};
use colored::Colorize;
use console::Term;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;

use crate::error::{print_success, suggest_fix, ErrorContext};
use crate::paths::config_file_path;
use crate::validation::Validator;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiupConfig {
    #[serde(default = "default_mirror_url")]
    pub mirror_url: String,
    #[serde(default = "default_cache_days")]
    pub cache_days: u32,
    #[serde(default = "default_auto_cleanup")]
    pub auto_cleanup: bool,
    #[serde(default = "default_max_cache_size")]
    pub max_cache_size: u64,
    #[serde(default = "default_default_network")]
    pub default_network: String,
    #[serde(default = "default_install_path")]
    pub install_path: Option<String>,
    #[serde(default = "default_disable_update_warnings")]
    pub disable_update_warnings: bool,
    #[serde(default = "default_github_token")]
    pub github_token: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ConfigValue {
    String(String),
    Number(u64),
    Boolean(bool),
}

impl ConfigValue {
    pub fn from_string(key: &str, value: &str) -> Result<Self> {
        match key {
            "mirror_url" | "default_network" | "install_path" | "github_token" => {
                Ok(ConfigValue::String(value.to_string()))
            }
            "cache_days" | "max_cache_size" => {
                let num = value
                    .parse::<u64>()
                    .map_err(|_| anyhow!("Invalid number value for {}: {}", key, value))?;
                Ok(ConfigValue::Number(num))
            }
            "auto_cleanup" | "disable_update_warnings" => {
                let bool_val = value.parse::<bool>().map_err(|_| {
                    anyhow!(
                        "Invalid boolean value for {}: {}. Use 'true' or 'false'",
                        key,
                        value
                    )
                })?;
                Ok(ConfigValue::Boolean(bool_val))
            }
            _ => bail!("Unknown configuration key: {}", key),
        }
    }
}

fn default_mirror_url() -> String {
    "https://github.com".to_string()
}

fn default_cache_days() -> u32 {
    30
}

fn default_auto_cleanup() -> bool {
    false
}

fn default_max_cache_size() -> u64 {
    1024 * 1024 * 1024 // 1GB
}

fn default_default_network() -> String {
    "testnet".to_string()
}

fn default_install_path() -> Option<String> {
    Some(
        crate::paths::get_default_bin_dir()
            .to_string_lossy()
            .to_string(),
    )
}

fn default_disable_update_warnings() -> bool {
    false
}

fn default_github_token() -> Option<String> {
    None
}

impl Default for SuiupConfig {
    fn default() -> Self {
        Self {
            mirror_url: default_mirror_url(),
            cache_days: default_cache_days(),
            auto_cleanup: default_auto_cleanup(),
            max_cache_size: default_max_cache_size(),
            default_network: default_default_network(),
            install_path: default_install_path(),
            disable_update_warnings: default_disable_update_warnings(),
            github_token: default_github_token(),
        }
    }
}

pub struct ConfigHandler {
    config: SuiupConfig,
}

impl ConfigHandler {
    pub fn new() -> Result<Self> {
        let config = Self::load_config()?;
        Ok(Self { config })
    }

    fn load_config() -> Result<SuiupConfig> {
        let config_path = config_file_path()?;

        if !config_path.exists() {
            let default_config = SuiupConfig::default();
            Self::save_config(&default_config)
                .with_config_context("Failed to create default configuration file")?;
            return Ok(default_config);
        }

        let content = fs::read_to_string(&config_path)
            .with_config_context("Failed to read configuration file")?;

        let config: SuiupConfig = serde_json::from_str(&content)
            .with_config_context("Configuration file contains invalid JSON. Try 'suiup config reset' to restore defaults")?;

        Ok(config)
    }

    fn save_config(config: &SuiupConfig) -> Result<()> {
        let config_path = config_file_path()?;

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .with_fs_context("Failed to create configuration directory")?;
        }

        let content = serde_json::to_string_pretty(config)
            .with_config_context("Failed to serialize configuration")?;

        fs::write(&config_path, content).with_fs_context("Failed to write configuration file")?;

        Ok(())
    }

    pub async fn get(&self, key: &str) -> Result<()> {
        let value = match key {
            "mirror_url" => self.config.mirror_url.clone(),
            "cache_days" => self.config.cache_days.to_string(),
            "auto_cleanup" => self.config.auto_cleanup.to_string(),
            "max_cache_size" => self.config.max_cache_size.to_string(),
            "default_network" => self.config.default_network.clone(),
            "install_path" => self
                .config
                .install_path
                .clone()
                .unwrap_or_else(|| "default".to_string()),
            "disable_update_warnings" => self.config.disable_update_warnings.to_string(),
            "github_token" => self
                .config
                .github_token
                .clone()
                .unwrap_or_else(|| "not set".to_string()),
            _ => bail!("Unknown configuration key: {}", key),
        };

        println!("{}", value);
        Ok(())
    }

    pub async fn set(&mut self, key: &str, value: ConfigValue) -> Result<()> {
        self.validate_config_value(key, &value)?;

        match key {
            "mirror_url" => {
                if let ConfigValue::String(ref v) = value {
                    self.config.mirror_url = v.clone();
                }
            }
            "cache_days" => {
                if let ConfigValue::Number(v) = value {
                    self.config.cache_days = v as u32;
                }
            }
            "auto_cleanup" => {
                if let ConfigValue::Boolean(v) = value {
                    self.config.auto_cleanup = v;
                }
            }
            "max_cache_size" => {
                if let ConfigValue::Number(v) = value {
                    self.config.max_cache_size = v;
                }
            }
            "default_network" => {
                if let ConfigValue::String(ref v) = value {
                    self.config.default_network = v.clone();
                }
            }
            "install_path" => {
                if let ConfigValue::String(ref v) = value {
                    self.config.install_path = if v == "default" {
                        None
                    } else {
                        Some(v.clone())
                    };
                }
            }
            "disable_update_warnings" => {
                if let ConfigValue::Boolean(v) = value {
                    self.config.disable_update_warnings = v;
                }
            }
            "github_token" => {
                if let ConfigValue::String(ref v) = value {
                    self.config.github_token = if v == "default" || v.is_empty() {
                        None
                    } else {
                        Some(v.clone())
                    };
                }
            }
            _ => bail!("Unknown configuration key: {}", key),
        }

        Self::save_config(&self.config)?;
        print_success(&format!(
            "Configuration updated: {} = {}",
            key.cyan(),
            match &value {
                ConfigValue::String(s) => s.clone(),
                ConfigValue::Number(n) => n.to_string(),
                ConfigValue::Boolean(b) => b.to_string(),
            }
        ));

        Ok(())
    }

    pub async fn list(&self) -> Result<()> {
        println!("{}", "Current Configuration:".bold().cyan());
        println!("  {} = {}", "mirror_url".yellow(), self.config.mirror_url);
        println!("  {} = {}", "cache_days".yellow(), self.config.cache_days);
        println!(
            "  {} = {}",
            "auto_cleanup".yellow(),
            self.config.auto_cleanup
        );
        println!(
            "  {} = {} MB",
            "max_cache_size".yellow(),
            self.config.max_cache_size / 1024 / 1024
        );
        println!(
            "  {} = {}",
            "default_network".yellow(),
            self.config.default_network
        );
        println!(
            "  {} = {}",
            "install_path".yellow(),
            self.config
                .install_path
                .as_ref()
                .unwrap_or(&"default".to_string())
        );
        println!(
            "  {} = {}",
            "disable_update_warnings".yellow(),
            self.config.disable_update_warnings
        );
        println!(
            "  {} = {}",
            "github_token".yellow(),
            self.config
                .github_token
                .as_ref()
                .map(|t| if t.len() > 8 {
                    format!("{}...", &t[..8])
                } else {
                    t.clone()
                })
                .unwrap_or_else(|| "not set".to_string())
        );
        Ok(())
    }

    pub async fn reset(&mut self, yes: bool) -> Result<()> {
        if !yes {
            let term = Term::stdout();
            print!("Are you sure you want to reset configuration to defaults? [y/N]: ");
            std::io::stdout().flush()?;

            let input = term.read_line()?;
            if !input.trim().to_lowercase().starts_with('y') {
                println!("Configuration reset cancelled.");
                return Ok(());
            }
        }

        self.config = SuiupConfig::default();
        Self::save_config(&self.config)?;
        print_success("Configuration reset to defaults");
        Ok(())
    }

    pub async fn unset(&mut self, key: &str) -> Result<()> {
        match key {
            "mirror_url" => {
                self.config.mirror_url = default_mirror_url();
            }
            "cache_days" => {
                self.config.cache_days = default_cache_days();
            }
            "auto_cleanup" => {
                self.config.auto_cleanup = default_auto_cleanup();
            }
            "max_cache_size" => {
                self.config.max_cache_size = default_max_cache_size();
            }
            "default_network" => {
                self.config.default_network = default_default_network();
            }
            "install_path" => {
                self.config.install_path = default_install_path();
            }
            "disable_update_warnings" => {
                self.config.disable_update_warnings = default_disable_update_warnings();
            }
            "github_token" => {
                self.config.github_token = default_github_token();
            }
            _ => bail!("Unknown configuration key: {}", key),
        }

        Self::save_config(&self.config)?;
        print_success(&format!(
            "Configuration key '{}' reset to default",
            key.cyan()
        ));
        Ok(())
    }

    pub async fn validate(&self) -> Result<()> {
        let mut errors = Vec::new();

        // Validate mirror URL
        if let Err(e) = Validator::validate_url(&self.config.mirror_url) {
            errors.push(format!("mirror_url: {}", e));
        }

        // Validate cache_days range
        if let Err(e) = Validator::validate_cache_days(self.config.cache_days) {
            errors.push(format!("cache_days: {}", e));
        }

        // Validate max_cache_size range
        if let Err(e) = Validator::validate_cache_size(self.config.max_cache_size) {
            errors.push(format!("max_cache_size: {}", e));
        }

        // Validate default_network
        if let Err(e) = Validator::validate_network(&self.config.default_network) {
            errors.push(format!("default_network: {}", e));
        }

        // Validate install_path if specified
        if let Some(ref path) = self.config.install_path {
            if let Err(e) = Validator::validate_path_writable(path) {
                errors.push(format!("install_path: {}", e));
            }
        }

        // Validate github_token if specified
        if let Some(ref token) = self.config.github_token {
            if !token.is_empty() {
                if let Err(e) =
                    self.validate_config_value("github_token", &ConfigValue::String(token.clone()))
                {
                    errors.push(format!("github_token: {}", e));
                }
            }
        }

        if errors.is_empty() {
            print_success("Configuration is valid");
        } else {
            println!("{} Configuration validation failed:", "✗".red());
            for error in &errors {
                println!("  {}", error.red());
            }
            suggest_fix(
                "validation",
                "Use 'suiup config set <key> <value>' to fix configuration issues",
            );
            bail!(
                "Configuration validation failed with {} error(s)",
                errors.len()
            );
        }

        Ok(())
    }

    pub fn get_config(&self) -> &SuiupConfig {
        &self.config
    }

    fn validate_config_value(&self, key: &str, value: &ConfigValue) -> Result<()> {
        match key {
            "mirror_url" => {
                if let ConfigValue::String(url) = value {
                    Validator::validate_url(url)?;
                }
            }
            "cache_days" => {
                if let ConfigValue::Number(days) = value {
                    Validator::validate_cache_days(*days as u32)?;
                }
            }
            "max_cache_size" => {
                if let ConfigValue::Number(size) = value {
                    Validator::validate_cache_size(*size)?;
                }
            }
            "default_network" => {
                if let ConfigValue::String(network) = value {
                    Validator::validate_network(network)?;
                }
            }
            "install_path" => {
                if let ConfigValue::String(path) = value {
                    if path != "default" {
                        Validator::validate_path_writable(path)?;
                    }
                }
            }
            "github_token" => {
                if let ConfigValue::String(token) = value {
                    if !token.is_empty() && token != "default" {
                        // Basic GitHub token validation - should start with ghp_, gho_, ghu_, ghs_, or ghr_
                        if !token.starts_with("ghp_")
                            && !token.starts_with("gho_")
                            && !token.starts_with("ghu_")
                            && !token.starts_with("ghs_")
                            && !token.starts_with("ghr_")
                            && token.len() < 20
                        {
                            return Err(anyhow!("Invalid GitHub token format. GitHub tokens should start with 'ghp_', 'gho_', 'ghu_', 'ghs_', or 'ghr_' and be at least 20 characters long."));
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}
