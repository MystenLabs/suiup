// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, HashMap};

use anyhow::{Error, Result};
use colored::Colorize;

use crate::handlers::installed_binaries_grouped_by_network;
use crate::handlers::release::{last_release_for_network, release_list};
use crate::registry::{BinaryRegistry, InstallationType};
use crate::standalone::StandaloneInstaller;
use crate::types::{BinaryVersion, InstalledBinaries};

enum UpdateStatus {
    UpToDate,
    UpdateAvailable,
    Nightly,
    FetchError(String),
}

struct StatusEntry {
    network: Option<String>,
    installed_version: String,
    latest_version: Option<String>,
    status: UpdateStatus,
}

/// Handles the `status` command -- checks for available updates for all installed binaries.
pub async fn handle_status(github_token: Option<String>) -> Result<(), Error> {
    let installed_binaries = InstalledBinaries::new()?;
    let binaries = installed_binaries.binaries().to_vec();

    if binaries.is_empty() {
        println!("No binaries installed. Use `suiup install` to install binaries.");
        return Ok(());
    }

    println!("{}", "Checking for updates...".dimmed());

    // Separate nightly binaries -- they are installed from branches and we don't
    // track commit SHAs, so we cannot check for updates.
    let nightly_binaries: Vec<&BinaryVersion> =
        binaries.iter().filter(|b| b.version == "nightly").collect();
    let release_binaries: Vec<BinaryVersion> = binaries
        .iter()
        .filter(|b| b.version != "nightly")
        .cloned()
        .collect();

    let binaries_by_network = installed_binaries_grouped_by_network(Some(installed_binaries))?;
    let registry = BinaryRegistry::global();

    // Collect unique installed binary names from release (non-nightly) binaries only
    let mut installed_names: Vec<String> = release_binaries
        .iter()
        .map(|b| b.binary_name.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    installed_names.sort();

    // Group installed binaries by repo for deduplication of API calls
    let mut repo_to_names: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for name in &installed_names {
        if let Some(config) = registry.get(name) {
            repo_to_names
                .entry(&config.repository)
                .or_default()
                .push(name);
        }
    }

    // Fetch releases per repo (deduplicated)
    let mut network_releases: HashMap<String, Result<Vec<crate::types::Release>, String>> =
        HashMap::new();
    let mut standalone_installers: HashMap<String, StandaloneInstaller> = HashMap::new();

    for repo_slug in repo_to_names.keys() {
        // Determine if this repo is standalone or network-based by checking the first binary's config
        let first_name = repo_to_names[repo_slug].first().unwrap();
        let config = registry.get(first_name).unwrap();

        if config.installation_type == InstallationType::Standalone {
            let mut installer = StandaloneInstaller::new(repo_slug, github_token.clone());
            match installer.get_releases().await {
                Ok(()) => {
                    standalone_installers.insert(repo_slug.to_string(), installer);
                }
                Err(e) => {
                    network_releases.insert(repo_slug.to_string(), Err(e.to_string()));
                }
            }
        } else {
            match release_list(repo_slug, github_token.clone()).await {
                Ok((releases, _)) => {
                    network_releases.insert(repo_slug.to_string(), Ok(releases));
                }
                Err(e) => {
                    network_releases.insert(repo_slug.to_string(), Err(e.to_string()));
                }
            }
        }
    }

    // Build status entries grouped by binary name
    let mut all_entries: BTreeMap<String, Vec<StatusEntry>> = BTreeMap::new();

    for name in &installed_names {
        let config = match registry.get(name) {
            Some(c) => c,
            None => continue,
        };

        let entries = all_entries.entry(name.clone()).or_default();

        if config.installation_type == InstallationType::Standalone || !config.network_based {
            // Standalone binary: find installed version, compare with latest
            let installed_version = find_max_version_for_binary(&release_binaries, name);
            let installed_version = match installed_version {
                Some(v) => v,
                None => continue,
            };

            if let Some(installer) = standalone_installers.get(&config.repository as &str) {
                match installer.latest_version() {
                    Ok(latest) => {
                        let status = if installed_version == latest {
                            UpdateStatus::UpToDate
                        } else {
                            UpdateStatus::UpdateAvailable
                        };
                        entries.push(StatusEntry {
                            network: None,
                            installed_version,
                            latest_version: Some(latest),
                            status,
                        });
                    }
                    Err(e) => {
                        entries.push(StatusEntry {
                            network: None,
                            installed_version,
                            latest_version: None,
                            status: UpdateStatus::FetchError(e.to_string()),
                        });
                    }
                }
            } else if let Some(Err(e)) = network_releases.get(&config.repository as &str) {
                entries.push(StatusEntry {
                    network: None,
                    installed_version,
                    latest_version: None,
                    status: UpdateStatus::FetchError(e.clone()),
                });
            }
        } else {
            // Network-based binary: check each network
            let releases = match network_releases.get(&config.repository as &str) {
                Some(Ok(r)) => r,
                Some(Err(e)) => {
                    // Show error for each network this binary is installed under
                    for (network, network_binaries) in &binaries_by_network {
                        if network_binaries.iter().any(|b| b.binary_name == *name) {
                            let installed_version =
                                find_max_version_in_network(network_binaries, name);
                            entries.push(StatusEntry {
                                network: Some(network.clone()),
                                installed_version: installed_version
                                    .unwrap_or_else(|| "unknown".to_string()),
                                latest_version: None,
                                status: UpdateStatus::FetchError(e.clone()),
                            });
                        }
                    }
                    continue;
                }
                None => continue,
            };

            for (network, network_binaries) in &binaries_by_network {
                // Filter out nightly entries from this network group
                let release_only: Vec<_> = network_binaries
                    .iter()
                    .filter(|b| b.version != "nightly")
                    .cloned()
                    .collect();
                let installed_version = match find_max_version_in_network(&release_only, name) {
                    Some(v) => v,
                    None => continue,
                };

                match last_release_for_network(releases, network).await {
                    Ok((_, latest_version)) => {
                        let status = if installed_version == latest_version {
                            UpdateStatus::UpToDate
                        } else {
                            UpdateStatus::UpdateAvailable
                        };
                        entries.push(StatusEntry {
                            network: Some(network.clone()),
                            installed_version,
                            latest_version: Some(latest_version),
                            status,
                        });
                    }
                    Err(_) => {
                        // No release found for this network -- skip silently
                        // (binary may be installed for a network that no longer has releases)
                    }
                }
            }
        }
    }

    // Append nightly entries at the end of their respective binary groups
    let mut nightly_entries: BTreeMap<String, Vec<StatusEntry>> = BTreeMap::new();
    for b in &nightly_binaries {
        nightly_entries
            .entry(b.binary_name.clone())
            .or_default()
            .push(StatusEntry {
                network: Some(b.network_release.clone()),
                installed_version: b.version.clone(),
                latest_version: None,
                status: UpdateStatus::Nightly,
            });
    }

    // Render output
    let mut total = 0;
    let mut update_count = 0;

    // Collect all binary names that have any entries (release or nightly)
    let mut all_names: Vec<String> = all_entries
        .keys()
        .chain(nightly_entries.keys())
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    all_names.sort();

    for name in &all_names {
        let release = all_entries.get(name).map(|e| e.as_slice()).unwrap_or(&[]);
        let nightly = nightly_entries
            .get(name)
            .map(|e| e.as_slice())
            .unwrap_or(&[]);

        if release.is_empty() && nightly.is_empty() {
            continue;
        }

        let config = registry.get(name);
        let repo = config.map(|c| c.repository.as_str()).unwrap_or("unknown");
        println!("\n{} ({})", name.bold(), repo.dimmed());

        // Compute dynamic column widths from release entries
        let has_network_col = release.iter().any(|e| e.network.is_some());
        let max_network_w = if has_network_col {
            release
                .iter()
                .filter_map(|e| e.network.as_ref())
                .map(|n| n.len())
                .max()
                .unwrap_or(0)
        } else {
            0
        };
        let max_version_w = release
            .iter()
            .map(|e| e.installed_version.len())
            .max()
            .unwrap_or(0);

        // Print release entries with aligned columns
        for entry in release {
            total += 1;
            print_release_entry(
                entry,
                name,
                has_network_col,
                max_network_w,
                max_version_w,
                &mut update_count,
            );
        }

        // Print nightly entries as simple lines
        for entry in nightly {
            let branch = entry.network.as_deref().unwrap_or("unknown");
            println!("  {} {}", branch, "(nightly)".dimmed());
        }
    }

    // Summary
    println!();
    if update_count > 0 {
        println!(
            "{}",
            format!("{update_count} of {total} have updates available.").yellow()
        );
    } else if total > 0 {
        println!("{}", "All binaries are up to date.".green());
    }

    Ok(())
}

fn print_release_entry(
    entry: &StatusEntry,
    binary_name: &str,
    has_network_col: bool,
    max_network_w: usize,
    max_version_w: usize,
    update_count: &mut usize,
) {
    let network_prefix = if has_network_col {
        let label = entry.network.as_deref().unwrap_or("");
        format!("{:<width$}  ", label, width = max_network_w)
    } else {
        String::new()
    };

    match &entry.status {
        UpdateStatus::UpToDate => {
            println!(
                "  {}{:<width$}   {}",
                network_prefix,
                entry.installed_version,
                "up to date".green(),
                width = max_version_w,
            );
        }
        UpdateStatus::UpdateAvailable => {
            *update_count += 1;
            let latest = entry.latest_version.as_deref().unwrap_or("?");
            let update_cmd = match &entry.network {
                Some(network) => {
                    format!("suiup install {}@{}-{}", binary_name, network, latest)
                }
                None => format!("suiup install {}@{}", binary_name, latest),
            };
            println!(
                "  {}{:<width$} {} {}   {}",
                network_prefix,
                entry.installed_version.yellow(),
                "\u{2192}".dimmed(),
                latest.green(),
                update_cmd.dimmed(),
                width = max_version_w,
            );
        }
        UpdateStatus::Nightly => {
            // Nightly entries are rendered separately, not through this function
        }
        UpdateStatus::FetchError(e) => {
            println!(
                "  {}{:<width$}   {} {}",
                network_prefix,
                entry.installed_version,
                "error:".red(),
                e.red(),
                width = max_version_w,
            );
        }
    }
}

/// Parse a version string (with optional leading 'v') into a semver::Version.
fn parse_semver(v: &str) -> Option<semver::Version> {
    semver::Version::parse(v.strip_prefix('v').unwrap_or(v)).ok()
}

/// Compare two version strings using semver, falling back to lexicographic comparison.
fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    match (parse_semver(a), parse_semver(b)) {
        (Some(va), Some(vb)) => va.cmp(&vb),
        _ => a.cmp(b),
    }
}

