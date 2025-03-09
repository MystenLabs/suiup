// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Error;
use flate2::read::GzDecoder;
use futures_util::StreamExt;
use indicatif::HumanBytes;
use indicatif::ProgressBar;
use indicatif::ProgressStyle;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use reqwest::header::ETAG;
use reqwest::header::IF_NONE_MATCH;
use reqwest::header::USER_AGENT;
use reqwest::Client;
use std::collections::HashMap;
use std::collections::HashSet;
#[cfg(not(windows))]
use std::fs::set_permissions;
use std::fs::File;
use std::io::BufReader;
use std::io::Write;
#[cfg(not(windows))]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;
use tar::Archive;
use tracing::debug;

use crate::commands::parse_component_with_version;
use crate::commands::BinaryName;
use crate::commands::CommandMetadata;
use crate::commands::ComponentCommands;
use crate::commands::DefaultCommands;
use crate::mvr;
use crate::paths::*;
use crate::types::Binaries;
use crate::types::BinaryVersion;
use crate::types::InstalledBinaries;
use crate::types::Release;
use crate::types::Version;
use crate::{GITHUB_REPO, RELEASES_ARCHIVES_FOLDER};
use downloader::downloader::Builder;
use downloader::Download;
use std::cmp::min;
use std::env;

pub const WALRUS_BASE_URL: &str = "https://storage.googleapis.com/mysten-walrus-binaries";

fn available_components() -> &'static [&'static str] {
    &["sui", "sui-bridge", "sui-faucet", "walrus", "mvr"]
}

fn install_binary(
    name: &str,
    network: String,
    version: &str,
    debug: bool,
    binary_path: PathBuf,
    yes: bool,
) -> Result<(), Error> {
    let mut installed_binaries = InstalledBinaries::new()?;
    installed_binaries.add_binary(BinaryVersion {
        binary_name: name.to_string(),
        network_release: network.clone(),
        version: version.to_string(),
        debug,
        path: Some(binary_path.to_string_lossy().to_string()),
    });
    installed_binaries.save_to_file()?;
    update_after_install(&vec![name.to_string()], network, version, debug, yes)?;
    Ok(())
}

// this is used for sui mostly
async fn install_from_release(
    name: &str,
    network: &str,
    version_spec: Option<String>,
    debug: bool,
    yes: bool,
) -> Result<(), Error> {
    let filename = match version_spec {
        Some(version) => download_release_at_version(network, &version).await?,
        None => download_latest_release(network).await?,
    };

    let version = extract_version_from_release(&filename)?;
    let binary_name = if debug && name == "sui" {
        format!("{}-debug", name)
    } else {
        name.to_string()
    };

    if !check_if_binaries_exist(&binary_name, network.to_string(), &version)? {
        println!("Adding binary: {name}-{version}");
        extract_component(&binary_name, network.to_string(), &filename)?;

        let binary_filename = format!("{}-{}", name, version);
        #[cfg(target_os = "windows")]
        let binary_filename = format!("{}.exe", binary_filename);

        let binary_path = binaries_folder()?.join(network).join(binary_filename);
        install_binary(name, network.to_string(), &version, debug, binary_path, yes)?;
    } else {
        println!("Binary {name}-{version} already installed. Use `suiup default set` to change the default binary.");
    }
    Ok(())
}

/// Compile the code from the main branch or the specified branch.
/// It checks if cargo is installed.
async fn install_from_nightly(
    name: &BinaryName,
    branch: &str,
    debug: bool,
    yes: bool,
) -> Result<(), Error> {
    println!("Installing {name} from {branch} branch");
    check_cargo_rust_installed()?;

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.green} {msg}")
            .unwrap()
            .tick_strings(&["-", "\\", "|", "/"]),
    );
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_message("Compiling...please wait");

    let repo_url = name.repo_url();
    let binaries_folder = binaries_folder()?;
    let binaries_folder_branch = binaries_folder.join(branch);
    let args = vec![
        "install",
        "--locked",
        "--force",
        "--git",
        repo_url,
        "--branch",
        branch,
        name.to_str(),
        "--root",
        binaries_folder_branch.to_str().unwrap(),
    ];
    let mut cmd = Command::new("cargo");
    cmd.args(&args);

    let cmd = cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::piped())
        .spawn()?;

    let output = cmd.wait_with_output()?;
    pb.finish_with_message("Done!");

    if !output.status.success() {
        let error_message = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Error during installation:\n{}", error_message));
    }

    println!("Installation completed successfully!");
    // bin folder is needed because cargo installs in  /folder/bin/binary_name.
    let orig_binary_path = binaries_folder_branch.join("bin").join(name.to_str());

    // rename the binary to `binary_name-nightly`, to keep things in sync across the board

    let dst_name = if debug {
        format!("{}-debug-nightly", name.to_str())
    } else {
        format!("{}-nightly", name.to_str())
    };
    let dst = binaries_folder_branch.join("bin").join(dst_name);

    #[cfg(windows)]
    let orig_binary_path = orig_binary_path.with_extension("exe");
    #[cfg(windows)]
    let dst = dst.with_extension("exe");

    std::fs::rename(&orig_binary_path, &dst)?;
    install_binary(
        name.to_str(),
        branch.to_string(),
        "nightly",
        debug,
        dst,
        yes,
    )?;

    Ok(())
}

