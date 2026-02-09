// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result, bail};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

const SUIBASE_REPO_URL: &str = "https://github.com/chainmovers/suibase.git";

pub fn install() -> Result<()> {
    ensure_supported_platform()?;
    ensure_command_available("git")?;

    let suibase_dir = suibase_dir()?;
    if suibase_dir.exists() {
        println!(
            "Suibase already exists at {}. Pulling latest changes...",
            suibase_dir.display()
        );
        run_command(
            "git",
            &[
                "-C",
                suibase_dir
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("Invalid suibase path"))?,
                "pull",
                "--ff-only",
            ],
            "Failed to pull suibase repository",
        )?;
    } else {
        println!("Cloning suibase into {} ...", suibase_dir.display());
        run_command(
            "git",
            &[
                "clone",
                SUIBASE_REPO_URL,
                suibase_dir.to_string_lossy().as_ref(),
            ],
            "Failed to clone suibase repository",
        )?;
    }

    let install_script = suibase_dir.join("install");
    ensure_file_exists(&install_script, "Suibase install script not found")?;
    println!("Running {} ...", install_script.display());
    run_script(&install_script, "Suibase install failed")
}

pub fn update() -> Result<()> {
    ensure_supported_platform()?;

    let update_script = suibase_dir()?.join("update");
    ensure_file_exists(
        &update_script,
        "Suibase update script not found. Run `suiup suibase install` first.",
    )?;
    println!("Running {} ...", update_script.display());
    run_script(&update_script, "Suibase update failed")
}

pub fn uninstall() -> Result<()> {
    ensure_supported_platform()?;

    let uninstall_script = suibase_dir()?.join("uninstall");
    ensure_file_exists(
        &uninstall_script,
        "Suibase uninstall script not found. It may already be removed.",
    )?;
    println!("Running {} ...", uninstall_script.display());
    run_script(&uninstall_script, "Suibase uninstall failed")
}

pub fn doctor() -> Result<()> {
    ensure_supported_platform()?;
    let suibase_dir = suibase_dir()?;
    let local_bin = local_bin_dir()?;

    println!("Checking suibase environment...");
    println!(
        "- suibase directory: {}",
        if suibase_dir.exists() {
            "OK"
        } else {
            "MISSING"
        }
    );
    println!(
        "- install script: {}",
        if suibase_dir.join("install").exists() {
            "OK"
        } else {
            "MISSING"
        }
    );
    println!(
        "- update script: {}",
        if suibase_dir.join("update").exists() {
            "OK"
        } else {
            "MISSING"
        }
    );
    println!(
        "- uninstall script: {}",
        if suibase_dir.join("uninstall").exists() {
            "OK"
        } else {
            "MISSING"
        }
    );
    let has_local_bin = path_contains(&local_bin);
    println!(
        "- PATH contains {}: {}",
        local_bin.display(),
        if has_local_bin { "YES" } else { "NO" }
    );

    if !has_local_bin {
        println!("  Add this to your shell profile:");
        println!("  export PATH=\"{}:$PATH\"", local_bin.display());
    }

    Ok(())
}

fn ensure_supported_platform() -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        bail!("suibase is supported on Linux/macOS/WSL2. Native Windows is not supported.");
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(())
    }
}

fn ensure_command_available(command: &str) -> Result<()> {
    let output = Command::new(command)
        .arg("--version")
        .output()
        .with_context(|| format!("Failed to execute `{command} --version`"))?;
    if !output.status.success() {
        bail!("{command} is not installed or not available in PATH");
    }
    Ok(())
}

fn run_script(script: &Path, context: &str) -> Result<()> {
    let status = Command::new("bash")
        .arg(script)
        .status()
        .with_context(|| format!("Failed to execute {}", script.display()))?;
    if !status.success() {
        bail!("{context}");
    }
    Ok(())
}

fn run_command(command: &str, args: &[&str], context: &str) -> Result<()> {
    let status = Command::new(command)
        .args(args)
        .status()
        .with_context(|| format!("Failed to execute command `{command}`"))?;
    if !status.success() {
        bail!("{context}");
    }
    Ok(())
}

fn ensure_file_exists(path: &Path, msg: &str) -> Result<()> {
    if !path.exists() {
        bail!("{msg}");
    }
    Ok(())
}

fn suibase_dir() -> Result<PathBuf> {
    Ok(home_dir()?.join("suibase"))
}

fn local_bin_dir() -> Result<PathBuf> {
    Ok(home_dir()?.join(".local").join("bin"))
}

fn home_dir() -> Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Unable to determine home directory"))
}

fn path_contains(target: &Path) -> bool {
    let path_var = match env::var_os("PATH") {
        Some(value) => value,
        None => return false,
    };

    env::split_paths(&path_var).any(|entry| entry == target)
}
