// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::fs;
    use std::sync::Mutex;
    use std::time::{Duration, SystemTime};
    use suiup::commands::{parse_component_with_version, BinaryName, CommandMetadata};
    use suiup::handlers::cleanup::handle_cleanup;
    use suiup::handlers::switch::parse_binary_spec;
    use suiup::handlers::config::{ConfigHandler, ConfigValue, SuiupConfig};
    use tempfile::TempDir;

    // Mutex to serialize cleanup tests that modify environment variables
    static CLEANUP_TEST_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_parse_component_with_version() -> Result<(), anyhow::Error> {
        let result = parse_component_with_version("sui")?;
        let expected = CommandMetadata {
            name: BinaryName::Sui,
            network: "testnet".to_string(),
            version: None,
        };
        assert_eq!(expected, result);

        let result = parse_component_with_version("sui@testnet-v1.39.3")?;
        let expected = CommandMetadata {
            name: BinaryName::Sui,
            network: "testnet".to_string(),
            version: Some("v1.39.3".to_string()),
        };
        assert_eq!(expected, result,);

        let result = parse_component_with_version("walrus")?;
        let expected = CommandMetadata {
            name: BinaryName::Walrus,
            network: "testnet".to_string(),
            version: None,
        };
        assert_eq!(expected, result);

        let result = parse_component_with_version("mvr")?;
        let expected = CommandMetadata {
            name: BinaryName::Mvr,
            network: "testnet".to_string(),
            version: None,
        };
        assert_eq!(expected, result);

        let result = parse_component_with_version("random");
        assert_eq!(
            result.unwrap_err().to_string(),
            "Invalid binary name: random. Use `suiup list` to find available binaries to install."
                .to_string()
        );

        Ok(())
    }

    #[test]
    fn test_sui_component_display() {
        assert_eq!(BinaryName::Sui.to_string(), "sui");
        assert_eq!(BinaryName::Mvr.to_string(), "mvr");
        assert_eq!(BinaryName::Walrus.to_string(), "walrus");
    }

    #[test]
    fn test_parse_binary_spec() -> Result<()> {
        // Test valid format
        let result = parse_binary_spec("sui@testnet")?;
        assert_eq!(result, ("sui".to_string(), "testnet".to_string()));

        let result = parse_binary_spec("mvr@main")?;
        assert_eq!(result, ("mvr".to_string(), "main".to_string()));

        let result = parse_binary_spec("walrus@devnet")?;
        assert_eq!(result, ("walrus".to_string(), "devnet".to_string()));

        // Test invalid formats
        let result = parse_binary_spec("sui");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid format"));

        let result = parse_binary_spec("sui@");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Binary name and network/release cannot be empty"));

        let result = parse_binary_spec("@testnet");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Binary name and network/release cannot be empty"));

        let result = parse_binary_spec("sui@testnet@extra");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid format"));

        Ok(())
    }

    #[tokio::test]
    async fn test_cleanup_empty_directory() -> Result<()> {
        let _guard = CLEANUP_TEST_MUTEX.lock().unwrap();
        let temp_dir = TempDir::new()?;
        #[cfg(windows)]
        std::env::set_var("TEMP", temp_dir.path());
        #[cfg(not(windows))]
        std::env::set_var("XDG_CACHE_HOME", temp_dir.path());

        // Test cleanup on empty directory
        let result = handle_cleanup(false, 30, true).await;
        assert!(result.is_ok());

        Ok(())
    }

    #[tokio::test]
    async fn test_cleanup_dry_run() -> Result<()> {
        let _guard = CLEANUP_TEST_MUTEX.lock().unwrap();
        let temp_dir = TempDir::new()?;
        let cache_dir = temp_dir.path().join("suiup").join("release_archives");
        fs::create_dir_all(&cache_dir)?;

        // Create test files with different ages
        let old_file = cache_dir.join("old_file.zip");
        let new_file = cache_dir.join("new_file.zip");

        fs::write(&old_file, b"old content")?;
        fs::write(&new_file, b"new content")?;

        // Make old file appear old by setting modified time
        let old_time = SystemTime::now() - Duration::from_secs(60 * 60 * 24 * 40); // 40 days ago
        filetime::set_file_mtime(&old_file, filetime::FileTime::from_system_time(old_time))?;

        #[cfg(windows)]
        std::env::set_var("TEMP", temp_dir.path());
        #[cfg(not(windows))]
        std::env::set_var("XDG_CACHE_HOME", temp_dir.path());

        // Dry run should not remove files
        let result = handle_cleanup(false, 30, true).await;
        assert!(result.is_ok());
        assert!(old_file.exists());
        assert!(new_file.exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_cleanup_remove_old_files() -> Result<()> {
        let _guard = CLEANUP_TEST_MUTEX.lock().unwrap();
        let temp_dir = TempDir::new()?;
        // Set up environment variable for cache directory
        #[cfg(windows)]
        std::env::set_var("TEMP", temp_dir.path());
        #[cfg(not(windows))]
        std::env::set_var("XDG_CACHE_HOME", temp_dir.path());

        // Create cache directory structure manually to ensure it exists
        let cache_dir = temp_dir.path().join("suiup").join("releases");
        fs::create_dir_all(&cache_dir)?;

        // Create test files
        let old_file = cache_dir.join("old_file.zip");
        let new_file = cache_dir.join("new_file.zip");

        fs::write(&old_file, b"old content")?;
        fs::write(&new_file, b"new content")?;

        // Make old file appear old
        let old_time = SystemTime::now() - Duration::from_secs(60 * 60 * 24 * 40); // 40 days ago
        filetime::set_file_mtime(&old_file, filetime::FileTime::from_system_time(old_time))?;

        // Actual cleanup should remove old file but keep new file
        let result = handle_cleanup(false, 30, false).await;
        assert!(result.is_ok());
        assert!(!old_file.exists());
        assert!(new_file.exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_cleanup_remove_all() -> Result<()> {
        let _guard = CLEANUP_TEST_MUTEX.lock().unwrap();
        let temp_dir = TempDir::new()?;
        // Set up environment variable for cache directory
        #[cfg(windows)]
        std::env::set_var("TEMP", temp_dir.path());
        #[cfg(not(windows))]
        std::env::set_var("XDG_CACHE_HOME", temp_dir.path());

        // Create cache directory structure manually to ensure it exists
        let cache_dir = temp_dir.path().join("suiup").join("releases");
        fs::create_dir_all(&cache_dir)?;

        // Create test files
        let file1 = cache_dir.join("file1.zip");
        let file2 = cache_dir.join("file2.zip");

        fs::write(&file1, b"content1")?;
        fs::write(&file2, b"content2")?;

        // Remove all should clear everything
        let result = handle_cleanup(true, 30, false).await;
        assert!(result.is_ok());
        assert!(!file1.exists());
        assert!(!file2.exists());
        assert!(cache_dir.exists()); // Directory should still exist

        Ok(())
    }

    // Config-related unit tests
    #[test]
    fn test_config_value_from_string_valid() -> Result<()> {
        // Test string values
        let result = ConfigValue::from_string("mirror_url", "https://example.com")?;
        assert!(matches!(result, ConfigValue::String(_)));

        let result = ConfigValue::from_string("default_network", "mainnet")?;
        assert!(matches!(result, ConfigValue::String(_)));

        let result = ConfigValue::from_string("install_path", "/custom/path")?;
        assert!(matches!(result, ConfigValue::String(_)));

        let result = ConfigValue::from_string("github_token", "ghp_1234567890abcdef")?;
        assert!(matches!(result, ConfigValue::String(_)));

        // Test numeric values
        let result = ConfigValue::from_string("cache_days", "7")?;
        assert!(matches!(result, ConfigValue::Number(7)));

        let result = ConfigValue::from_string("max_cache_size", "2147483648")?;
        assert!(matches!(result, ConfigValue::Number(2147483648)));

        // Test boolean values
        let result = ConfigValue::from_string("auto_cleanup", "true")?;
        assert!(matches!(result, ConfigValue::Boolean(true)));

        let result = ConfigValue::from_string("enable_update_warnings", "false")?;
        assert!(matches!(result, ConfigValue::Boolean(false)));

        let result = ConfigValue::from_string("disable_update_warnings", "true")?;
        assert!(matches!(result, ConfigValue::Boolean(true)));

        Ok(())
    }

    #[test]
    fn test_config_value_from_string_invalid() {
        // Test invalid key
        let result = ConfigValue::from_string("invalid_key", "value");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown configuration key"));

        // Test invalid boolean
        let result = ConfigValue::from_string("auto_cleanup", "maybe");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid boolean value"));

        // Test invalid number
        let result = ConfigValue::from_string("cache_days", "not_a_number");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid number value"));
    }

    #[test]
    fn test_suiup_config_default() {
        let config = SuiupConfig::default();

        assert_eq!(config.mirror_url, "https://github.com");
        assert_eq!(config.cache_days, 30);
        assert_eq!(config.auto_cleanup, false);
        assert_eq!(config.max_cache_size, 1024 * 1024 * 1024); // 1GB
        assert_eq!(config.default_network, "testnet");
        assert_eq!(config.install_path, None);
        assert_eq!(config.enable_update_warnings, true);
        assert_eq!(config.disable_update_warnings, false);
        assert_eq!(config.github_token, None);
    }

    #[test]
    fn test_config_serialization() -> Result<()> {
        let config = SuiupConfig {
            mirror_url: "https://custom.mirror.com".to_string(),
            cache_days: 14,
            auto_cleanup: true,
            max_cache_size: 2147483648, // 2GB
            default_network: "mainnet".to_string(),
            install_path: Some("/custom/path".to_string()),
            enable_update_warnings: false,
            disable_update_warnings: true,
            github_token: Some("ghp_test_token".to_string()),
        };

        // Test serialization
        let json = serde_json::to_string(&config)?;
        assert!(json.contains("custom.mirror.com"));
        assert!(json.contains("mainnet"));
        assert!(json.contains("ghp_test_token"));

        // Test deserialization
        let deserialized: SuiupConfig = serde_json::from_str(&json)?;
        assert_eq!(deserialized.mirror_url, config.mirror_url);
        assert_eq!(deserialized.cache_days, config.cache_days);
        assert_eq!(deserialized.auto_cleanup, config.auto_cleanup);
        assert_eq!(deserialized.max_cache_size, config.max_cache_size);
        assert_eq!(deserialized.default_network, config.default_network);
        assert_eq!(deserialized.install_path, config.install_path);
        assert_eq!(deserialized.enable_update_warnings, config.enable_update_warnings);
        assert_eq!(deserialized.disable_update_warnings, config.disable_update_warnings);
        assert_eq!(deserialized.github_token, config.github_token);

        Ok(())
    }

    #[test]
    fn test_config_partial_deserialization() -> Result<()> {
        // Test that missing fields use default values
        let json = r#"{"mirror_url": "https://example.com", "cache_days": 7}"#;
        let config: SuiupConfig = serde_json::from_str(json)?;

        assert_eq!(config.mirror_url, "https://example.com");
        assert_eq!(config.cache_days, 7);
        // Other fields should use defaults
        assert_eq!(config.auto_cleanup, false); // default
        assert_eq!(config.max_cache_size, 1024 * 1024 * 1024); // default
        assert_eq!(config.default_network, "testnet"); // default
        assert_eq!(config.install_path, None); // default
        assert_eq!(config.enable_update_warnings, true); // default
        assert_eq!(config.disable_update_warnings, false); // default
        assert_eq!(config.github_token, None); // default

        Ok(())
    }

    #[test]
    fn test_config_value_types() {
        // Test ConfigValue enum variants
        let string_val = ConfigValue::String("test".to_string());
        let number_val = ConfigValue::Number(42);
        let bool_val = ConfigValue::Boolean(true);

        match string_val {
            ConfigValue::String(s) => assert_eq!(s, "test"),
            _ => panic!("Expected String variant"),
        }

        match number_val {
            ConfigValue::Number(n) => assert_eq!(n, 42),
            _ => panic!("Expected Number variant"),
        }

        match bool_val {
            ConfigValue::Boolean(b) => assert!(b),
            _ => panic!("Expected Boolean variant"),
        }
    }

    #[test]
    fn test_config_github_token_validation() -> Result<()> {
        // Test valid GitHub token formats
        let valid_tokens = vec![
            "ghp_1234567890abcdef1234567890abcdef12345678",
            "gho_1234567890abcdef1234567890abcdef12345678",
            "ghu_1234567890abcdef1234567890abcdef12345678",
            "ghs_1234567890abcdef1234567890abcdef12345678",
            "ghr_1234567890abcdef1234567890abcdef12345678",
        ];

        for token in valid_tokens {
            let result = ConfigValue::from_string("github_token", token);
            assert!(result.is_ok(), "Token {} should be valid", token);
        }

        // Test that ConfigValue creation succeeds even for short tokens
        // (validation happens at a higher level)
        let short_tokens = vec![
            "invalid_token",
            "short",
            "ghp_short",
        ];

        for token in short_tokens {
            let result = ConfigValue::from_string("github_token", token);
            assert!(result.is_ok(), "ConfigValue creation should succeed for {}", token);
        }

        Ok(())
    }

    #[test]
    fn test_config_network_values() -> Result<()> {
        let valid_networks = vec!["testnet", "devnet", "mainnet"];

        for network in valid_networks {
            let result = ConfigValue::from_string("default_network", network)?;
            match result {
                ConfigValue::String(n) => assert_eq!(n, network),
                _ => panic!("Expected String variant for network"),
            }
        }

        Ok(())
    }

    #[test]
    fn test_config_numeric_ranges() -> Result<()> {
        // Test valid numeric values
        let result = ConfigValue::from_string("cache_days", "1")?;
        assert!(matches!(result, ConfigValue::Number(1)));

        let result = ConfigValue::from_string("cache_days", "365")?;
        assert!(matches!(result, ConfigValue::Number(365)));

        let result = ConfigValue::from_string("max_cache_size", "1048576")?; // 1MB
        assert!(matches!(result, ConfigValue::Number(1048576)));

        let result = ConfigValue::from_string("max_cache_size", "10737418240")?; // 10GB
        assert!(matches!(result, ConfigValue::Number(10737418240)));

        // Test zero values
        let result = ConfigValue::from_string("cache_days", "0")?;
        assert!(matches!(result, ConfigValue::Number(0)));

        Ok(())
    }

}
