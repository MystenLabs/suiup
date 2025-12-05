#!/usr/bin/env python3
"""
Update script to fetch latest standalone releases and update releases.json
"""

import json
import subprocess
import sys
import tempfile
import re
import argparse
import os
from pathlib import Path
from typing import Dict, List, Optional
import urllib.request
import urllib.error

# Configuration
# Note: Currently only mvr has standalone binaries with the naming pattern `mvr-ubuntu-x86_64`
# Other tools use .tgz archives with different naming patterns:
# - walrus: walrus-testnet-vX.Y.Z-ubuntu-x86_64.tgz
# - walrus-sites: site-builder-mainnet-vX.Y.Z-ubuntu-x86_64.tgz
# - sui: does not have standalone binaries in releases
REPOS = {
    "mvr": "MystenLabs/mvr",
    "sui": "MystenLabs/sui",
    "walrus": "MystenLabs/walrus",
    "walrus-sites": "MystenLabs/walrus-sites",
}

NUM_RELEASES = 1  # Number of releases to fetch per network/type


# Global flag for color output (will be set by main())
USE_COLOR = True


# Colors for output
class Colors:
    @staticmethod
    def _color(code: str) -> str:
        """Return color code if colors are enabled, empty string otherwise."""
        return code if USE_COLOR else ""

    @property
    def RED(self) -> str:
        return self._color("\033[0;31m")

    @property
    def GREEN(self) -> str:
        return self._color("\033[0;32m")

    @property
    def YELLOW(self) -> str:
        return self._color("\033[1;33m")

    @property
    def BLUE(self) -> str:
        return self._color("\033[0;34m")

    @property
    def NC(self) -> str:
        return self._color("\033[0m")


# Create a singleton instance
Colors = Colors()


def get_filename_pattern(binary: str) -> str:
    """Get the filename pattern for a binary.

    Returns a pattern that will be matched against asset names.
    For .tgz archives, we match the pattern within the filename.
    """
    patterns = {
        "mvr": "mvr-ubuntu-x86_64",  # Direct binary: mvr-ubuntu-x86_64
        "sui": "sui-.*-ubuntu-x86_64.tgz",  # Archive: sui-testnet-v1.59.0-ubuntu-x86_64.tgz
        "walrus": "walrus-.*-ubuntu-x86_64.tgz",  # Archive: walrus-testnet-v1.35.0-ubuntu-x86_64.tgz
        "walrus-sites": "site-builder-.*-ubuntu-x86_64.tgz",  # Archive: site-builder-mainnet-v1.3.0-ubuntu-x86_64.tgz
    }
    return patterns.get(binary, f"{binary}-ubuntu-x86_64")


def fetch_releases(binary: str, repo: str) -> List[Dict[str, str]]:
    """Fetch releases from GitHub API."""
    print(
        f"{Colors.BLUE}Fetching latest {NUM_RELEASES} releases for {binary}...{Colors.NC}",
        file=sys.stderr,
    )

    url = f"https://api.github.com/repos/{repo}/releases"
    filename_pattern = get_filename_pattern(binary)

    try:
        req = urllib.request.Request(url)
        req.add_header("User-Agent", "suiup-update-script")

        with urllib.request.urlopen(req) as response:
            releases_data = json.loads(response.read())
    except urllib.error.URLError as e:
        print(
            f"{Colors.RED}Failed to fetch releases from GitHub: {e}{Colors.NC}",
            file=sys.stderr,
        )
        return []

    # Filter releases that have the required asset
    # Compile pattern as regex
    pattern_re = re.compile(filename_pattern)

    filtered_releases = []

    # For tools with network variants (sui, walrus, walrus-sites), try to get diverse networks
    # Track which network types we've seen
    network_counts = {}

    for release in releases_data[
        : NUM_RELEASES * 10
    ]:  # Search more releases to find diversity
        assets = release.get("assets", [])
        matching_asset = None

        for asset in assets:
            # Use regex match for .tgz patterns, exact match for direct binaries
            if pattern_re.fullmatch(asset["name"]):
                matching_asset = asset
                break

        if matching_asset:
            tag = release["tag_name"]

            # Extract network type from tag (e.g., "mainnet-v1.58.3" -> "mainnet")
            network = tag.split("-")[0] if "-" in tag else "default"

            # For network-based releases, limit to NUM_RELEASES per network
            if network != "default":
                network_counts[network] = network_counts.get(network, 0) + 1
                if network_counts[network] > NUM_RELEASES:
                    continue

            filtered_releases.append(
                {
                    "tag": tag,
                    "url": matching_asset["browser_download_url"],
                    "filename": matching_asset["name"],
                }
            )

            # Stop if we have enough overall or enough per network
            total_desired = NUM_RELEASES * 3  # Get up to 3 networks worth
            if len(filtered_releases) >= total_desired:
                break

    if not filtered_releases:
        print(
            f"{Colors.YELLOW}Warning: No releases found for {binary}{Colors.NC}",
            file=sys.stderr,
        )

    return filtered_releases


