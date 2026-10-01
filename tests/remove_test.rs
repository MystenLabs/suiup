// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn remove_command(root: &Path) -> Command {
    let mut cmd = cargo_bin_cmd!("suiup");
    cmd.args(["remove", "sui"])
        .env("HOME", root)
        .env("USERPROFILE", root)
        .env("LOCALAPPDATA", root)
        .env("XDG_CONFIG_HOME", root)
        .env("XDG_DATA_HOME", root)
        .env("XDG_CACHE_HOME", root.join("cache"))
        .env("TEMP", root.join("cache"))
        .env("SUIUP_DEFAULT_BIN_DIR", root.join("bin"));
    cmd
}

fn write_metadata(root: &Path, binaries: &Value, defaults: &Value) -> Result<()> {
    fs::create_dir_all(root.join("suiup"))?;
    fs::write(
        root.join("suiup/installed_binaries.json"),
        serde_json::to_vec(&json!({ "binaries": binaries }))?,
    )?;
    fs::write(
        root.join("suiup/default_version.json"),
        serde_json::to_vec(defaults)?,
    )?;
    Ok(())
}

fn read_metadata(root: &Path, filename: &str) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(
        root.join("suiup").join(filename),
    )?)?)
}

fn sui_entry(path: &Path, network: &str, version: &str, debug: bool) -> Value {
    json!({ "binary_name": "sui", "network_release": network,
            "version": version, "debug": debug, "path": path })
}

fn default_sui_path(root: &Path) -> std::path::PathBuf {
    root.join("bin")
        .join(if cfg!(windows) { "sui.exe" } else { "sui" })
}

#[test]
fn remove_preserves_active_network_version_and_build() -> Result<()> {
    for (network, version, debug) in [
        ("testnet", "v1.2.0", false),
        ("testnet", "v1.2.0", true),
        ("main", "nightly", false),
    ] {
        let temp = TempDir::new()?;
        let root = temp.path();
        let active_path = root.join("active");
        fs::write(&active_path, b"active sui")?;
        fs::create_dir_all(root.join("bin"))?;
        fs::write(default_sui_path(root), b"default sui")?;
        let active = sui_entry(&active_path, network, version, debug);
        let inactive = [
            sui_entry(&root.join("old"), network, "v1.0.0", debug),
            sui_entry(&root.join("other-network"), "mainnet", version, debug),
            sui_entry(&root.join("other-build"), network, version, !debug),
        ];
        for entry in &inactive {
            fs::write(entry["path"].as_str().unwrap(), b"inactive sui")?;
        }
        let defaults = json!({ "sui": [network, version, debug] });
        write_metadata(
            root,
            &json!([
                inactive[0],
                active,
                inactive[1],
                inactive[2],
                sui_entry(&root.join("missing"), network, "v0.9.0", debug),
            ]),
            &defaults,
        )?;

        remove_command(root).assert().success();

        assert_eq!(fs::read(&active_path)?, b"active sui");
        assert_eq!(fs::read(default_sui_path(root))?, b"default sui");
        for entry in &inactive {
            assert!(!Path::new(entry["path"].as_str().unwrap()).exists());
        }
        assert_eq!(
            read_metadata(root, "installed_binaries.json")?,
            json!({ "binaries": [active] })
        );
        assert_eq!(read_metadata(root, "default_version.json")?, defaults);
    }
    Ok(())
}

#[test]
fn remove_inactive_entry_sharing_active_path_keeps_file() -> Result<()> {
    let temp = TempDir::new()?;
    let root = temp.path();
    let path = root.join("shared-sui");
    fs::write(&path, b"active sui")?;
    let active = sui_entry(&path, "testnet", "v1.0.0", true);
    let inactive = sui_entry(&path, "testnet", "v1.0.0", false);
    let defaults = json!({ "sui": ["testnet", "v1.0.0", true] });
    write_metadata(root, &json!([inactive, active]), &defaults)?;

    remove_command(root).assert().success();

    assert_eq!(fs::read(&path)?, b"active sui");
    assert_eq!(
        read_metadata(root, "installed_binaries.json")?,
        json!({ "binaries": [active] })
    );
    assert_eq!(read_metadata(root, "default_version.json")?, defaults);
    Ok(())
}

#[test]
fn remove_only_active_version_is_a_noop() -> Result<()> {
    let temp = TempDir::new()?;
    let root = temp.path();
    let path = root.join("active-sui");
    fs::write(&path, b"active sui")?;
    fs::create_dir_all(root.join("bin"))?;
    fs::write(default_sui_path(root), b"default sui")?;
    let active = sui_entry(&path, "testnet", "v1.0.0", false);
    let defaults = json!({ "sui": ["testnet", "v1.0.0", false] });
    write_metadata(root, &json!([active]), &defaults)?;

    for _ in 0..2 {
        remove_command(root).assert().success();
        assert_eq!(fs::read(&path)?, b"active sui");
        assert_eq!(fs::read(default_sui_path(root))?, b"default sui");
        assert_eq!(
            read_metadata(root, "installed_binaries.json")?,
            json!({ "binaries": [active] })
        );
        assert_eq!(read_metadata(root, "default_version.json")?, defaults);
    }
    Ok(())
}

