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
    fs::write(root.join("bin/sui"), b"default sui")?;
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
        &json!({ "sui": ["testnet", "v1.2.0", false],
                 "walrus": ["testnet", "v1.0.0", false] }),
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
    assert!(!root.join("bin/sui").exists());
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
fn remove_reports_other_io_errors_without_dropping_metadata() -> Result<()> {
    let temp = TempDir::new()?;
    let root = temp.path();
    let path = root.join("suiup/binaries/testnet/sui-v1.0.0");
    fs::create_dir_all(&path)?;
    let binaries = json!([{ "binary_name": "sui", "network_release": "testnet",
                           "version": "v1.0.0", "debug": false, "path": path }]);
    let defaults = json!({ "sui": ["testnet", "v1.0.0", false] });
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