def compute_hash(url: str) -> Optional[str]:
    """Download a file and compute its Nix SRI hash."""
    try:
        with tempfile.NamedTemporaryFile(delete=False) as tmp_file:
            tmp_path = tmp_file.name

            # Download the file
            req = urllib.request.Request(url)
            req.add_header("User-Agent", "suiup-update-script")

            with urllib.request.urlopen(req) as response:
                tmp_file.write(response.read())

        # Compute SHA256 hash
        result = subprocess.run(
            ["nix-hash", "--type", "sha256", "--flat", tmp_path],
            capture_output=True,
            text=True,
            check=True,
        )
        hash_hex = result.stdout.strip()

        # Convert to SRI format
        result = subprocess.run(
            [
                "nix",
                "hash",
                "convert",
                "--hash-algo",
                "sha256",
                "--to",
                "sri",
                hash_hex,
            ],
            capture_output=True,
            text=True,
            check=True,
        )
        sri_hash = result.stdout.strip()

        # Clean up
        Path(tmp_path).unlink()

        return sri_hash
    except Exception as e:
        print(f"{Colors.RED}Failed to compute hash: {e}{Colors.NC}", file=sys.stderr)
        return None


def generate_releases_for_binary(binary: str, repo: str) -> Dict[str, Dict[str, str]]:
    """Generate version -> {hash, url} mapping for a binary."""
    releases = fetch_releases(binary, repo)

    if not releases:
        return {}

    result = {}
    for release in releases:
        tag = release["tag"]
        url = release["url"]

        print(
            f"{Colors.GREEN}  Processing {binary} {tag}...{Colors.NC}", file=sys.stderr
        )

        hash_value = compute_hash(url)
        if not hash_value:
            print(
                f"{Colors.RED}  Failed to compute hash for {tag}{Colors.NC}",
                file=sys.stderr,
            )
            continue

        print(f"{Colors.GREEN}  Hash: {hash_value}{Colors.NC}", file=sys.stderr)
        result[tag] = {
            "hash": hash_value,
            "url": url,
        }

    return result


def cleanup_old_releases(
    releases: Dict[str, Dict[str, str]], max_per_network: int = 10
) -> tuple[Dict[str, Dict[str, str]], List[str]]:
    """Keep only the latest N releases per network type.

    Args:
        releases: Dict of version -> release info
        max_per_network: Maximum number of releases to keep per network (default: 10)

    Returns:
        Tuple of (cleaned up releases dict, list of removed versions)
    """
    # Group releases by network
    network_groups: Dict[str, List[str]] = {}
    for version in releases.keys():
        # Extract network from version (e.g., "mainnet-v1.58.3" -> "mainnet")
        network = version.split("-")[0] if "-" in version else "default"
        if network not in network_groups:
            network_groups[network] = []
        network_groups[network].append(version)

    # Keep only latest releases per network
    versions_to_keep = set()
    removed_versions = []
    for network, versions in network_groups.items():
        # Sort versions in descending order (latest first)
        sorted_versions = sorted(versions, reverse=True)
        # Keep only the latest max_per_network
        kept = sorted_versions[:max_per_network]
        removed = sorted_versions[max_per_network:]
        versions_to_keep.update(kept)
        removed_versions.extend(removed)

    # Return filtered releases and list of removed versions
    return {
        v: r for v, r in releases.items() if v in versions_to_keep
    }, removed_versions


