// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result, bail};
use tracing::debug;

use crate::fs_utils::{read_json_file, write_json_file};
use crate::paths::{default_file_path, get_default_bin_dir, installed_binaries_file};
use crate::types::{BinaryVersion, InstalledBinaries};

/// Remove inactive versions and stale installed entries while preserving the active binary.
pub fn remove_component(binary: &str) -> Result<()> {
    let default_bin_path = get_default_bin_dir().join(binary);
    #[cfg(windows)]
    let default_bin_path = default_bin_path.with_extension("exe");

    remove_component_from_paths(
        binary,
        &installed_binaries_file()?,
        &default_file_path()?,
        &default_bin_path,
        |path| std::fs::remove_file(path),
    )
}

fn remove_component_from_paths(
    binary: &str,
    installed_file: &Path,
    default_file: &Path,
    default_bin_path: &Path,
    mut remove_file: impl FnMut(&Path) -> std::io::Result<()>,
) -> Result<()> {
    let mut installed_binaries: InstalledBinaries = read_json_file(installed_file)?;
    let binaries = installed_binaries
        .binaries()
        .iter()
        .filter(|b| binary == b.binary_name)
        .cloned()
        .collect::<Vec<_>>();

    let mut default_binaries: std::collections::BTreeMap<String, (String, String, bool)> =
        read_json_file(default_file)?;
    let active_version = default_binaries.get(binary).cloned();

    if binaries.is_empty() && active_version.is_none() {
        println!("No binaries found to remove");
        return Ok(());
    }

    let is_active = |entry: &BinaryVersion| {
        active_version
            .as_ref()
            .is_some_and(|(network, version, debug)| {
                entry.network_release == *network
                    && entry.version == *version
                    && entry.debug == *debug
            })
    };

    // Validate every path before deletion. Legacy entries can share an active file's path.
    let mut active_paths = HashSet::new();
    for entry in &binaries {
        if let Some(path) = entry.path.as_deref()
            && is_file_or_missing(Path::new(path))?
            && is_active(entry)
        {
            active_paths.insert(Path::new(path));
        }
    }

    let default_exists = is_file_or_missing(default_bin_path)?;
    let preserve_default = active_version.is_some() && (!active_paths.is_empty() || default_exists);

    let removal_result: Result<()> = (|| {
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
                match remove_file(Path::new(path)) {
                    Ok(()) => println!("Removed binary: {} from {path}", entry.binary_name),
                    Err(err) if err.kind() == ErrorKind::NotFound => {
                        println!("Binary {path} does not exist. Removing its installed entry.");
                    }
                    Err(err) => {
                        return Err(err).with_context(|| format!("Cannot remove file {path}"));
                    }
                }
            }
            installed_binaries.remove_version(entry);
        }

        if preserve_default {
            println!("Keeping the default executable and setting for {binary}");
        } else {
            match remove_file(default_bin_path) {
                Ok(()) => debug!("Removed default binary {}", default_bin_path.display()),
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(err).with_context(|| {
                        format!("Cannot remove file {}", default_bin_path.display())
                    });
                }
            }
            default_binaries.remove(binary);
            write_json_file(default_file, &default_binaries)?;
        }
        Ok(())
    })();

    // Commit completed removals even if a later deletion or default update failed.
    let save_result = write_json_file(installed_file, &installed_binaries);
    match (removal_result, save_result) {
        (Err(removal), Err(save)) => {
            Err(removal.context(format!("Cannot save completed removals: {save:#}")))
        }
        (Err(err), _) | (_, Err(err)) => Err(err),
        (Ok(()), Ok(())) => Ok(()),
    }
}

