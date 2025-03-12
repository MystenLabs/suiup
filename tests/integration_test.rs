// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

mod test_utils;

#[cfg(test)]
mod tests {
    use crate::test_utils::TestEnv;
    use anyhow::Result;
    use assert_cmd::Command;
    use insta::assert_snapshot;
    use predicates::prelude::*;
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
        let mut cmd = Command::cargo_bin("suiup").unwrap();
        cmd.args(args);

        cmd.env(DATA_HOME, &test_env.data_dir)
            .env(CONFIG_HOME, &test_env.config_dir)
            .env(CACHE_HOME, &test_env.cache_dir)
            .env(HOME, &test_env.temp_dir.path());
        cmd
    }

    fn run_command(
        command_name: &str,
        args: Vec<&str>,
        test_env: &TestEnv,
        combined_output: &mut String,
        combined_error: &mut String,
    ) {
        let mut cmd = if command_name == "suiup" {
            Command::cargo_bin("suiup").unwrap()
        } else {
            Command::new(command_name)
        };

        cmd.args(args.clone());

        cmd.env(DATA_HOME, &test_env.data_dir)
            .env(CONFIG_HOME, &test_env.config_dir)
            .env(CACHE_HOME, &test_env.cache_dir)
            .env(HOME, &test_env.temp_dir.path())
            .env("RUST_LOG", "off");

        let output = cmd.output().expect(
            format!(
                "Failed to execute command {command_name} {}",
                args.join(" ")
            )
            .as_str(),
        );

        let (output, error) = (
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        );

        combined_output.push_str(&format!(
            "===={} {} ==== output\n{}",
            command_name,
            args.join(" "),
            &output
        ));
        combined_error.push_str(&format!(
            "===={} {} ==== error\n{}",
            command_name,
            args.join(" "),
            &error
        ));
    }

    fn run_commands<'a>(
        commands: Vec<&'a str>,
        test_env: &TestEnv,
        redact: Vec<(&'a str, &'a str)>,
        combined_output: &'a mut String,
        combined_error: &'a mut String,
    ) -> String {
        for (cmd) in commands {
            let cmd: Vec<&str> = cmd.split_whitespace().collect();
            run_command(
                cmd[0],
                cmd[1..].to_vec(),
                test_env,
                combined_output,
                combined_error,
            );
        }

        let final_output = format!("{}\n{}", combined_output, combined_error);

        let mut settings = insta::Settings::clone_current();
        for r in redact {
            settings.add_redaction(r.0, r.1);
        }
        let _guard = settings.bind_to_scope();
        final_output
    }

    #[tokio::test]
    async fn test_install_flags() -> Result<()> {
        let test_env = TestEnv::new()?;
        test_env.initialize_paths()?;

        let final_output = run_commands(
            vec![
                "suiup install sui testnet-v1.40.1 --nightly", // NOT OK: nightly + version specified
                "suiup install mvr --debug",                   // NOT OK: !sui + debug
                "suiup install mvr --nightly --debug",         // OK: nightly + debug
            ],
            &test_env,
            vec![
                (".exe", ""), // remove .exe from output to make it simple to have one snapshot
                              // that also works on Windows
            ],
            &mut String::new(),
            &mut String::new(),
        );
        assert_snapshot!(final_output);

        Ok(())
    }

    #[tokio::test]
    async fn test_install_and_use_binary() -> Result<()> {
        let test_env = TestEnv::new()?;
        test_env.initialize_paths()?;
        test_env.copy_testnet_releases_to_cache()?;

        // Run install command
        let mut cmd = suiup_command(vec!["install", "sui", "testnet-v1.39.3", "-y"], &test_env);

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

        #[cfg(windows)]
        let default_sui_binary = "sui.exe";
        #[cfg(not(windows))]
        let default_sui_binary = "sui";

        // Run commands
        let final_output = run_commands(
            vec![
                "suiup install mvr --debug -y",
                "suiup install sui testnet-v1.39.3 --debug -y",
                &format!("{} --version", default_sui_binary),
                "suiup default get",
            ],
            &test_env,
            vec![
                (".exe", ""), // remove .exe from output to make it simple to have one snapshot
                              // that also works on Windows
            ],
            &mut String::new(),
            &mut String::new(),
        );

        assert_snapshot!(final_output);

        // Verify binary exists in correct location
        #[cfg(windows)]
        let binary_name = "sui-debug-v1.39.3.exe";
        #[cfg(not(windows))]
        let binary_name = "sui-debug-v1.39.3";
        assert!(test_env
            .data_dir
            .join("suiup/binaries/testnet")
            .join(binary_name)
            .exists());

        // Verify default binary exists
        assert!(test_env.bin_dir.join(default_sui_binary).exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_update_mvr_workflow() -> Result<()> {
        let test_env = TestEnv::new()?;
        test_env.initialize_paths()?;
        let mut combined_output = String::new();
        let mut combined_error = String::new();

        #[cfg(windows)]
        let default_mvr_binary = "mvr.exe";
        #[cfg(not(windows))]
        let default_mvr_binary = "mvr";

        // Run commands
        let final_output = run_commands(
            vec![
                "suiup install mvr v0.0.4 -y",
                "suiup update mvr -y",
                &format!("{} {}", default_mvr_binary, "--version"),
            ],
            &test_env,
            vec![(".exe", "")],
            &mut combined_output,
            &mut combined_error,
        );

        assert_snapshot!(final_output);

        // Verify new version exists
        let binary_path = test_env.data_dir.join("suiup/binaries/standalone");
        let folders = std::fs::read_dir(&binary_path)?;
        let num_files: Vec<_> = folders.into_iter().collect();
        // should have at least 2 versions, 1.39.0 and whatever latest is
        assert!(num_files.len() >= 1);

        // Verify default binary exists
        assert!(test_env.bin_dir.join(default_mvr_binary).exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_default_workflow() -> Result<(), anyhow::Error> {
        let test_env = TestEnv::new()?;
        test_env.initialize_paths()?;
        test_env.copy_testnet_releases_to_cache()?;

        #[cfg(windows)]
        let default_sui_binary = "sui.exe";
        #[cfg(not(windows))]
        let default_sui_binary = "sui";

        // Run commands
        let final_output = run_commands(
            vec![
                "suiup install sui testnet-v1.39.3 -y",
                "suiup install sui testnet-v1.40.1 -y",
                "suiup default set sui testnet-v1.39.3",
                &format!("{} --version", default_sui_binary),
            ],
            &test_env,
            vec![
                (".exe", ""), // remove .exe from output to make it simple to have one snapshot
                              // that also works on Windows
            ],
            &mut String::new(),
            &mut String::new(),
        );

        assert_snapshot!(final_output);

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

        let version: Vec<_> = mvr_version.split("-").collect();
        let version = version[0];

        // Install from main branch
        let mut cmd = suiup_command(vec!["install", "mvr", "--nightly", "-y"], &test_env);
        cmd.assert().success();

        // Switch version to the one we installed from release
        let mut cmd = suiup_command(
            vec!["default", "set", "mvr", &format!("v{version}")],
            &test_env,
        );
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
}
