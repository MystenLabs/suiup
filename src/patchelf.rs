// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

/// Default path to the nix-runtime-deps.json file
const DEFAULT_PATCHELF_CONFIG: &str = "/usr/share/suiup/nix-runtime-deps.json";

/// Patchelf executable name
const PATCHELF_EXECUTABLE: &str = "patchelf";

#[derive(Debug, Deserialize)]
pub struct NixRuntimeDeps {
    pub interpreter: String,
    pub lib_path: String,
}

/// Load the Nix runtime dependencies from a JSON file
/// This file path is specified via the SUIUP_PATCHELF environment variable,
/// or falls back to the default path
pub fn load_nix_runtime_deps() -> Result<NixRuntimeDeps> {
    let config_path = std::env::var("SUIUP_PATCHELF_CONFIG")
        .unwrap_or_else(|_| DEFAULT_PATCHELF_CONFIG.to_string());

    let config_path = Path::new(&config_path);

    if !config_path.exists() {
        return Err(anyhow!(
            "Nix runtime dependencies config not found at {}. Set SUIUP_PATCHELF_CONFIG environment variable or ensure the file exists at the default location.",
            config_path.display()
        ));
    }

    let content = std::fs::read_to_string(config_path).map_err(|e| {
        anyhow!(
            "Failed to read json dependencies file: {} {}",
            config_path.display(),
            e
        )
    })?;
    let deps: NixRuntimeDeps = serde_json::from_str(&content)?;
    Ok(deps)
}

/// Patch a binary with patchelf using the Nix runtime dependencies
pub fn patch_binary(binary_path: &Path) -> Result<()> {
    #[cfg(not(target_os = "linux"))]
    {
        // patchelf is only relevant on Linux
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        if !binary_path.exists() {
            return Err(anyhow!("Binary not found: {}", binary_path.display()));
        }

        let deps = load_nix_runtime_deps()?;

        println!("Patching binary: {}", binary_path.display());

        // Set interpreter
        let status = Command::new(PATCHELF_EXECUTABLE)
            .arg("--set-interpreter")
            .arg(&deps.interpreter)
            .arg("--set-rpath")
            .arg(&deps.lib_path)
            .arg(binary_path)
            .status()
            .map_err(|e| {
                anyhow!(
                    "Failed to run {} (is it installed?): {}",
                    PATCHELF_EXECUTABLE,
                    e
                )
            })?;

        if !status.success() {
            return Err(anyhow!(
                "Failed to set interpreter / rpath with {}",
                PATCHELF_EXECUTABLE
            ));
        }

        println!("✓ Binary patched successfully");
        println!("  Interpreter: {}", deps.interpreter);
        println!("  RPATH: {}", deps.lib_path);

        Ok(())
    }
}

/// Check if patchelf is available in the system
#[allow(dead_code)]
pub fn is_patchelf_available() -> bool {
    #[cfg(not(target_os = "linux"))]
    {
        false
    }

    #[cfg(target_os = "linux")]
    {
        Command::new(PATCHELF_EXECUTABLE)
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}
