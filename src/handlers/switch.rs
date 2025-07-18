// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, bail, Result};
use tracing::info;

use crate::{
    handlers::update_default_version_file,
    paths::{binaries_dir, get_default_bin_dir},
    types::{BinaryVersion, InstalledBinaries},
};

#[cfg(not(windows))]
use std::os::unix::fs::PermissionsExt;

/// Handle the switch command
pub fn handle_switch(binary_spec: &str) -> Result<()> {
    // Parse the binary@network_release format
    let (binary_name, network_release) = parse_binary_spec(binary_spec)?;

    // Find the matching installed binary
    let installed_binaries = InstalledBinaries::new()?;
    let matching_binary =
        find_matching_binary(&installed_binaries, &binary_name, &network_release)?;

    // Switch to the found binary
    switch_to_binary(&matching_binary)?;

    println!(
        "Successfully switched to {}-{} from {}",
        matching_binary.binary_name, matching_binary.version, matching_binary.network_release
    );

    Ok(())
}

/// Parse binary@network_release format
pub fn parse_binary_spec(spec: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = spec.split('@').collect();

    if parts.len() != 2 {
        bail!(
            "Invalid format. Use 'binary@network_release' format (e.g., 'sui@testnet', 'mvr@main')"
        );
    }

    let binary_name = parts[0].to_string();
    let network_release = parts[1].to_string();

    if binary_name.is_empty() || network_release.is_empty() {
        bail!("Binary name and network/release cannot be empty");
    }

    Ok((binary_name, network_release))
}

/// Find the matching binary from installed binaries
pub fn find_matching_binary(
    installed_binaries: &InstalledBinaries,
    binary_name: &str,
    network_release: &str,
) -> Result<BinaryVersion> {
    let binaries = installed_binaries.binaries();

    // Find all matching binaries for the given binary name and network/release
    let mut matching_binaries: Vec<&BinaryVersion> = binaries
        .iter()
        .filter(|b| b.binary_name == binary_name && b.network_release == network_release)
        .collect();

    if matching_binaries.is_empty() {
        bail!(
            "No installed binary found for {}@{}. Use 'suiup show' to see available binaries.",
            binary_name,
            network_release
        );
    }

    // Sort by version to get the latest one (this is a simple string sort, might need improvement)
    matching_binaries.sort_by(|a, b| b.version.cmp(&a.version));

    Ok(matching_binaries[0].clone())
}

/// Switch to the specified binary by copying it to the default bin directory
fn switch_to_binary(binary: &BinaryVersion) -> Result<()> {
    let src = get_binary_source_path(binary);
    let dst = get_binary_destination_path(binary);

    // Copy the binary file
    copy_binary_file(&src, &dst, &binary.binary_name)?;

    // Set executable permissions on Unix systems
    #[cfg(unix)]
    set_executable_permissions(&dst)?;

    // Update the default version file
    update_default_version_file(
        &vec![binary.binary_name.clone()],
        binary.network_release.clone(),
        &binary.version,
        binary.debug,
    )?;

    Ok(())
}

/// Construct the source path for a binary
fn get_binary_source_path(binary: &BinaryVersion) -> std::path::PathBuf {
    let mut src = binaries_dir();
    src.push(&binary.network_release);

    // Handle nightly builds which have a different directory structure
    if binary.version == "nightly" {
        src.push("bin");
    }

    let binary_filename = if binary.debug {
        format!("{}-debug-{}", binary.binary_name, binary.version)
    } else {
        format!("{}-{}", binary.binary_name, binary.version)
    };

    src.push(&binary_filename);

    #[cfg(target_os = "windows")]
    src.set_extension("exe");

    src
}

/// Construct the destination path for a binary
fn get_binary_destination_path(binary: &BinaryVersion) -> std::path::PathBuf {
    let mut dst = get_default_bin_dir();
    let dst_name = if binary.debug {
        format!("{}-debug", binary.binary_name)
    } else {
        binary.binary_name.clone()
    };

    dst.push(&dst_name);

    #[cfg(target_os = "windows")]
    dst.set_extension("exe");

    dst
}

/// Copy binary file from source to destination
fn copy_binary_file(src: &std::path::Path, dst: &std::path::Path, binary_name: &str) -> Result<()> {
    info!("Copying from {} to {}", src.display(), dst.display());

    // Remove existing file if it exists
    if dst.exists() {
        std::fs::remove_file(dst)?;
    }

    // Copy the binary
    std::fs::copy(src, dst).map_err(|e| {
        anyhow!(
            "Error copying {} to the default folder (src: {}, dst: {}): {}",
            binary_name,
            src.display(),
            dst.display(),
            e
        )
    })?;

    Ok(())
}

/// Set executable permissions on Unix systems
#[cfg(unix)]
fn set_executable_permissions(path: &std::path::Path) -> Result<()> {
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}