async fn install_walrus(network: String, yes: bool) -> Result<(), Error> {
    if !check_if_binaries_exist("walrus", network.clone(), "latest")? {
        println!("Adding component: walrus-latest");
        let (os, arch) = detect_os_arch()?;
        let download_dir = binaries_folder()?.join(network.clone());
        let download_to = download_dir.join("walrus-latest");
        download_file(
            &format!(
                "{}/walrus-{network}-latest-{os}-{arch}",
                WALRUS_BASE_URL,
                network = network
            ),
            &download_to,
            "walrus-latest",
        )
        .await?;

        let filename = "walrus";

        #[cfg(target_os = "windows")]
        let filename = format!("walrus.exe");

        install_binary(&filename, network, "latest", false, binaries_folder()?, yes)?;
    } else {
        println!("Binary walrus-latest already installed");
    }
    Ok(())
}

/// Install MVR CLI
async fn install_mvr(version: Option<String>, yes: bool) -> Result<(), Error> {
    let network = "standalone".to_string();
    let binary_name = BinaryName::Mvr.to_string();
    if !check_if_binaries_exist(
        &binary_name,
        network.clone(),
        &version.clone().unwrap_or_default(),
    )? {
        let mut installer = mvr::MvrInstaller::new();
        let installed_version = installer.download_version(version).await?;

        println!("Adding component: mvr-{installed_version}");

        let binary_path = binaries_folder()?
            .join(&network)
            .join(format!("{}-{}", binary_name, installed_version));
        install_binary(
            &binary_name,
            network,
            &installed_version,
            false,
            binary_path,
            yes,
        )?;
    } else {
        let version = version.unwrap_or_default();
        println!("Binary mvr-{version} already installed. Use `suiup default set mvr {version}` to set the default version to the specified one.");
    }

    Ok(())
}

// Main component handling function
pub(crate) async fn handle_component(cmd: ComponentCommands) -> Result<(), Error> {
    match cmd {
        ComponentCommands::List => {
            let components = available_components();
            println!("Available binaries to install:");
            for component in components {
                println!(" - {}", component);
            }
        }
        ComponentCommands::Add {
            components,
            nightly,
            debug,
            yes,
        } => {
            if components.is_empty() {
                print!("No components provided. Use `suiup list` to see available components.");
                return Ok(());
            }

            // Ensure installation directories exist
            let default_bin_dir = get_default_bin_dir();
            std::fs::create_dir_all(&default_bin_dir)?;

            let installed_bins_dir = binaries_folder()?;
            std::fs::create_dir_all(&installed_bins_dir)?;

            let components = components.join(" ");
            let component =
                parse_component_with_version(&components).map_err(|e| anyhow!("{e}"))?;

            let name = component.name;
            let network = component.network;
            let version = component.version;
            let available_components = available_components();
            if !available_components.contains(&name.to_string().as_str()) {
                bail!("Binary {} does not exist", name);
            }

            if name != BinaryName::Sui && debug && nightly.is_none() {
                bail!("Debug flag is only available for the `sui` binary");
            }

            if nightly.is_some() && version.is_some() {
                bail!("Cannot install from nightly and a release at the same time. Remove the version or the nightly flag");
            }

            match (&name, &nightly) {
                (BinaryName::Walrus, _) => {
                    std::fs::create_dir_all(&installed_bins_dir.join(network.clone()))?;
                    install_walrus(network, yes).await?;
                }
                (BinaryName::Mvr, nightly) => {
                    std::fs::create_dir_all(&installed_bins_dir.join("standalone"))?;
                    if let Some(branch) = nightly {
                        install_from_nightly(&name, branch, debug, yes).await?;
                    } else {
                        install_mvr(version, yes).await?;
                    }
                }
                (_, Some(branch)) => {
                    install_from_nightly(&name, branch, debug, yes).await?;
                }
                _ => {
                    install_from_release(&name.to_string().as_str(), &network, version, debug, yes)
                        .await?;
                }
            }
        }
        ComponentCommands::Remove { binary } => {
            let mut installed_binaries = InstalledBinaries::new()?;

            let binaries_to_remove = installed_binaries
                .binaries()
                .iter()
                .filter(|b| binary.to_string() == b.binary_name)
                .collect::<Vec<_>>();

            if binaries_to_remove.is_empty() {
                println!("No binaries found to remove");
                return Ok(());
            }

            println!("Binaries to remove: {binaries_to_remove:?}");

            for p in &binaries_to_remove {
                if let Some(p) = p.path.as_ref() {
                    if !PathBuf::from(p).exists() {
                        println!("Binary {p} does not exist. Aborting the command.");

                        return Ok(());
                    }
                }
            }

            let default_file = default_file_path()?;
            let default = std::fs::read_to_string(&default_file)
                .map_err(|_| anyhow!("Cannot read file {}", default_file.display()))?;
            let mut default_binaries: HashMap<String, (String, Version, bool)> =
                serde_json::from_str(&default).map_err(|_| {
                    anyhow!("Cannot decode default binary file to JSON. Is the file corrupted?")
                })?;

            // Remove the installed binaries folder
            for binary in &binaries_to_remove {
                if let Some(p) = binary.path.as_ref() {
                    println!("Found binary path: {p}");
                    debug!("Removing binary: {p}");
                    std::fs::remove_file(p).map_err(|e| anyhow!("Cannot remove file: {e}"))?;
                    debug!("File removed: {p}");
                    println!("Removed binary: {} from {p}", binary.binary_name);
                }
            }

            // Remove the binaries from the default-bin folder
            let default_binaries_to_remove = binaries_to_remove
                .iter()
                .map(|x| &x.binary_name)
                .collect::<HashSet<_>>();
            for binary in default_binaries_to_remove {
                let default_bin_path = default_bin_folder()
                    .map_err(|e| anyhow!("Cannot find the default-bin folder: {e}"))?
                    .join(&binary);
                if default_bin_path.exists() {
                    std::fs::remove_file(default_bin_path)
                        .map_err(|e| anyhow!("Cannot remove file: {e}"))?;
                }

                default_binaries.remove(binary);
                debug!("Removed {binary} from default binaries JSON file");
            }

            // Remove from default binaries metadata file
            File::create(&default_file)
                .map_err(|_| anyhow!("Cannot create file: {}", default_file.display()))?
                .write_all(serde_json::to_string_pretty(&default_binaries)?.as_bytes())?;

            // Remove from installed_binaries metadata file
            installed_binaries.remove_binary(&binary.to_string());
            debug!("Removed {binary} from installed_binaries JSON file. Saving updated data");
            // Save file
            installed_binaries.save_to_file()?;
        }
    }
    Ok(())
}