/// Return whether a path resolves to a regular file, rejecting other existing file types.
fn is_file_or_missing(path: &Path) -> Result<bool> {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(true),
        Ok(_) => bail!("Cannot remove file {}: not a regular file", path.display()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err).with_context(|| format!("Cannot inspect file {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn completed_removals_are_saved_after_a_later_deletion_race() -> Result<()> {
        let temp = TempDir::new()?;
        let root = temp.path();
        let installed_file = root.join("installed.json");
        let default_file = root.join("defaults.json");
        let default_binary = root.join("default-sui");
        let entries = ["missing", "removed", "blocked", "pending"].map(|name| {
            json!({ "binary_name": "sui", "network_release": "testnet",
                    "version": name, "debug": false, "path": root.join(name) })
        });
        for name in ["removed", "blocked", "pending"] {
            fs::write(root.join(name), b"sui")?;
        }
        write_json_file(&installed_file, &json!({ "binaries": entries }))?;
        write_json_file(&default_file, &json!({}))?;

        let result = remove_component_from_paths(
            "sui",
            &installed_file,
            &default_file,
            &default_binary,
            |path| {
                if path == root.join("blocked") {
                    fs::remove_file(path)?;
                    fs::create_dir(path)?;
                }
                fs::remove_file(path)
            },
        );

        assert!(result.is_err());
        assert!(!root.join("removed").exists());
        assert!(root.join("blocked").is_dir());
        assert_eq!(fs::read(root.join("pending"))?, b"sui");
        assert_eq!(
            read_json_file::<Value>(&installed_file)?,
            json!({ "binaries": [entries[2], entries[3]] })
        );
        Ok(())
    }

    #[test]
    fn deletion_and_metadata_save_errors_are_both_reported() -> Result<()> {
        let temp = TempDir::new()?;
        let root = temp.path();
        let installed_file = root.join("installed.json");
        let default_file = root.join("defaults.json");
        let default_binary = root.join("default-sui");
        let binary_path = root.join("sui");
        fs::write(&binary_path, b"sui")?;
        fs::write(&default_binary, b"sui")?;
        write_json_file(
            &installed_file,
            &json!({ "binaries": [{
            "binary_name": "sui", "network_release": "testnet",
            "version": "v1.0.0", "debug": false, "path": binary_path,
        }] }),
        )?;
        write_json_file(&default_file, &json!({}))?;

        let error = remove_component_from_paths(
            "sui",
            &installed_file,
            &default_file,
            &default_binary,
            |path| {
                if path == default_binary {
                    for blocked in [&installed_file, &default_binary] {
                        fs::remove_file(blocked)?;
                        fs::create_dir(blocked)?;
                    }
                }
                fs::remove_file(path)
            },
        )
        .unwrap_err();

        assert!(!binary_path.exists());
        let diagnostic = format!("{error:#}");
        assert!(diagnostic.contains(&installed_file.display().to_string()));
        assert!(diagnostic.contains(&default_binary.display().to_string()));
        Ok(())
    }

    #[test]
    fn completed_removals_are_saved_after_default_cleanup_errors() -> Result<()> {
        for fail_metadata_write in [false, true] {
            let temp = TempDir::new()?;
            let root = temp.path();
            let installed_file = root.join("installed.json");
            let default_file = root.join("defaults.json");
            let default_binary = root.join("default-sui");
            let binary_path = root.join("sui");
            fs::write(&binary_path, b"sui")?;
            fs::write(&default_binary, b"sui")?;
            write_json_file(
                &installed_file,
                &json!({ "binaries": [{
                "binary_name": "sui", "network_release": "testnet",
                "version": "v1.0.0", "debug": false, "path": binary_path,
            }] }),
            )?;
            write_json_file(&default_file, &json!({}))?;

            let result = remove_component_from_paths(
                "sui",
                &installed_file,
                &default_file,
                &default_binary,
                |path| {
                    if path == default_binary {
                        let blocked = if fail_metadata_write {
                            &default_file
                        } else {
                            path
                        };
                        fs::remove_file(blocked)?;
                        fs::create_dir(blocked)?;
                    }
                    fs::remove_file(path)
                },
            );

            assert!(result.is_err());
            assert!(!binary_path.exists());
            assert_eq!(
                read_json_file::<Value>(&installed_file)?,
                json!({ "binaries": [] })
            );
            if fail_metadata_write {
                assert!(!default_binary.exists());
                assert!(default_file.is_dir());
            } else {
                assert!(default_binary.is_dir());
                assert_eq!(read_json_file::<Value>(&default_file)?, json!({}));
            }
        }
        Ok(())
    }
}
