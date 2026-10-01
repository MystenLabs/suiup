// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result};
use tracing::debug;

use crate::fs_utils::{read_json_file, write_json_file};
use crate::paths::{default_file_path, get_default_bin_dir};
use crate::types::{BinaryVersion, InstalledBinaries};

/// Remove inactive versions and stale installed entries while preserving the active binary.
pub fn remove_component(binary: &str) -> Result<()> {
    let mut installed_binaries = InstalledBinaries::new()?;
    let binaries = installed_binaries
        .binaries()
        .iter()
        .filter(|b| binary == b.binary_name)
        .cloned()
        .collect::<Vec<_>>();

    let default_file = default_file_path()?;
    let mut default_binaries: std::collections::BTreeMap<String, (String, String, bool)> =
        read_json_file(&default_file)?;
    let active_version = default_binaries.get(binary);

    if binaries.is_empty() && active_version.is_none() {
        println!("No binaries found to remove");
        return Ok(());
    }

    let is_active = |entry: &BinaryVersion| {
        active_version.is_some_and(|(network, version, debug)| {
            entry.network_release == *network && entry.version == *version && entry.debug == *debug
        })
    };

    // Legacy entries can share a path across debug and release builds.
    let mut active_paths = HashSet::new();
    for entry in binaries.iter().filter(|entry| is_active(entry)) {
        if let Some(path) = entry.path.as_deref()
            && Path::new(path)
                .try_exists()
                .with_context(|| format!("Cannot check active binary {path}"))?
        {
            active_paths.insert(Path::new(path));
        }
    }

    let default_bin_path = get_default_bin_dir().join(binary);
    #[cfg(windows)]
    let default_bin_path = default_bin_path.with_extension("exe");

    let preserve_default = active_version.is_some()
        && (!active_paths.is_empty()
            || default_bin_path.try_exists().with_context(|| {
                format!("Cannot check default binary {}", default_bin_path.display())
            })?);

    for entry in &binaries {
        if is_active(entry)
            && entry
                .path
                .as_deref()
                .is_some_and(|path| active_paths.contains(Path::new(path)))
        {
            println!("Keeping active binary: {entry} [{}]", entry.network_release);
            continue;
        }

        if let Some(path) = entry.path.as_deref()
            && !active_paths.contains(Path::new(path))
            && !(preserve_default && Path::new(path) == default_bin_path)
        {
            match std::fs::remove_file(path) {
                Ok(()) => println!("Removed binary: {} from {path}", entry.binary_name),
                Err(err) if err.kind() == ErrorKind::NotFound => {
                    println!("Binary {path} does not exist. Removing its installed entry.");
                }
                Err(err) => return Err(err).with_context(|| format!("Cannot remove file {path}")),
            }
        }
        installed_binaries.remove_version(entry);
    }

    if preserve_default {
        println!("Keeping the default executable and setting for {binary}");
    } else {
        match std::fs::remove_file(&default_bin_path) {
            Ok(()) => debug!("Removed default binary {}", default_bin_path.display()),
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("Cannot remove file {}", default_bin_path.display()));
            }
        }
        default_binaries.remove(binary);
        write_json_file(&default_file, &default_binaries)?;
    }

    installed_binaries.save_to_file()?;
    Ok(())
}