/// Handles the default commands
pub(crate) fn handle_default(cmd: DefaultCommands) -> Result<(), Error> {
    match cmd {
        DefaultCommands::Get => {
            // let default_binaries = DefaultBinaries::load()?;
            let default = std::fs::read_to_string(default_file_path()?)?;
            let default: HashMap<String, (String, Version, bool)> = serde_json::from_str(&default)?;
            let default_binaries = Binaries::from(default);
            println!("\x1b[1mDefault binaries:\x1b[0m\n{default_binaries}");
        }

        DefaultCommands::Set {
            name,
            debug,
            nightly,
        } => {
            if name.len() != 2 && nightly.is_none() {
                bail!("Invalid number of arguments. Version is required: 'sui testnet-v1.39.3', 'sui testnet' -- this will use an installed binary that has the higest testnet version. \n For `mvr` only pass the version: `mvr v0.0.5`")
            }

            let CommandMetadata {
                name,
                network,
                version,
            } = parse_component_with_version(&name.join(" "))?;

            let network = if name == BinaryName::Mvr {
                if let Some(ref nightly) = nightly {
                    nightly
                } else {
                    "standalone"
                }
            } else {
                &network
            };

            // a map of network --> to BinaryVersion
            let installed_binaries = installed_binaries_grouped_by_network(None)?;
            let binaries = installed_binaries
                .get(network)
                .ok_or_else(|| anyhow!("No binaries installed for {network}"))?;

            // Check if the binary exists in any network
            let binary_exists = installed_binaries
                .values()
                .any(|bins| bins.iter().any(|x| x.binary_name == name.to_string()));
            if !binary_exists {
                bail!("Binary {name} not found in installed binaries. Use `suiup show` to see installed binaries.");
            }

            let version = if let Some(version) = version {
                version
            } else {
                binaries
                    .iter()
                    .filter(|b| b.binary_name == name.to_string())
                    .max_by(|a, b| a.version.cmp(&b.version))
                    .map(|b| b.version.clone())
                    .ok_or_else(|| anyhow!("No version found for {name} in {network}"))?
            };

            // check if the binary for this network and version exists
            let binary_version = format!("{}-{}", name, version);
            debug!("Checking if {binary_version} exists");
            binaries
                .iter()
                .find(|b| {
                    b.binary_name == name.to_string() && b.version == version && b.network_release == network
                })
                .ok_or_else(|| {
                    anyhow!("Binary {binary_version} from {network} release not found. Use `suiup show` to see installed binaries.")
                })?;

            // copy files to default-bin
            let mut dst = default_bin_folder()
                .map_err(|e| anyhow::anyhow!("Cannot find the default bin folder: {e}"))?;
            let name = if debug {
                format!("{}-debug", name)
            } else {
                format!("{}", name)
            };

            dst.push(&name.to_string());

            #[cfg(target_os = "windows")]
            dst.set_extension("exe");

            let mut src = binaries_folder()
                .map_err(|e| anyhow::anyhow!("Cannot find the binaries folder: {e}"))?;
            src.push(network.to_string());

            if nightly.is_some() {
                // cargo install adds a bin folder to the specified path :-)
                src.push("bin");
            }

            if debug {
                src.push(format!("{}-debug-{}", name, version));
            } else {
                src.push(binary_version);
            }

            #[cfg(target_os = "windows")]
            src.set_extension("exe");

            #[cfg(not(target_os = "windows"))]
            {
                if dst.exists() {
                    std::fs::remove_file(&dst)?;
                }

                std::fs::copy(&src, &dst)?;

                #[cfg(unix)]
                {
                    let mut perms = std::fs::metadata(&dst)?.permissions();
                    perms.set_mode(0o755);
                    std::fs::set_permissions(&dst, perms)?;
                }
            }

            #[cfg(target_os = "windows")]
            {
                std::fs::copy(&src, &dst)?;
            }

            update_default_version_file(
                &vec![name.to_string()],
                network.to_string(),
                &version,
                debug,
            )?;
            println!("Default binary updated successfully");
        }
    }

    Ok(())
}

/// Handles the `show` command
pub fn handle_show() -> Result<(), Error> {
    let default = std::fs::read_to_string(default_file_path()?)?;
    let default: HashMap<String, (String, Version, bool)> = serde_json::from_str(&default)?;
    let default_binaries = Binaries::from(default);

    println!("\x1b[1mDefault binaries:\x1b[0m\n{default_binaries}");

    let installed_binaries = installed_binaries_grouped_by_network(None)?;

    println!("\x1b[1mInstalled binaries:\x1b[0m");

    for (network, binaries) in installed_binaries {
        println!("[{network} release/branch]");
        for binary in binaries {
            println!("    {binary}");
        }
    }

    Ok(())
}

