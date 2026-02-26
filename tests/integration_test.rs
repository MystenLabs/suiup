// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

mod test_utils;

#[cfg(test)]
mod tests {
    use crate::test_utils::TestEnv;
    use anyhow::Result;
    use assert_cmd::Command;
    use assert_cmd::cargo::cargo_bin_cmd;
    use predicates::prelude::*;
    use std::fs;
    use std::time::{Duration, SystemTime};
    use suiup::paths::installed_binaries_file;

    #[cfg(not(windows))]
    const DATA_HOME: &str = "XDG_DATA_HOME";
    #[cfg(not(windows))]
    const CONFIG_HOME: &str = "XDG_CONFIG_HOME";
    #[cfg(not(windows))]
    const CACHE_HOME: &str = "XDG_CACHE_HOME";
    #[cfg(not(windows))]
    const HOME: &str = "HOME";

    #[cfg(windows)]
    const DATA_HOME: &str = "LOCALAPPDATA";
    #[cfg(windows)]
    const CONFIG_HOME: &str = "LOCALAPPDATA";
    #[cfg(windows)]
    const CACHE_HOME: &str = "TEMP";
    #[cfg(windows)]
    const HOME: &str = "HOME";

    fn suiup_command(args: Vec<&str>, test_env: &TestEnv) -> Command {
        let mut cmd = cargo_bin_cmd!("suiup");
        cmd.args(args);

        cmd.env(DATA_HOME, &test_env.data_dir)
            .env(CONFIG_HOME, &test_env.config_dir)
            .env(CACHE_HOME, &test_env.cache_dir)
            .env(HOME, test_env.temp_dir.path())
            .env_remove("GITHUB_TOKEN");

        cmd
    }

    fn github_is_reachable() -> bool {
        use std::net::{TcpStream, ToSocketAddrs};

        let Ok(addrs) = ("github.com", 443).to_socket_addrs() else {
            return false;
        };

        for addr in addrs {
            if TcpStream::connect_timeout(&addr, Duration::from_secs(2)).is_ok() {
                return true;
            }
        }

        false
    }

