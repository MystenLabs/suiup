// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::fs;
    use std::time::{Duration, SystemTime};
    use suiup::commands::{BinaryName, CommandMetadata, parse_component_with_version};
    use suiup::handlers::cleanup::handle_cleanup;
    use suiup::paths;
    use suiup::set_env_var;
    use tempfile::TempDir;

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
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid binary name: random"));

        Ok(())
    }

    #[test]
    fn test_sui_component_display() {
        assert_eq!(BinaryName::Sui.to_string(), "sui");
        assert_eq!(BinaryName::Mvr.to_string(), "mvr");
        assert_eq!(BinaryName::Walrus.to_string(), "walrus");
    }

    #[tokio::test]
    async fn test_cleanup_empty_directory() -> Result<()> {
        let temp_dir = TempDir::new()?;
        set_env_var!("XDG_CACHE_HOME", temp_dir.path());

        // Test cleanup on empty directory
        let result = handle_cleanup(false, 30, true).await;
        assert!(result.is_ok());

        Ok(())
    }

    #[tokio::test]
    async fn test_cleanup_dry_run() -> Result<()> {
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

        unsafe {
            std::env::set_var("XDG_CACHE_HOME", temp_dir.path());
        }

        // Dry run should not remove files
        let result = handle_cleanup(false, 30, true).await;
        assert!(result.is_ok());
        assert!(old_file.exists());
        assert!(new_file.exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_cleanup_remove_old_files() -> Result<()> {
        let temp_dir = TempDir::new()?;
        // Set up environment variable for cache directory
        unsafe {
            std::env::set_var("XDG_CACHE_HOME", temp_dir.path());
        }
        // Create cache directory
        let cache_dir = paths::release_archive_dir();
        fs::create_dir_all(&cache_dir)?;

        // Create test files
        let old_file = cache_dir.join("old_file.zip");
        let new_file = cache_dir.join("new_file.zip");

        fs::write(&old_file, b"old content")?;
        fs::write(&new_file, b"new content")?;

        // Make old file appear old
        let old_time = SystemTime::now() - Duration::from_secs(60 * 60 * 24 * 40); // 40 days ago
        filetime::set_file_mtime(&old_file, filetime::FileTime::from_system_time(old_time))?;

        unsafe {
            std::env::set_var("XDG_CACHE_HOME", temp_dir.path());
        }

        // Actual cleanup should remove old file but keep new file
        let result = handle_cleanup(false, 30, false).await;
        assert!(result.is_ok());
        assert!(!old_file.exists());
        assert!(new_file.exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_cleanup_remove_all() -> Result<()> {
        let temp_dir = TempDir::new()?;
        // Set up environment variable for cache directory
        unsafe {
            std::env::set_var("XDG_CACHE_HOME", temp_dir.path());
        }
        // Create cache directory
        let cache_dir = paths::release_archive_dir();
        fs::create_dir_all(&cache_dir)?;

        // Create test files
        let file1 = cache_dir.join("file1.zip");
        let file2 = cache_dir.join("file2.zip");

        fs::write(&file1, b"content1")?;
        fs::write(&file2, b"content2")?;

        unsafe {
            std::env::set_var("XDG_CACHE_HOME", temp_dir.path());
        }

        // Remove all should clear everything
        let result = handle_cleanup(true, 30, false).await;
        assert!(result.is_ok());
        assert!(!file1.exists());
        assert!(!file2.exists());
        assert!(cache_dir.exists()); // Directory should still exist

        Ok(())
    }
}