/// Handles the `update` command
pub async fn handle_update(binary_name: Vec<String>, yes: bool) -> Result<(), Error> {
    if binary_name.is_empty() || binary_name.len() > 2 {
        bail!("Invalid number of arguments for `update` command");
    }

    let CommandMetadata { name, version, .. } =
        parse_component_with_version(&binary_name.join(" "))?;

    if version.is_some() {
        bail!("Update should be done without a version. Use `suiup install` to specify a version");
    }

    if !available_components().contains(&name.to_str()) {
        bail!("Invalid component name: {}", name);
    }

    let installed_binaries = InstalledBinaries::new()?;
    let binaries = installed_binaries.binaries();
    if !binaries.iter().any(|x| x.binary_name == name.to_str()) {
        bail!("Binary {name} not found in installed binaries. Use `suiup show` to see installed binaries and `suiup install` to install the binary.")
    }
    let binaries_by_network = installed_binaries_grouped_by_network(Some(installed_binaries))?;

    let mut network_local_last_version: Vec<(String, String)> = vec![];

    for (network, binaries) in &binaries_by_network {
        let last_version = binaries
            .iter()
            .filter(|x| x.binary_name == name.to_str())
            .collect::<Vec<_>>();
        if last_version.is_empty() {
            continue;
        }
        let last_version = if last_version.len() > 1 {
            last_version
                .iter()
                .max_by(|a, b| a.version.cmp(&b.version))
                .unwrap()
        } else {
            last_version.first().unwrap()
        };
        network_local_last_version.push((network.clone(), last_version.version.clone()));
    }
    // map of network and last version known locally

    // find the last local version of the name binary, for each network
    // then find the last release for each network and compare the versions

    if name == BinaryName::Mvr {
        handle_component(ComponentCommands::Add {
            components: binary_name,
            debug: false,
            nightly: None,
            yes,
        })
        .await?;
        return Ok(());
    }

    if name == BinaryName::Walrus {
        handle_component(ComponentCommands::Add {
            components: binary_name,
            debug: false,
            nightly: None,
            yes,
        })
        .await?;
        return Ok(());
    }

    let releases = release_list().await?.0;
    let mut to_update = vec![];
    for (n, v) in &network_local_last_version {
        let last_release = last_release_for_network(&releases, &n).await?;
        let last_version = last_release.1;
        if v == &last_version {
            println!("[{n} release] {name} is up to date");
        } else {
            println!("[{n} release] {name} is outdated. Local: {v}, Latest: {last_version}");
            to_update.push((n, last_version));
        }
    }

    for (n, v) in to_update.iter() {
        println!("Updating {name} to {v} from {n} release");
        handle_component(ComponentCommands::Add {
            components: binary_name.clone(),
            debug: false,
            nightly: None,
            yes,
        })
        .await?;
    }

    Ok(())
}

/// Handles the `which` command
pub fn handle_which() -> Result<(), Error> {
    let default_bin = default_bin_folder()?;
    println!("Default binaries path: {}", default_bin.display());
    // Implement displaying the path to the active CLI binary here
    Ok(())
}

/// Fetches the list of releases from the GitHub repository
pub(crate) async fn release_list() -> Result<(Vec<Release>, Option<String>), anyhow::Error> {
    let client = Client::new();
    let release_url = format!("https://api.github.com/repos/{}/releases", GITHUB_REPO);
    let mut request = client.get(&release_url).header("User-Agent", "suiup");
    // Add ETag for caching
    if let Ok(etag) = read_etag_file() {
        request = request.header(IF_NONE_MATCH, etag);
    }

    let response = request.send().await?;

    // note this only works with authenticated requests. Should add support for that later.
    if response.status() == reqwest::StatusCode::NOT_MODIFIED {
        // If nothing has changed, return an empty list and the existing ETag
        if let Some((releases, etag)) = load_cached_release_list()
            .map_err(|e| anyhow!("Cannot load release list from cache: {e}"))?
        {
            return Ok((releases, Some(etag)));
        }
    }

    let etag = response
        .headers()
        .get(ETAG)
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let response = response.error_for_status();
    if let Err(ref e) = response {
        bail!("Error: {e}");
    }
    let releases: Vec<Release> = response.unwrap().json().await?;
    save_release_list(&releases, etag.clone())?;

    Ok((releases, etag))
}

fn cache_path() -> PathBuf {
    let cache_dir = dirs::cache_dir().expect("Could not find cache directory");
    if !cache_dir.exists() {
        std::fs::create_dir_all(&cache_dir.join("suiup"))
            .expect("Could not create cache directory");
    }
    cache_dir.join("suiup")
}

fn load_cached_release_list() -> Result<Option<(Vec<Release>, String)>, anyhow::Error> {
    let cache_file = cache_path().join("releases.json");
    let etag_file = cache_path().join("etag.txt");

    if cache_file.exists() && etag_file.exists() {
        println!("Loading releases list from cache");
        let cache_content: Vec<Release> = serde_json::from_str(
            &std::fs::read_to_string(&cache_file)
                .map_err(|_| anyhow!("Cannot read from file {}", cache_file.display()))?,
        )
        .map_err(|_| {
            anyhow!(
                "Cannot deserialize the releases cached file {}",
                cache_file.display()
            )
        })?;
        let etag_content = std::fs::read_to_string(&etag_file)
            .map_err(|_| anyhow!("Cannot read from file {}", etag_file.display()))?;

        Ok(Some((cache_content, etag_content)))
    } else {
        Ok(None)
    }
}