/// Find the max version of a binary across all networks/entries.
fn find_max_version_for_binary(binaries: &[BinaryVersion], name: &str) -> Option<String> {
    binaries
        .iter()
        .filter(|b| b.binary_name == name)
        .max_by(|a, b| compare_versions(&a.version, &b.version))
        .map(|b| b.version.clone())
}

/// Find the max version of a binary within a specific network's binaries.
fn find_max_version_in_network(network_binaries: &[BinaryVersion], name: &str) -> Option<String> {
    network_binaries
        .iter()
        .filter(|b| b.binary_name == name)
        .max_by(|a, b| compare_versions(&a.version, &b.version))
        .map(|b| b.version.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::BinaryVersion;

    fn make_binary(name: &str, network: &str, version: &str) -> BinaryVersion {
        BinaryVersion {
            binary_name: name.to_string(),
            network_release: network.to_string(),
            version: version.to_string(),
            debug: false,
            path: None,
        }
    }

    #[test]
    fn find_max_version_picks_highest() {
        let binaries = vec![
            make_binary("sui", "testnet", "v1.39.0"),
            make_binary("sui", "testnet", "v1.40.1"),
            make_binary("sui", "testnet", "v1.39.3"),
        ];
        assert_eq!(
            find_max_version_for_binary(&binaries, "sui"),
            Some("v1.40.1".to_string())
        );
    }

    #[test]
    fn find_max_version_filters_by_name() {
        let binaries = vec![
            make_binary("sui", "testnet", "v1.39.0"),
            make_binary("mvr", "standalone", "v9.0.0"),
            make_binary("sui", "testnet", "v1.40.1"),
        ];
        assert_eq!(
            find_max_version_for_binary(&binaries, "sui"),
            Some("v1.40.1".to_string())
        );
    }

    #[test]
    fn find_max_version_uses_semver_not_lexicographic() {
        let binaries = vec![
            make_binary("mvr", "standalone", "v0.0.5"),
            make_binary("mvr", "standalone", "v0.0.13"),
            make_binary("mvr", "standalone", "v0.0.14"),
        ];
        assert_eq!(
            find_max_version_for_binary(&binaries, "mvr"),
            Some("v0.0.14".to_string())
        );
    }

    #[test]
    fn find_max_version_in_network_uses_semver_not_lexicographic() {
        let binaries = vec![
            make_binary("sui", "testnet", "v1.9.0"),
            make_binary("sui", "testnet", "v1.10.1"),
            make_binary("sui", "testnet", "v1.9.3"),
        ];
        assert_eq!(
            find_max_version_in_network(&binaries, "sui"),
            Some("v1.10.1".to_string())
        );
    }

    #[test]
    fn find_max_version_returns_none_when_missing() {
        let binaries = vec![make_binary("sui", "testnet", "v1.39.0")];
        assert_eq!(find_max_version_for_binary(&binaries, "mvr"), None);
    }

    #[test]
    fn find_max_version_returns_none_on_empty() {
        let binaries: Vec<BinaryVersion> = vec![];
        assert_eq!(find_max_version_for_binary(&binaries, "sui"), None);
    }

    #[test]
    fn find_max_version_in_network_picks_highest() {
        let binaries = vec![
            make_binary("sui", "testnet", "v1.39.0"),
            make_binary("sui", "testnet", "v1.40.1"),
            make_binary("walrus", "testnet", "v1.15.0"),
        ];
        assert_eq!(
            find_max_version_in_network(&binaries, "sui"),
            Some("v1.40.1".to_string())
        );
    }

    #[test]
    fn find_max_version_in_network_ignores_other_binaries() {
        let binaries = vec![
            make_binary("sui", "testnet", "v1.39.0"),
            make_binary("walrus", "testnet", "v9.0.0"),
        ];
        assert_eq!(
            find_max_version_in_network(&binaries, "sui"),
            Some("v1.39.0".to_string())
        );
    }

    #[test]
    fn nightly_binaries_are_separated() {
        let binaries = vec![
            make_binary("sui", "testnet", "v1.39.0"),
            make_binary("sui", "main", "nightly"),
            make_binary("mvr", "standalone", "v0.6.4"),
            make_binary("mvr", "main", "nightly"),
        ];

        let nightly: Vec<_> = binaries.iter().filter(|b| b.version == "nightly").collect();
        let release: Vec<_> = binaries.iter().filter(|b| b.version != "nightly").collect();

        assert_eq!(nightly.len(), 2);
        assert_eq!(release.len(), 2);
        assert!(nightly.iter().all(|b| b.version == "nightly"));
        assert!(release.iter().all(|b| b.version != "nightly"));
    }

    #[test]
    fn nightly_excluded_from_max_version() {
        let binaries = vec![
            make_binary("sui", "testnet", "v1.39.0"),
            make_binary("sui", "main", "nightly"),
        ];
        // "nightly" > "v1.39.0" lexicographically, so if not filtered it would win
        let release_only: Vec<_> = binaries
            .into_iter()
            .filter(|b| b.version != "nightly")
            .collect();
        assert_eq!(
            find_max_version_for_binary(&release_only, "sui"),
            Some("v1.39.0".to_string())
        );
    }
}
