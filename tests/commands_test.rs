// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use suiup::commands::{parse_component_with_version, BinaryName, CommandMetadata};
    use suiup::handlers::switch::parse_binary_spec;

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
}