fn save_release_list(releases: &[Release], etag: Option<String>) -> Result<(), anyhow::Error> {
    println!("Saving releases list to cache");
    let cache_dir = cache_path();
    std::fs::create_dir_all(&cache_dir).expect("Could not create cache directory");

    let cache_file = cache_dir.join("releases.json");
    let etag_file = cache_dir.join("etag.txt");

    let cache_content =
        serde_json::to_string_pretty(releases).expect("Could not serialize releases file: {}");

    std::fs::write(&cache_file, cache_content).map_err(|_| {
        anyhow!(
            "Could not write cache releases file: {}",
            cache_file.display(),
        )
    })?;
    if let Some(etag) = etag {
        std::fs::write(&etag_file, etag)
            .map_err(|_| anyhow!("Could not write ETag file: {}", etag_file.display()))?;
    }
    Ok(())
}

fn read_etag_file() -> Result<String, anyhow::Error> {
    let etag_file = cache_path().join("etag.txt");
    if etag_file.exists() {
        let etag = std::fs::read_to_string(&etag_file)
            .map_err(|_| anyhow!("Cannot read from file {}", etag_file.display()));
        etag
    } else {
        Ok("".to_string())
    }
}

/// Finds the last release for a given network
async fn find_last_release_by_network(releases: Vec<Release>, network: &str) -> Option<Release> {
    releases
        .into_iter()
        .find(|r| r.assets.iter().any(|a| a.name.contains(network)))
}

/// Detects the current OS and architecture
pub fn detect_os_arch() -> Result<(String, String), Error> {
    let os = match whoami::platform() {
        whoami::Platform::Linux => "ubuntu",
        whoami::Platform::Windows => "windows",
        whoami::Platform::MacOS => "macos",
        _ => anyhow::bail!("Unsupported OS. Supported only: Linux, Windows, MacOS"),
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" if os == "macos" => "arm64",
        "aarch64" => "aarch64",
        _ => anyhow::bail!("Unsupported architecture. Supported only: x86_64, aarch64"),
    };

    println!("Detected: {os}-{arch}...");
    Ok((os.to_string(), arch.to_string()))
}

/// Downloads a release with a specific version
/// The network is used to filter the release
async fn download_release_at_version(
    network: &str,
    version: &str,
) -> Result<String, anyhow::Error> {
    let (os, arch) = detect_os_arch()?;

    let tag = format!("{}-{}", network, version);

    println!("Searching for release with tag: {}...", tag);
    let client = reqwest::Client::new();
    let mut headers = HeaderMap::new();

    let releases = release_list().await?.0;

    if let Some(release) = releases
        .iter()
        .find(|r| r.assets.iter().any(|a| a.name.contains(&tag)))
    {
        download_asset_from_github(release, &os, &arch).await
    } else {
        headers.insert(USER_AGENT, HeaderValue::from_static("suiup"));

        let url = format!(
            "https://api.github.com/repos/{GITHUB_REPO}/releases/tags/{}",
            tag
        );
        let response = client.get(&url).headers(headers).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("release {tag} not found");
        }

        let release: Release = response.json().await?;
        download_asset_from_github(&release, &os, &arch).await
    }
}

/// Downloads the latest release for a given network
async fn download_latest_release(network: &str) -> Result<String, anyhow::Error> {
    println!("Downloading release list");
    let releases = release_list().await?;

    let (os, arch) = detect_os_arch()?;

    let last_release = find_last_release_by_network(releases.0, network)
        .await
        .ok_or_else(|| anyhow!("Could not find last release"))?;

    println!(
        "Last {network} release: {}",
        extract_version_from_release(&last_release.assets[0].name)?
    );

    download_asset_from_github(&last_release, &os, &arch).await
}

pub async fn download_file(url: &str, download_to: &PathBuf, name: &str) -> Result<String, Error> {
    let client = Client::new();
    let response = client.get(url).header("User-Agent", "suiup").send().await?;

    let response = response.error_for_status();

    if let Err(ref e) = response {
        bail!("Encountered unexpected error: {e}");
    }

    let response = response.unwrap();

    if !response.status().is_success() {
        return Err(anyhow!("Failed to download: {}", response.status()));
    }

    let mut total_size = response.content_length().unwrap_or_else(|| 0);
    //walrus is on google storage, so different content length header
    if total_size == 0 {
        total_size = response
            .headers()
            .get("x-goog-stored-content-length")
            .and_then(|c| c.to_str().ok())
            .and_then(|c| c.parse::<u64>().ok())
            .unwrap_or(0);
    }

    if download_to.exists() {
        if download_to.metadata()?.len() == total_size {
            println!("Found {name} in cache");
            return Ok(name.to_string());
        } else {
            std::fs::remove_file(&download_to)?;
        }
    }

    let mut dl = Builder::default()
        .download_folder(download_to.parent().unwrap())
        .build()
        .unwrap();

    let response = dl.async_download(&[Download::new(url)]).await;
    // response.iter().for_each(|v| match v {
    //     Ok(v) => println!("Downloaded: {:?}", v),
    //     Err(e) => println!("Error: {:?}", e),
    // });

    Ok(name.to_string())
}