#[test]
fn remove_missing_active_entry_preserves_default_copy() -> Result<()> {
    let temp = TempDir::new()?;
    let root = temp.path();
    fs::create_dir_all(root.join("bin"))?;
    fs::write(default_sui_path(root), b"default sui")?;
    let missing = sui_entry(&root.join("missing-sui"), "testnet", "v1.0.0", false);
    let defaults = json!({ "sui": ["testnet", "v1.0.0", false] });
    write_metadata(root, &json!([missing]), &defaults)?;

    for _ in 0..2 {
        remove_command(root).assert().success();
        assert_eq!(fs::read(default_sui_path(root))?, b"default sui");
        assert_eq!(
            read_metadata(root, "installed_binaries.json")?,
            json!({ "binaries": [] })
        );
        assert_eq!(read_metadata(root, "default_version.json")?, defaults);
    }
    Ok(())
}

#[test]
fn remove_clears_stale_default_without_installed_entries() -> Result<()> {
    let temp = TempDir::new()?;
    let root = temp.path();
    write_metadata(
        root,
        &json!([]),
        &json!({ "sui": ["testnet", "v1.0.0", false] }),
    )?;

    remove_command(root).assert().success();

    assert_eq!(read_metadata(root, "default_version.json")?, json!({}));
    assert_eq!(
        read_metadata(root, "installed_binaries.json")?,
        json!({ "binaries": [] })
    );
    Ok(())
}

#[test]
fn remove_missing_and_existing_binaries_saves_metadata() -> Result<()> {
    let temp = TempDir::new()?;
    let root = temp.path();
    let binaries_dir = root.join("suiup/binaries/testnet");
    fs::create_dir_all(&binaries_dir)?;
    let present = binaries_dir.join("sui-v1.2.0");
    let missing = binaries_dir.join("sui-v1.1.0");
    let walrus_path = binaries_dir.join("walrus-v1.0.0");
    fs::write(&present, b"sui")?;
    fs::write(&walrus_path, b"walrus")?;
    fs::create_dir_all(root.join("bin"))?;
    fs::write(default_sui_path(root), b"default sui")?;
    fs::write(root.join("bin/walrus"), b"default walrus")?;

    let walrus = json!({
        "binary_name": "walrus", "network_release": "testnet",
        "version": "v1.0.0", "debug": false, "path": walrus_path,
    });
    write_metadata(
        root,
        &json!([
            { "binary_name": "sui", "network_release": "testnet",
              "version": "v1.1.0", "debug": false, "path": missing },
            { "binary_name": "sui", "network_release": "testnet",
              "version": "v1.2.0", "debug": false, "path": present },
            { "binary_name": "sui", "network_release": "testnet",
              "version": "v1.2.0", "debug": true, "path": present },
            { "binary_name": "sui", "network_release": "mainnet",
              "version": "v1.0.0", "debug": false, "path": null },
            walrus,
        ]),
        &json!({ "walrus": ["testnet", "v1.0.0", false] }),
    )?;

    remove_command(root).assert().success();

    assert_eq!(
        read_metadata(root, "installed_binaries.json")?,
        json!({ "binaries": [walrus] })
    );
    assert_eq!(
        read_metadata(root, "default_version.json")?,
        json!({ "walrus": ["testnet", "v1.0.0", false] })
    );
    assert!(!present.exists());
    assert!(!default_sui_path(root).exists());
    assert_eq!(fs::read(walrus_path)?, b"walrus");
    assert_eq!(fs::read(root.join("bin/walrus"))?, b"default walrus");
    Ok(())
}

#[test]
fn remove_all_missing_binaries_saves_empty_metadata() -> Result<()> {
    let temp = TempDir::new()?;
    let root = temp.path();
    write_metadata(
        root,
        &json!([{ "binary_name": "sui", "network_release": "testnet",
                  "version": "v1.0.0", "debug": false,
                  "path": root.join("suiup/binaries/testnet/sui-v1.0.0") }]),
        &json!({ "sui": ["testnet", "v1.0.0", false] }),
    )?;

    remove_command(root).assert().success();

    assert_eq!(
        read_metadata(root, "installed_binaries.json")?,
        json!({ "binaries": [] })
    );
    assert_eq!(read_metadata(root, "default_version.json")?, json!({}));
    remove_command(root).assert().success();
    assert_eq!(
        read_metadata(root, "installed_binaries.json")?,
        json!({ "binaries": [] })
    );
    assert_eq!(read_metadata(root, "default_version.json")?, json!({}));
    Ok(())
}