    #[tokio::test]
    async fn test_install_flags() -> Result<()> {
        let test_env = TestEnv::new()?;
        test_env.initialize_paths()?;

        // NOT OK: nightly + version specified
        let mut cmd = suiup_command(
            vec!["install", "sui@testnet-v1.40.1", "--nightly"],
            &test_env,
        );
        cmd.assert().failure().stderr(predicate::str::contains(
            "Error: Cannot install from nightly and a release at the same time",
        ));

        // NOT OK: !sui + debug
        let mut cmd = suiup_command(vec!["install", "mvr", "--debug"], &test_env);
        cmd.assert().failure().stderr(predicate::str::contains(
            "Error: Debug flag is only available for the `sui` binary",
        ));

        // OK: nightly + debug
        // OK: nightly (if nightly + debug work, nightly works on its own too)
        if github_is_reachable() {
            let mut cmd = suiup_command(vec!["install", "mvr", "--nightly", "--debug"], &test_env);
            cmd.assert().success();
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_sui_install_and_use_binary() -> Result<()> {
        let test_env = TestEnv::new()?;
        test_env.initialize_paths()?;
        test_env.copy_testnet_releases_to_cache()?;

        // Run install command
        let mut cmd = suiup_command(vec!["install", "sui@testnet-1.39.3", "-y"], &test_env);

        #[cfg(windows)]
        let assert_string = "'sui.exe' extracted successfully!";
        #[cfg(not(windows))]
        let assert_string = "'sui' extracted successfully!";

        cmd.assert()
            .success()
            .stdout(predicate::str::contains(assert_string));

        // Verify binary exists in correct location
        #[cfg(windows)]
        let binary_path = test_env
            .data_dir
            .join("suiup/binaries/testnet/sui-v1.39.3.exe");
        #[cfg(not(windows))]
        let binary_path = test_env.data_dir.join("suiup/binaries/testnet/sui-v1.39.3");
        assert!(binary_path.exists());

        // Verify default binary exists
        #[cfg(windows)]
        let default_sui_binary = test_env.bin_dir.join("sui.exe");
        #[cfg(not(windows))]
        let default_sui_binary = test_env.bin_dir.join("sui");
        assert!(default_sui_binary.exists());

        // Test binary execution
        let mut cmd = Command::new(default_sui_binary);
        cmd.arg("--version");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("1.39.3"));

        Ok(())
    }

    #[tokio::test]
    async fn test_install_nightly() -> Result<()> {
        Ok(())
    }

    #[tokio::test]
    async fn test_install_debug() -> Result<()> {
        let test_env = TestEnv::new()?;
        test_env.initialize_paths()?;
        test_env.copy_testnet_releases_to_cache()?;

        // Run install command
        let mut cmd = suiup_command(vec!["install", "mvr", "--debug", "-y"], &test_env);
        cmd.assert().failure().stderr(predicate::str::contains(
            "Error: Debug flag is only available for the `sui` binary",
        ));

        // Run install command
        let mut cmd = suiup_command(
            vec!["install", "sui@testnet-1.39.3", "--debug", "-y"],
            &test_env,
        );

        #[cfg(windows)]
        let assert_string = "'sui-debug.exe' extracted successfully!";
        #[cfg(not(windows))]
        let assert_string = "'sui-debug' extracted successfully!";

        cmd.assert()
            .success()
            .stdout(predicate::str::contains(assert_string));

        // Verify binary exists in correct location
        // TODO! For windows, the test environment variables are not respected
        #[cfg(windows)]
        let binary_path = test_env
            .data_dir
            .join("suiup/binaries/testnet/sui-debug-v1.39.3.exe");
        #[cfg(not(windows))]
        let binary_path = test_env
            .data_dir
            .join("suiup/binaries/testnet/sui-debug-v1.39.3");
        assert!(binary_path.exists());

        // Verify default binary exists
        // TODO! For windows, the test environment variables are not respected
        #[cfg(windows)]
        let default_sui_binary = test_env.bin_dir.join("sui.exe");
        #[cfg(not(windows))]
        let default_sui_binary = test_env.bin_dir.join("sui");
        assert!(default_sui_binary.exists());

        // Test binary execution
        // on windows this fails due to being a debug binary
        // thread \'main\' has overflowed its stack
        #[cfg(not(windows))]
        {
            let mut cmd = Command::new(default_sui_binary);
            cmd.arg("--version");
            cmd.assert()
                .success()
                .stdout(predicate::str::contains("1.39.3"));
        }

        let mut cmd = suiup_command(vec!["default", "get"], &test_env);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Yes"));

        Ok(())
    }

    #[tokio::test]
    async fn test_update_workflow() -> Result<()> {
        let test_env = TestEnv::new()?;
        test_env.initialize_paths()?;

        // Install older version
        let mut cmd = suiup_command(vec!["install", "mvr@0.0.4", "-y"], &test_env);
        cmd.assert().success();

        // Run update
        let mut cmd = suiup_command(vec!["update", "mvr", "-y"], &test_env);
        cmd.assert().success();

        // Verify new version exists
        let binary_path = test_env.data_dir.join("suiup/binaries/standalone");
        let folders = std::fs::read_dir(&binary_path)?;
        let num_files: Vec<_> = folders.into_iter().collect();
        // should have at least 2 versions, 1.39.0 and whatever latest is
        assert!(!num_files.is_empty());

        // Verify default binary exists
        #[cfg(windows)]
        let default_mvr_binary = test_env.bin_dir.join("mvr.exe");
        #[cfg(not(windows))]
        let default_mvr_binary = test_env.bin_dir.join("mvr");
        assert!(default_mvr_binary.exists());

        // Test binary execution
        let mut cmd = Command::new(default_mvr_binary);
        cmd.arg("--version");
        cmd.assert().success();

        Ok(())
    }

    #[tokio::test]
    async fn test_default_workflow() -> Result<(), anyhow::Error> {
        let test_env = TestEnv::new()?;
        test_env.initialize_paths()?;
        test_env.copy_testnet_releases_to_cache()?;

        // Install 1.39.3
        let mut cmd = suiup_command(vec!["install", "sui@testnet-1.39.3", "-y"], &test_env);
        #[cfg(windows)]
        let assert_string = "'sui.exe' extracted successfully!";
        #[cfg(not(windows))]
        let assert_string = "'sui' extracted successfully!";
        cmd.assert()
            .success()
            .stdout(predicate::str::contains(assert_string));
        // Test binary execution
        #[cfg(windows)]
        let default_sui_binary = test_env.bin_dir.join("sui.exe");
        #[cfg(not(windows))]
        let default_sui_binary = test_env.bin_dir.join("sui");
        let mut cmd = Command::new(&default_sui_binary);
        cmd.arg("--version");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("1.39.3"));

        // Install 1.40.1
        let mut cmd = suiup_command(vec!["install", "sui@testnet-1.40.1", "-y"], &test_env);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains(assert_string));
        // Test binary execution
        let mut cmd = Command::new(&default_sui_binary);
        cmd.arg("--version");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("1.40.1"));

        // Switch from 1.39.3 to 1.40.1
        let mut cmd = suiup_command(vec!["default", "set", "sui@testnet-1.39.3"], &test_env);
        cmd.assert().success();

        let mut cmd = Command::new(default_sui_binary);
        cmd.arg("--version");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("1.39.3"));

        Ok(())
    }

    #[tokio::test]
    async fn test_default_mvr_workflow() -> Result<(), anyhow::Error> {
        let test_env = TestEnv::new()?;
        test_env.initialize_paths()?;

        // Install last version and nightly
        let mut cmd = suiup_command(vec!["install", "mvr", "-y"], &test_env);
        cmd.assert().success();
        assert!(installed_binaries_file().unwrap().exists());

        let default_mvr_binary = test_env.bin_dir.join("mvr");
        let version_cmd = Command::new(&default_mvr_binary)
            .arg("--version")
            .output()
            .expect("Failed to run command");
        let mvr_version = if version_cmd.status.success() {
            String::from_utf8_lossy(&version_cmd.stdout).replace("mvr ", "")
        } else {
            panic!("Could not run command")
        };

        let version: Vec<_> = mvr_version.split('-').collect();
        let version = version[0];

        // Install from main branch
        if !github_is_reachable() {
            return Ok(());
        }
        let mut cmd = suiup_command(vec!["install", "mvr", "--nightly", "-y"], &test_env);
        cmd.assert().success();

        // Switch version to the one we installed from release
        let mut cmd = suiup_command(vec!["default", "set", &format!("mvr@{version}")], &test_env);
        cmd.assert().success();

        let mut version_cmd = Command::new(&default_mvr_binary);
        version_cmd.arg("--version");
        version_cmd
            .assert()
            .success()
            .stdout(predicate::str::contains(version));

        // Now switch from a release version to nightly
        let mut cmd = suiup_command(vec!["default", "set", "mvr", "--nightly"], &test_env);
        cmd.assert().success();

        Ok(())
    }

    #[tokio::test]
    async fn test_walrus_install_and_use_binary() -> Result<()> {
        let test_env = TestEnv::new()?;
        test_env.initialize_paths()?;
        test_env.copy_testnet_releases_to_cache()?;

        // Run install command
        let mut cmd = suiup_command(vec!["install", "walrus@testnet-v1.18.2", "-y"], &test_env);

        #[cfg(windows)]
        let assert_string = "'walrus.exe' extracted successfully!";
        #[cfg(not(windows))]
        let assert_string = "'walrus' extracted successfully!";

        cmd.assert()
            .success()
            .stdout(predicate::str::contains(assert_string));

        // Verify binary exists in correct location
        #[cfg(windows)]
        let binary_path = test_env
            .data_dir
            .join("suiup/binaries/testnet/walrus-v1.18.2.exe");
        #[cfg(not(windows))]
        let binary_path = test_env
            .data_dir
            .join("suiup/binaries/testnet/walrus-v1.18.2");
        assert!(binary_path.exists());

        // Verify default binary exists
        #[cfg(windows)]
        let default_sui_binary = test_env.bin_dir.join("walrus.exe");
        #[cfg(not(windows))]
        let default_sui_binary = test_env.bin_dir.join("walrus");
        assert!(default_sui_binary.exists());

        // Test binary execution
        let mut cmd = Command::new(default_sui_binary);
        cmd.arg("--version");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("1.18.2"));

        Ok(())
    }

    #[tokio::test]
    async fn test_show_default_flag() -> Result<()> {
        let test_env = TestEnv::new()?;
        test_env.initialize_paths()?;

        // Test show without --default flag (should show both default and installed)
        let mut cmd = suiup_command(vec!["show"], &test_env);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Default binaries:"))
            .stdout(predicate::str::contains("Installed binaries:"));

        // Test show with --default flag (should only show default binaries)
        let mut cmd = suiup_command(vec!["show", "--default"], &test_env);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Default binaries:"))
            .stdout(predicate::str::contains("Installed binaries:").not());

        Ok(())
    }

    #[tokio::test]
    async fn test_switch_command_basic() -> Result<()> {
        let test_env = TestEnv::new()?;
        test_env.initialize_paths()?;

        // Test switch with non-existent binary (should fail gracefully)
        let mut cmd = suiup_command(vec!["switch", "sui@testnet"], &test_env);
        cmd.assert().failure().stderr(predicate::str::contains(
            "No binaries installed for testnet",
        ));

        Ok(())
    }

    #[tokio::test]
    async fn test_switch_command_error_cases() -> Result<()> {
        let test_env = TestEnv::new()?;
        test_env.initialize_paths()?;

        // Test invalid format (empty parts)
        let mut cmd = suiup_command(vec!["switch", "sui@"], &test_env);
        cmd.assert().failure().stderr(predicate::str::contains(
            "Version cannot be empty. Use 'binary' or 'binary@version' (e.g., sui@v1.60.0)",
        ));

        // Test non-existent binary
        let mut cmd = suiup_command(vec!["switch", "sui@nonexistent"], &test_env);
        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("Invalid version format: 'nonexistent'. Expected a version like 'v1.60.0' or '1.60.0', or when applicable, 'testnet', 'devnet', 'mainnet'."));

        Ok(())
    }

    #[tokio::test]
    async fn test_switch_command_help() -> Result<()> {
        let test_env = TestEnv::new()?;
        test_env.initialize_paths()?;

        // Test switch command help
        let mut cmd = suiup_command(vec!["switch", "--help"], &test_env);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains(
                "Switch to a different version of an installed binary",
            ))
            .stdout(predicate::str::contains("BINARY_SPEC"))
            .stdout(predicate::str::contains("sui@testnet"));

        Ok(())
    }

    #[tokio::test]
    async fn test_switch_workflow() -> Result<()> {
        let test_env = TestEnv::new()?;
        test_env.initialize_paths()?;
        test_env.copy_testnet_releases_to_cache()?;

        // Install first version (1.39.3)
        let mut cmd = suiup_command(vec!["install", "sui@testnet-1.39.3", "-y"], &test_env);
        cmd.assert().success();

        // Verify first version is set as default
        let mut cmd = suiup_command(vec!["default", "get"], &test_env);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("sui"))
            .stdout(predicate::str::contains("testnet"))
            .stdout(predicate::str::contains("v1.39.3"));

        // Install second version (1.40.1)
        let mut cmd = suiup_command(vec!["install", "sui@testnet-1.40.1", "-y"], &test_env);
        cmd.assert().success();

        // Explicitly set the default to the second installed version before switch validation.
        let mut cmd = suiup_command(vec!["default", "set", "sui@testnet-1.40.1"], &test_env);
        cmd.assert().success();

        // Verify second version is default
        let mut cmd = suiup_command(vec!["default", "get"], &test_env);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("sui"))
            .stdout(predicate::str::contains("testnet"))
            .stdout(predicate::str::contains("v1.40.1"));

        // Use switch command to go back to testnet (should pick latest, which is 1.40.1)
        let mut cmd = suiup_command(vec!["switch", "sui@testnet"], &test_env);
        cmd.assert().success().stdout(predicate::str::contains(
            "Default binary updated to sui@testnet-v1.40.1 version",
        ));

        // Verify switch command maintained the default (since it picked the latest)
        let mut cmd = suiup_command(vec!["default", "get"], &test_env);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("sui"))
            .stdout(predicate::str::contains("testnet"))
            .stdout(predicate::str::contains("v1.40.1"));

        // Verify default get shows correct info
        let mut cmd = suiup_command(vec!["default", "get"], &test_env);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("sui"))
            .stdout(predicate::str::contains("testnet"));

        // Test show command with and without --default flag
        let mut cmd = suiup_command(vec!["show"], &test_env);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Default binaries:"))
            .stdout(predicate::str::contains("Installed binaries:"));

        let mut cmd = suiup_command(vec!["show", "--default"], &test_env);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Default binaries:"))
            .stdout(predicate::str::contains("Installed binaries:").not());

        Ok(())
    }

    #[tokio::test]
    async fn test_cleanup_command_help() -> Result<()> {
        let test_env = TestEnv::new()?;

        let text = if cfg!(windows) {
            "Usage: suiup.exe cleanup"
        } else {
            "Usage: suiup cleanup"
        };
        let mut cmd = suiup_command(vec!["cleanup", "--help"], &test_env);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains(text))
            .stdout(predicate::str::contains("--all"))
            .stdout(predicate::str::contains("--days"))
            .stdout(predicate::str::contains("--dry-run"))
            .stdout(predicate::str::contains("Remove all cache files"))
            .stdout(predicate::str::contains("Days to keep files in cache"));

        Ok(())
    }

    #[tokio::test]
    async fn test_no_command_shows_help() -> Result<()> {
        let test_env = TestEnv::new()?;
        test_env.initialize_paths()?;

        let mut cmd = suiup_command(vec![], &test_env);
        let usage = if cfg!(windows) {
            "Usage: suiup.exe"
        } else {
            "Usage: suiup"
        };
        cmd.assert()
            .success()
            .stdout(predicate::str::contains(usage));

        Ok(())
    }

    #[tokio::test]
    async fn test_cleanup_dry_run_workflow() -> Result<()> {
        let test_env = TestEnv::new()?;
        test_env.initialize_paths()?;
        test_env.copy_testnet_releases_to_cache()?;

        let cache_dir = test_env.cache_dir.join("suiup").join("releases");

        fs::create_dir_all(&cache_dir)?;

        let old_file = cache_dir.join("old_release.zip");
        fs::write(&old_file, b"old release content")?;

        // Make the file appear old
        let old_time = SystemTime::now() - Duration::from_secs(60 * 60 * 24 * 40); // 40 days ago
        filetime::set_file_mtime(&old_file, filetime::FileTime::from_system_time(old_time))?;

        // Run dry run cleanup
        let mut cmd = suiup_command(vec!["cleanup", "--dry-run"], &test_env);
        cmd.assert().success().stdout(predicate::str::contains(
            "Removing release archives older than 30 days",
        ));

        // Verify file still exists after dry run
        assert!(old_file.exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_cleanup_with_days_filter() -> Result<()> {
        let test_env = TestEnv::new()?;
        test_env.initialize_paths()?;
        test_env.copy_testnet_releases_to_cache()?;

        let cache_dir = test_env.cache_dir.join("suiup").join("releases");

        fs::create_dir_all(&cache_dir)?;

        // Create files with different ages
        let old_file = cache_dir.join("old_file.zip");
        let recent_file = cache_dir.join("recent_file.zip");

        fs::write(&old_file, b"old content")?;
        fs::write(&recent_file, b"recent content")?;

        // Make old file 40 days old
        let old_time = SystemTime::now() - Duration::from_secs(60 * 60 * 24 * 40);
        filetime::set_file_mtime(&old_file, filetime::FileTime::from_system_time(old_time))?;

        // Run cleanup with 30 days filter
        let mut cmd = suiup_command(vec!["cleanup", "--days", "30"], &test_env);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains(
                "Removing release archives older than 30 days",
            ))
            .stdout(predicate::str::contains("Cleanup complete"));

        // Old file should be removed, recent file should remain
        assert!(!old_file.exists());
        assert!(recent_file.exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_cleanup_all_files() -> Result<()> {
        let test_env = TestEnv::new()?;
        test_env.initialize_paths()?;
        test_env.copy_testnet_releases_to_cache()?;

        let cache_dir = test_env.cache_dir.join("suiup").join("releases");

        fs::create_dir_all(&cache_dir)?;

        // Create additional test files
        let file1 = cache_dir.join("test1.zip");
        let file2 = cache_dir.join("test2.zip");

        fs::write(&file1, b"test content 1")?;
        fs::write(&file2, b"test content 2")?;

        // Verify files exist before cleanup
        assert!(file1.exists());
        assert!(file2.exists());

        // Run cleanup with --all flag
        let mut cmd = suiup_command(vec!["cleanup", "--all"], &test_env);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Removing all release archives"))
            .stdout(predicate::str::contains("Cache cleared successfully"));

        // All files should be removed
        assert!(!file1.exists());
        assert!(!file2.exists());

        // But directory should still exist
        assert!(cache_dir.exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_cleanup_after_install_workflow() -> Result<()> {
        let test_env = TestEnv::new()?;
        test_env.initialize_paths()?;
        test_env.copy_testnet_releases_to_cache()?;

        // Install a component first to create cache files
        let mut cmd = suiup_command(vec!["install", "sui@testnet-1.39.3", "-y"], &test_env);
        cmd.assert().success();

        let cache_dir = test_env.cache_dir.join("suiup").join("releases");

        fs::create_dir_all(&cache_dir)?;

        let old_file = cache_dir.join("old_archive.zip");
        fs::write(&old_file, b"old archive")?;
        let old_time = SystemTime::now() - Duration::from_secs(60 * 60 * 24 * 40);
        filetime::set_file_mtime(&old_file, filetime::FileTime::from_system_time(old_time))?;

        // Run cleanup
        let mut cmd = suiup_command(vec!["cleanup", "--days", "30"], &test_env);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Cleanup complete"));

        // Verify old file is removed
        assert!(!old_file.exists());

        // Verify installed binary still works
        #[cfg(windows)]
        let default_sui_binary = test_env.bin_dir.join("sui.exe");
        #[cfg(not(windows))]
        let default_sui_binary = test_env.bin_dir.join("sui");

        let mut cmd = Command::new(default_sui_binary);
        cmd.arg("--version");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("1.39.3"));

        Ok(())
    }
}