pub async fn download_file1(url: &str, download_to: &PathBuf, name: &str) -> Result<String, Error> {
    let client = Client::new();
    let response = client.get(url).header("User-Agent", "suiup").send().await?;

    let response = response.error_for_status();

    if let Err(ref e) = response {
        bail!("Encountered unexpected error: {e}");
    }

    let response = response.unwrap();

    if !response.status().is_success() {
        return Err(anyhow!("Failed to download: {}", response.status()));
    }

    let mut total_size = response.content_length().unwrap_or_else(|| 0);
    //walrus is on google storage, so different content length header
    if total_size == 0 {
        total_size = response
            .headers()
            .get("x-goog-stored-content-length")
            .and_then(|c| c.to_str().ok())
            .and_then(|c| c.parse::<u64>().ok())
            .unwrap_or(0);
    }

    if download_to.exists() {
        if download_to.metadata()?.len() == total_size {
            println!("Found {name} in cache");
            return Ok(name.to_string());
        } else {
            std::fs::remove_file(&download_to)?;
        }
    }

    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
            .template("Downloading release: {spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta}) {msg}")
            .unwrap()
            .progress_chars("=>-"));

    let mut file = std::fs::File::create(&download_to)?;
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();
    let start = Instant::now();

    while let Some(item) = stream.next().await {
        let chunk = item?;
        file.write_all(&chunk)?;
        let new = min(downloaded + (chunk.len() as u64), total_size);
        downloaded = new;
        pb.set_position(new);

        let elapsed = start.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            let speed = downloaded as f64 / elapsed;
            pb.set_message(format!("Speed: {}/s", HumanBytes(speed as u64)));
        }
    }

    pb.finish_with_message("Download complete");

    Ok(name.to_string())
}

/// Downloads the archived release from GitHub and returns the file name
/// The `network, os, and arch` parameters are used to retrieve the correct release for the target
/// architecture and OS
async fn download_asset_from_github(
    release: &Release,
    os: &str,
    arch: &str,
) -> Result<String, anyhow::Error> {
    let asset = release
        .assets
        .iter()
        .find(|&a| a.name.contains(arch) && a.name.contains(os.to_string().to_lowercase().as_str()))
        .ok_or_else(|| anyhow!("Asset not found for {os}-{arch}"))?;

    let url = asset.clone().browser_download_url;
    let name = asset.clone().name;
    let path = release_archive_folder()?;
    let mut file_path = path.clone();
    file_path.push(&asset.name);

    download_file(&url, &file_path, &name).await
}

/// Extracts a component from the release archive. The component's name is identified by the
/// `binary` parameter.
///
/// This extracts the component to the binaries folder under the network from which release comes
/// from, and sets the correct permissions for Unix based systems.
fn extract_component(orig_binary: &str, network: String, filename: &str) -> Result<(), Error> {
    let mut archive_path = release_archive_folder()?;
    archive_path.push(filename);

    let file = File::open(archive_path.as_path())
        .map_err(|_| anyhow!("Cannot open archive file: {}", archive_path.display()))?;
    let tar = GzDecoder::new(file);
    let mut archive = Archive::new(tar);

    let binary = format!("{orig_binary}");
    #[cfg(windows)]
    let binary = format!("{}.exe", orig_binary);

    // Check if the current entry matches the file name
    for file in archive.entries()? {
        let mut f = file.unwrap();
        if f.path()?.file_name().map(|x| x.to_str()).flatten() == Some(&binary) {
            println!("Extracting file: {}", &binary);

            let mut output_path = binaries_folder()?;
            output_path.push(network.to_string());
            if !output_path.is_dir() {
                std::fs::create_dir_all(output_path.as_path())?;
            }
            let version = extract_version_from_release(filename)?;
            let binary_version = format!("{}-{}", orig_binary, version);
            #[cfg(not(windows))]
            output_path.push(&binary_version);
            #[cfg(windows)]
            output_path.push(&format!("{}.exe", binary_version));

            let mut output_file = File::create(&output_path)?;

            std::io::copy(&mut f, &mut output_file)?;
            println!(" '{}' extracted successfully!", &binary);
            #[cfg(not(target_os = "windows"))]
            {
                // Retrieve and apply the original file permissions on Unix-like systems
                if let Ok(permissions) = f.header().mode() {
                    set_permissions(output_path, PermissionsExt::from_mode(permissions))?;
                }
            }
            break;
        }
    }

    Ok(())
}

/// Extracts the version from a release filename
pub(crate) fn extract_version_from_release(release: &str) -> Result<String, Error> {
    let re = regex::Regex::new(r"v\d+\.\d+\.\d+").unwrap();
    let captures = re
        .captures(release)
        .ok_or_else(|| anyhow!("Could not extract version from release"))?;

    Ok(captures.get(0).unwrap().as_str().to_string())
}

/// Returns the path to the default binaries folder. The folder is created if it does not exist.
fn default_bin_folder() -> Result<PathBuf, anyhow::Error> {
    let path = get_default_bin_dir();
    if !path.is_dir() {
        std::fs::create_dir_all(path.as_path())
            .map_err(|e| anyhow!("Could not create directory {} due to {e}", path.display()))?;
    }

    Ok(path)
}

/// Returns the path to the releases archives folder. The folder is created if it does not exist.
/// This is used to cache the release archives.
pub fn release_archive_folder() -> Result<PathBuf, anyhow::Error> {
    let mut path = get_suiup_cache_dir();
    path.push(RELEASES_ARCHIVES_FOLDER);
    if !path.is_dir() {
        std::fs::create_dir_all(path.as_path())
            .map_err(|e| anyhow!("Could not create directory {} due to {e}", path.display()))?;
    }
    Ok(path)
}