#[test]
fn remove_preflights_all_installed_paths_before_deleting() -> Result<()> {
    let temp = TempDir::new()?;
    let root = temp.path();
    let present = root.join("present");
    let directory = root.join("directory");
    fs::write(&present, b"inactive sui")?;
    fs::create_dir(&directory)?;
    let binaries = json!([
        sui_entry(&root.join("missing"), "testnet", "v1.0.0", false),
        sui_entry(&present, "testnet", "v1.1.0", false),
        sui_entry(&directory, "testnet", "v1.2.0", false),
    ]);
    write_metadata(root, &binaries, &json!({}))?;

    remove_command(root).assert().failure();

    assert_eq!(fs::read(present)?, b"inactive sui");
    assert!(directory.is_dir());
    assert_eq!(
        read_metadata(root, "installed_binaries.json")?,
        json!({ "binaries": binaries })
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn remove_preserves_file_symlinks_and_prunes_dangling_symlinks() -> Result<()> {
    use std::os::unix::fs::symlink;

    for target_exists in [true, false] {
        let temp = TempDir::new()?;
        let root = temp.path();
        let target = root.join("sui-target");
        let active = root.join("active-link");
        if target_exists {
            fs::write(&target, b"active sui")?;
        }
        fs::create_dir_all(root.join("bin"))?;
        symlink(&target, &active)?;
        symlink(&target, default_sui_path(root))?;
        let binaries = json!([sui_entry(&active, "testnet", "v1.0.0", false)]);
        let defaults = json!({ "sui": ["testnet", "v1.0.0", false] });
        write_metadata(root, &binaries, &defaults)?;

        remove_command(root).assert().success();

        if target_exists {
            assert_eq!(fs::read(active)?, b"active sui");
            assert_eq!(fs::read(default_sui_path(root))?, b"active sui");
            assert_eq!(
                read_metadata(root, "installed_binaries.json")?,
                json!({ "binaries": binaries })
            );
            assert_eq!(read_metadata(root, "default_version.json")?, defaults);
        } else {
            assert_eq!(
                fs::symlink_metadata(active).unwrap_err().kind(),
                std::io::ErrorKind::NotFound
            );
            assert_eq!(
                fs::symlink_metadata(default_sui_path(root))
                    .unwrap_err()
                    .kind(),
                std::io::ErrorKind::NotFound
            );
            assert_eq!(
                read_metadata(root, "installed_binaries.json")?,
                json!({ "binaries": [] })
            );
            assert_eq!(read_metadata(root, "default_version.json")?, json!({}));
        }
    }
    Ok(())
}

#[test]
fn remove_rejects_active_directory() -> Result<()> {
    let temp = TempDir::new()?;
    let root = temp.path();
    let directory = root.join("active-directory");
    fs::create_dir(&directory)?;
    let binaries = json!([sui_entry(&directory, "testnet", "v1.0.0", false)]);
    let defaults = json!({ "sui": ["testnet", "v1.0.0", false] });
    write_metadata(root, &binaries, &defaults)?;

    remove_command(root).assert().failure();

    assert!(directory.is_dir());
    assert_eq!(
        read_metadata(root, "installed_binaries.json")?,
        json!({ "binaries": binaries })
    );
    assert_eq!(read_metadata(root, "default_version.json")?, defaults);
    Ok(())
}

#[test]
fn remove_rejects_default_directory_before_deleting() -> Result<()> {
    for active_file_exists in [false, true] {
        let temp = TempDir::new()?;
        let root = temp.path();
        let active = root.join("active");
        let inactive = root.join("inactive");
        if active_file_exists {
            fs::write(&active, b"active sui")?;
        }
        fs::write(&inactive, b"inactive sui")?;
        fs::create_dir_all(default_sui_path(root))?;
        let binaries = json!([
            sui_entry(&inactive, "testnet", "v1.0.0", false),
            sui_entry(&active, "testnet", "v1.1.0", false),
        ]);
        let defaults = json!({ "sui": ["testnet", "v1.1.0", false] });
        write_metadata(root, &binaries, &defaults)?;

        remove_command(root).assert().failure();

        assert_eq!(fs::read(inactive)?, b"inactive sui");
        assert!(default_sui_path(root).is_dir());
        assert_eq!(
            read_metadata(root, "installed_binaries.json")?,
            json!({ "binaries": binaries })
        );
        assert_eq!(read_metadata(root, "default_version.json")?, defaults);
    }
    Ok(())
}

#[test]
fn remove_reports_other_io_errors_without_dropping_metadata() -> Result<()> {
    let temp = TempDir::new()?;
    let root = temp.path();
    let path = root.join("suiup/binaries/testnet/sui-v1.0.0");
    fs::create_dir_all(&path)?;
    let binaries = json!([{ "binary_name": "sui", "network_release": "testnet",
                           "version": "v1.0.0", "debug": false, "path": path }]);
    let defaults = json!({});
    write_metadata(root, &binaries, &defaults)?;

    remove_command(root)
        .assert()
        .failure()
        .stderr(predicates::str::contains("Cannot remove file"));

    assert!(path.is_dir());
    assert_eq!(
        read_metadata(root, "installed_binaries.json")?,
        json!({ "binaries": binaries })
    );
    assert_eq!(read_metadata(root, "default_version.json")?, defaults);
    Ok(())
}
