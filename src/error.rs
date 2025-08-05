// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Result};
use colored::Colorize;
use std::fmt;

#[derive(Debug)]
pub enum SuiupError {
    ConfigError(String),
    InstallationError(String),
    ValidationError(String),
    NetworkError(String),
    FileSystemError(String),
    VersionError(String),
}

impl fmt::Display for SuiupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SuiupError::ConfigError(msg) => write!(f, "{} {}", "Configuration Error:".red().bold(), msg),
            SuiupError::InstallationError(msg) => write!(f, "{} {}", "Installation Error:".red().bold(), msg),
            SuiupError::ValidationError(msg) => write!(f, "{} {}", "Validation Error:".yellow().bold(), msg),
            SuiupError::NetworkError(msg) => write!(f, "{} {}", "Network Error:".red().bold(), msg),
            SuiupError::FileSystemError(msg) => write!(f, "{} {}", "File System Error:".red().bold(), msg),
            SuiupError::VersionError(msg) => write!(f, "{} {}", "Version Error:".yellow().bold(), msg),
        }
    }
}

impl std::error::Error for SuiupError {}

pub trait ErrorContext<T> {
    fn with_context(self, context: &str) -> Result<T>;
    fn with_config_context(self, context: &str) -> Result<T>;
    fn with_install_context(self, context: &str) -> Result<T>;
    fn with_validation_context(self, context: &str) -> Result<T>;
    fn with_network_context(self, context: &str) -> Result<T>;
    fn with_fs_context(self, context: &str) -> Result<T>;
    fn with_version_context(self, context: &str) -> Result<T>;
}

impl<T, E> ErrorContext<T> for Result<T, E>
where
    E: Into<anyhow::Error>,
{
    fn with_context(self, context: &str) -> Result<T> {
        self.map_err(|e| anyhow!("{}: {}", context, e.into()))
    }

    fn with_config_context(self, context: &str) -> Result<T> {
        self.map_err(|_| anyhow::Error::from(SuiupError::ConfigError(context.to_string())))
    }

    fn with_install_context(self, context: &str) -> Result<T> {
        self.map_err(|_| anyhow::Error::from(SuiupError::InstallationError(context.to_string())))
    }

    fn with_validation_context(self, context: &str) -> Result<T> {
        self.map_err(|_| anyhow::Error::from(SuiupError::ValidationError(context.to_string())))
    }

    fn with_network_context(self, context: &str) -> Result<T> {
        self.map_err(|_| anyhow::Error::from(SuiupError::NetworkError(context.to_string())))
    }

    fn with_fs_context(self, context: &str) -> Result<T> {
        self.map_err(|_| anyhow::Error::from(SuiupError::FileSystemError(context.to_string())))
    }

    fn with_version_context(self, context: &str) -> Result<T> {
        self.map_err(|_| anyhow::Error::from(SuiupError::VersionError(context.to_string())))
    }
}

pub fn user_friendly_error(err: &anyhow::Error) -> String {
    if let Some(suiup_err) = err.downcast_ref::<SuiupError>() {
        format!("{}", suiup_err)
    } else {
        format!("{} {}", "Error:".red().bold(), err)
    }
}

pub fn suggest_fix(_error_type: &str, suggestion: &str) {
    println!("\n{} {}", "💡 Suggestion:".cyan().bold(), suggestion);
    println!("{} suiup config --help", "Try:".green(), );
}

pub fn print_success(message: &str) {
    println!("{} {}", "✓".green().bold(), message);
}

pub fn print_warning(message: &str) {
    println!("{} {}", "⚠".yellow().bold(), message);
}

pub fn print_info(message: &str) {
    println!("{} {}", "ℹ".blue().bold(), message);
}