/// Returns the path to the binaries folder. The folder is created if it does not exist.
pub fn binaries_folder() -> Result<PathBuf, anyhow::Error> {
    let mut path = get_suiup_data_dir();
    path.push("binaries");
    if !path.is_dir() {
        std::fs::create_dir_all(path.as_path())
            .map_err(|e| anyhow!("Could not create directory {} due to {e}", path.display()))?;
    }
    Ok(path)
}

/// Prompts the user and asks if they want to update the default version with the one that was just
/// installed.
fn update_after_install(
    name: &Vec<String>,
    network: String,
    version: &str,
    debug: bool,
    yes: bool,
) -> Result<(), Error> {
    let input = if yes {
        "y".to_string()
    } else {
        let prompt = "Do you want to set this new installed version as the default one? [y/N] ";

        print!("{prompt}");
        std::io::stdout().flush().unwrap();

        // Create a mutable String to store the input
        let mut input = String::new();

        // Read the input from the console
        std::io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");

        // Trim the input and convert to lowercase for comparison
        input.trim().to_lowercase()
    };

    // Check the user's response
    match input.as_str() {
        "y" | "yes" => {
            for binary in name {
                let mut filename = if debug {
                    format!("{}-debug-{}", binary, version)
                } else {
                    format!("{}-{}", binary, version)
                };

                if version.is_empty() {
                    filename = filename.strip_suffix('-').unwrap_or_default().to_string();
                }

                let binary_folder = if version == "nightly" {
                    binaries_folder()?.join(network.to_string()).join("bin")
                } else {
                    binaries_folder()?.join(network.to_string())
                };

                if !binary_folder.exists() {
                    std::fs::create_dir_all(&binary_folder)?;
                }

                #[cfg(target_os = "windows")]
                let filename = format!("{}.exe", filename);

                println!(
                    "Installing binary to {}/{}",
                    binary_folder.display(),
                    filename
                );

                let src = binary_folder.join(&filename);
                let dst = default_bin_folder()?.join(binary);

                println!("Setting {} as default", &filename);

                #[cfg(target_os = "windows")]
                let mut dst = dst.clone();
                #[cfg(target_os = "windows")]
                dst.set_extension("exe");

                std::fs::copy(&src, &dst).map_err(|e| {
                    anyhow!(
                        "Error copying the binary to the default folder (src: {}, dst: {}): {e}",
                        src.display(),
                        dst.display()
                    )
                })?;

                #[cfg(unix)]
                {
                    let mut perms = std::fs::metadata(&dst)?.permissions();
                    perms.set_mode(0o755);
                    std::fs::set_permissions(&dst, perms)?;
                }

                println!("[{network}] {binary}-{version} set as default");
            }
            update_default_version_file(name, network, version, debug)?;
            check_path_and_warn()?;
        }

        "" | "n" | "no" => {
            println!("Keeping the current default version.");
        }
        _ => {
            println!("Invalid input. Please enter 'y' or 'n'.");
            update_after_install(name, network, version, debug, yes)?;
        }
    }
    Ok(())
}

/// Updates the default version file with the new installed version.
fn update_default_version_file(
    binaries: &Vec<String>,
    network: String,
    version: &str,
    debug: bool,
) -> Result<(), Error> {
    let path = default_file_path()?;
    let file = File::open(&path)?;
    let reader = BufReader::new(file);
    println!("Updating default version file...");
    let mut map: HashMap<String, (String, Version, bool)> = serde_json::from_reader(reader)?;

    for binary in binaries {
        let b = map.get_mut(binary);
        if let Some(b) = b {
            b.0 = network.clone();
            b.1 = version.to_string();
            b.2 = debug;
        } else {
            map.insert(
                binary.to_string(),
                (network.clone(), version.to_string(), debug),
            );
        }
    }

    let mut file = File::create(path)?;
    file.write_all(serde_json::to_string_pretty(&map)?.as_bytes())?;

    Ok(())
}

/// Checks if the binaries exist in the binaries folder
fn check_if_binaries_exist(binary: &str, network: String, version: &str) -> Result<bool, Error> {
    let mut path = binaries_folder()?;
    path.push(network.to_string());

    let binary_version = if version.is_empty() {
        format!("{}", binary)
    } else {
        format!("{}-{}", binary, version)
    };

    #[cfg(target_os = "windows")]
    {
        path.push(format!("{}.exe", binary_version));
    }
    path.push(&binary_version);
    Ok(path.exists())
}

/// Returns the path to the default version file. The file is created if it does not exist.
pub fn default_file_path() -> Result<PathBuf, Error> {
    let path = get_config_file("default_version.json");
    if !path.exists() {
        let mut file = File::create(&path)?;
        let default = HashMap::<String, (String, String)>::new();
        let default_str = serde_json::to_string_pretty(&default)?;
        file.write_all(default_str.as_bytes())?;
    }
    Ok(path)
}