def main():
    """Main script execution."""
    parser = argparse.ArgumentParser(
        description="Update standalone releases JSON with latest GitHub releases",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s                              # Update releases.json in current directory
  %(prog)s nix/releases.json            # Update specific file
  %(prog)s --force                      # Re-download all releases
  %(prog)s --max-releases 5             # Keep only 5 latest per network
  %(prog)s --no-color                   # Disable colored output
        """,
    )
    parser.add_argument(
        "file",
        nargs="?",
        default="releases.json",
        help="Path to releases.json file to update (default: %(default)s)",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Re-download and recompute hashes for existing releases",
    )
    parser.add_argument(
        "--max-releases",
        type=int,
        default=10,
        metavar="N",
        help="Maximum number of releases to keep per network (default: %(default)s)",
    )
    parser.add_argument(
        "--no-color",
        action="store_true",
        help="Disable colored output (also honors NO_COLOR environment variable)",
    )
    args = parser.parse_args()

    # Set color output based on --no-color flag or NO_COLOR environment variable
    global USE_COLOR
    USE_COLOR = not args.no_color and not os.environ.get("NO_COLOR")

    print(
        f"{Colors.GREEN}=== Updating Standalone Releases ==={Colors.NC}",
        file=sys.stderr,
    )
    print("", file=sys.stderr)

    releases_json_path = Path(args.file)

    # Create parent directory if it doesn't exist
    releases_json_path.parent.mkdir(parents=True, exist_ok=True)

    # Load existing releases
    existing_releases = {}
    if releases_json_path.exists():
        # Create backup
        backup_path = releases_json_path.with_suffix(".json.bak")
        backup_path.write_text(releases_json_path.read_text())
        print(
            f"{Colors.YELLOW}  Backup saved to {backup_path}{Colors.NC}",
            file=sys.stderr,
        )

        try:
            existing_releases = json.loads(releases_json_path.read_text())
        except json.JSONDecodeError:
            print(
                f"{Colors.YELLOW}Warning: Could not parse existing {releases_json_path}{Colors.NC}",
                file=sys.stderr,
            )

    print("", file=sys.stderr)

    # Build updated releases structure
    new_releases = {}
    # Track changes for summary
    changes: Dict[str, Dict[str, List[str]]] = {}

    for binary in ["mvr", "sui", "walrus", "walrus-sites"]:
        if binary not in REPOS:
            continue

        # Start with existing releases for this binary
        existing_binary_releases = existing_releases.get(binary, {})
        changes[binary] = {"added": [], "removed": []}

        if not args.force and existing_binary_releases:
            print(
                f"{Colors.BLUE}Using existing {binary} releases (use --force to re-download){Colors.NC}",
                file=sys.stderr,
            )
            # Still apply cleanup to existing releases
            cleaned_releases, removed = cleanup_old_releases(
                existing_binary_releases, args.max_releases
            )
            new_releases[binary] = cleaned_releases
            changes[binary]["removed"] = removed
        else:
            # Fetch new releases and merge with existing
            fetched_releases = generate_releases_for_binary(binary, REPOS[binary])

            # Track newly added versions
            for version in fetched_releases.keys():
                if version not in existing_binary_releases:
                    changes[binary]["added"].append(version)

            # Merge: prefer fetched (new) releases, but keep old ones not in new list
            merged_releases = existing_binary_releases.copy()
            merged_releases.update(fetched_releases)

            # Cleanup: keep only latest N per network
            cleaned_releases, removed = cleanup_old_releases(
                merged_releases, args.max_releases
            )
            new_releases[binary] = cleaned_releases
            changes[binary]["removed"] = removed

    # Write the updated JSON
    releases_json_path.write_text(json.dumps(new_releases, indent=2) + "\n")

    print("", file=sys.stderr)
    print(
        f"{Colors.GREEN}✓ Updated {releases_json_path} successfully{Colors.NC}",
        file=sys.stderr,
    )
    if releases_json_path.with_suffix(".json.bak").exists():
        print(
            f"{Colors.YELLOW}  Backup saved to {releases_json_path.with_suffix('.json.bak')}{Colors.NC}",
            file=sys.stderr,
        )

    # Display changes summary
    print("", file=sys.stderr)
    print(f"{Colors.GREEN}=== Changes Summary ==={Colors.NC}", file=sys.stderr)
    has_changes = False
    for binary, change_info in changes.items():
        if change_info["added"] or change_info["removed"]:
            has_changes = True
            print(f"\n{Colors.BLUE}{binary}:{Colors.NC}", file=sys.stderr)
            if change_info["added"]:
                print(f"  {Colors.GREEN}Added:{Colors.NC}", file=sys.stderr)
                for version in sorted(change_info["added"]):
                    print(f"    + {version}", file=sys.stderr)
            if change_info["removed"]:
                print(f"  {Colors.YELLOW}Removed:{Colors.NC}", file=sys.stderr)
                for version in sorted(change_info["removed"]):
                    print(f"    - {version}", file=sys.stderr)

    if not has_changes:
        print(f"  {Colors.YELLOW}No changes{Colors.NC}", file=sys.stderr)

    # Display all available versions per component
    print("", file=sys.stderr)
    print(f"{Colors.GREEN}=== Available Versions ==={Colors.NC}", file=sys.stderr)
    for binary in ["mvr", "sui", "walrus", "walrus-sites"]:
        versions = new_releases.get(binary, {})
        if versions:
            print(
                f"\n{Colors.BLUE}{binary}:{Colors.NC} ({len(versions)} versions)",
                file=sys.stderr,
            )
            # Group by network
            network_groups: Dict[str, List[str]] = {}
            for version in versions.keys():
                network = version.split("-")[0] if "-" in version else "default"
                if network not in network_groups:
                    network_groups[network] = []
                network_groups[network].append(version)

            # Display grouped by network
            for network in sorted(network_groups.keys()):
                network_versions = sorted(network_groups[network], reverse=True)
                print(f"  {network}: {', '.join(network_versions)}", file=sys.stderr)

    print("", file=sys.stderr)
    print(f"{Colors.GREEN}Example commands:{Colors.NC}", file=sys.stderr)
    print(
        f"{Colors.BLUE}  nix build '.#sui'           # Build latest mainnet sui{Colors.NC}",
        file=sys.stderr,
    )
    print(
        f"{Colors.BLUE}  nix build '.#walrus'        # Build latest mainnet walrus{Colors.NC}",
        file=sys.stderr,
    )
    print(
        f"{Colors.BLUE}  nix run .#update-releases   # Update releases{Colors.NC}",
        file=sys.stderr,
    )


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print(f"\n{Colors.YELLOW}Interrupted by user{Colors.NC}", file=sys.stderr)
        sys.exit(1)
    except Exception as e:
        print(f"{Colors.RED}Error: {e}{Colors.NC}", file=sys.stderr)
        sys.exit(1)