/// Returns a map of installed binaries grouped by network releases
fn installed_binaries_grouped_by_network(
    installed_binaries: Option<InstalledBinaries>,
) -> Result<HashMap<String, Vec<BinaryVersion>>, Error> {
    let installed_binaries = if let Some(installed_binaries) = installed_binaries {
        installed_binaries
    } else {
        InstalledBinaries::new()?
    };
    let binaries = installed_binaries.binaries();
    let mut files_by_folder: HashMap<String, Vec<BinaryVersion>> = HashMap::new();

    for b in binaries {
        if let Some(f) = files_by_folder.get_mut(&b.network_release.to_string()) {
            f.push(b.clone());
        } else {
            files_by_folder.insert(b.network_release.to_string(), vec![b.clone()]);
        }
    }

    Ok(files_by_folder)
}

pub(crate) fn installed_binaries_file() -> Result<PathBuf, Error> {
    let path = get_config_file("installed_binaries.json");
    if !path.exists() {
        InstalledBinaries::create_file(&path)?;
    }
    Ok(path)
}

pub(crate) fn initialize() -> Result<(), Error> {
    std::fs::create_dir_all(get_suiup_config_dir())?;
    std::fs::create_dir_all(get_suiup_data_dir())?;
    std::fs::create_dir_all(get_suiup_cache_dir())?;
    binaries_folder()?;
    release_archive_folder()?;
    default_bin_folder()?;
    default_file_path()?;
    installed_binaries_file()?;
    Ok(())
}

pub(crate) async fn last_release_for_network<'a>(
    releases: &'a [Release],
    network: &'a str,
) -> Result<(&'a str, String), Error> {
    if let Some(release) = releases
        .iter()
        .find(|r| r.assets.iter().any(|a| a.name.contains(network)))
    {
        Ok((
            network,
            extract_version_from_release(release.assets[0].name.as_str())?,
        ))
    } else {
        bail!("No release found for {network}")
    }
}

fn check_cargo_rust_installed() -> Result<(), Error> {
    if let Ok(output) = Command::new("rustc").arg("--version").output() {
        if output.status.success() {
            print!(
                "Rust is installed: {}",
                String::from_utf8_lossy(&output.stdout)
            );
        } else {
            bail!("Rust is not installed");
        }
    } else {
        bail!("Failed to execute rustc command");
    }

    // Check if cargo is installed
    if let Ok(output) = Command::new("cargo").arg("--version").output() {
        if output.status.success() {
            print!(
                "Cargo is installed: {}",
                String::from_utf8_lossy(&output.stdout)
            );
        } else {
            bail!("Cargo is not installed");
        }
    } else {
        bail!("Failed to execute cargo command");
    }
    Ok(())
}

fn check_path_and_warn() -> Result<(), Error> {
    let local_bin = get_default_bin_dir();

    // Check if the bin directory exists in PATH
    if let Ok(path) = env::var("PATH") {
        #[cfg(windows)]
        let path_separator = ';';
        #[cfg(not(windows))]
        let path_separator = ':';

        if !path
            .split(path_separator)
            .any(|p| PathBuf::from(p) == local_bin)
        {
            println!("\nWARNING: {} is not in your PATH", local_bin.display());

            #[cfg(windows)]
            {
                println!("\nTo add it to your PATH:");
                println!("1. Press Win + X and select 'System'");
                println!("2. Click on 'Advanced system settings'");
                println!("3. Click on 'Environment Variables'");
                println!("4. Under 'User variables', find and select 'Path'");
                println!("5. Click 'Edit'");
                println!("6. Click 'New'");
                println!("7. Add the following path:");
                println!("    %USERPROFILE%\\.local\\bin");
                println!("8. Click 'OK' on all windows");
                println!("9. Restart your terminal\n");
            }

            #[cfg(not(windows))]
            {
                println!("Add one of the following lines depending on your shell:");
                println!("\nFor bash/zsh (~/.bashrc or ~/.zshrc):");
                println!("    export PATH=\"$HOME/.local/bin:$PATH\"");
                println!("\nFor fish (~/.config/fish/config.fish):");
                println!("    fish_add_path $HOME/.local/bin");
                println!("\nThen restart your shell or run one of:");
                println!("    source ~/.bashrc        # for bash");
                println!("    source ~/.zshrc         # for zsh");
                println!("    source ~/.config/fish/config.fish  # for fish\n");
            }
        }
    }
    Ok(())
}

// pub(crate) fn print_completion_instructions(shell: &Shell) {
//     match shell {
//         Shell::Bash => {
//             println!("\nTo install bash completions:");
//             println!("1. Create completion directory if it doesn't exist:");
//             println!("    mkdir -p ~/.local/share/bash-completion/completions");
//             println!("2. Add completions to the directory:");
//             println!(
//                 "    suiup completion bash > ~/.local/share/bash-completion/completions/suiup"
//             );
//             println!("\nMake sure you have bash-completion installed and loaded in your ~/.bashrc");
//         }
//         Shell::Fish => {
//             println!("\nTo install fish completions:");
//             println!("1. Create completion directory if it doesn't exist:");
//             println!("    mkdir -p ~/.config/fish/completions");
//             println!("2. Add completions to the directory:");
//             println!("    suiup completion fish > ~/.config/fish/completions/suiup.fish");
//         }
//         Shell::Zsh => {
//             println!("\nTo install zsh completions:");
//             println!("1. Create completion directory if it doesn't exist:");
//             println!("    mkdir -p ~/.zsh/completions");
//             println!("2. Add completions to the directory:");
//             println!("    suiup completion zsh > ~/.zsh/completions/_suiup");
//             println!("3. Add the following to your ~/.zshrc:");
//             println!("    fpath=(~/.zsh/completions $fpath)");
//             println!("    autoload -U compinit; compinit");
//         }
//         _ => {}
//     }
// }
