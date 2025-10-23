#!/bin/bash
set -e

# Configuration
GITHUB_REPO="MystenLabs/suiup"
RELEASES_URL="https://github.com/${GITHUB_REPO}/releases"

# Set up colors for output
CYAN='\033[0;36m'
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

printf '%bsuiup installer script%b\n' "${CYAN}" "${NC}"
printf 'This script will install the suiup binary to your system.\n'

# Environment variables for controlling behavior:
# SUIUP_INSTALL_DIR - Custom installation directory
# SUIUP_SKIP_CHECKSUM - Set to "true" to skip file integrity verification (NOT RECOMMENDED)
# GITHUB_TOKEN - GitHub token for API authentication (avoids rate limits)
#
# Security Note: Skipping integrity verification reduces security. Only use SUIUP_SKIP_CHECKSUM 
# in trusted environments or when you can manually verify the downloaded binary.

# Global flag to track if integrity verification was skipped
SKIP_INTEGRITY_CHECK=false
INTEGRITY_SKIP_REASON=""

# Get latest version from GitHub
get_latest_version() {
    # Check if GITHUB_TOKEN is set and use it for authentication
    auth_header=""
    if [ -n "$GITHUB_TOKEN" ]; then
        auth_header="Authorization: Bearer $GITHUB_TOKEN"
    fi

    if command -v curl >/dev/null 2>&1; then
        if [ -n "$auth_header" ]; then
            curl -fsSL -H "$auth_header" "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" | grep '"tag_name":' | sed 's/.*"tag_name": "\([^"]*\)".*/\1/'
        else
            curl -fsSL "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" | grep '"tag_name":' | sed 's/.*"tag_name": "\([^"]*\)".*/\1/'
        fi
    elif command -v wget >/dev/null 2>&1; then
        if [ -n "$auth_header" ]; then
            wget --quiet --header="$auth_header" -O- "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" | grep '"tag_name":' | sed 's/.*"tag_name": "\([^"]*\)".*/\1/'
        else
            wget --quiet -O- "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" | grep '"tag_name":' | sed 's/.*"tag_name": "\([^"]*\)".*/\1/'
        fi
    else
        printf '%bError: Neither curl nor wget is available for fetching release information.%b\n' "${RED}" "${NC}"
        printf '\n%bNext steps:%b\n' "${CYAN}" "${NC}"
        printf '1. Install curl or wget using your system package manager:\n'
        printf '   - Ubuntu/Debian: sudo apt-get install curl\n'
        printf '   - CentOS/RHEL/Fedora: sudo yum install curl (or dnf install curl)\n'
        printf '   - macOS: curl is pre-installed, or install via Homebrew: brew install curl\n'
        printf '   - Windows: Download from https://curl.se/windows/ or use WSL\n\n'
        printf '2. Alternatively, download suiup manually:\n'
        printf '   - Visit: https://github.com/%s/releases\n' "$GITHUB_REPO"
        printf '   - Download the appropriate binary for your OS/architecture\n'
        printf '   - Extract and place in your PATH\n'
        exit 1
    fi
}

# Detect operating system
detect_os() {
    case "$(uname -s)" in
        Linux*)     echo "linux" ;;
        Darwin*)    echo "macos" ;;
        MINGW*|MSYS*|CYGWIN*) echo "windows" ;;
        *)          echo "unknown" ;;
    esac
}

# Detect architecture
detect_arch() {
    arch=$(uname -m)
    case "$arch" in
        x86_64|amd64) echo "x86_64" ;;
        arm64|aarch64) echo "arm64" ;;
        *)          echo "unknown" ;;
    esac
}

# Get the appropriate download URL
get_download_url() {
    os=$1
    arch=$2
    version=$3
    
    # Construct the filename based on OS and architecture
    if [ "$os" = "macos" ]; then
        echo "${RELEASES_URL}/download/${version}/suiup-macOS-${arch}.tar.gz"
    elif [ "$os" = "linux" ]; then
        echo "${RELEASES_URL}/download/${version}/suiup-Linux-musl-${arch}.tar.gz"
    elif [ "$os" = "windows" ]; then
        # Based on GitHub releases, Windows version is available for both x86_64 and arm64
        echo "${RELEASES_URL}/download/${version}/suiup-Windows-msvc-${arch}.zip"
    else
        echo ""
    fi
}

# Determine installation directory
get_install_dir() {
    os=$1
    
    if [ "$os" = "macos" ] || [ "$os" = "linux" ]; then
        # Use ~/.local/bin on Unix-like systems if it exists or can be created
        local_bin="$HOME/.local/bin"
        if [ -d "$local_bin" ] || mkdir -p "$local_bin" 2>/dev/null; then
            echo "$local_bin"
        else
            # Fallback to /usr/local/bin if we can write to it
            if [ -w "/usr/local/bin" ]; then
                echo "/usr/local/bin"
            else
                # Last resort, use a directory in home
                mkdir -p "$HOME/bin"
                echo "$HOME/bin"
            fi
        fi
    elif [ "$os" = "windows" ]; then
        # On Windows, use %USERPROFILE%\.local\bin
        win_dir="$HOME/.local/bin"
        mkdir -p "$win_dir"
        echo "$win_dir"
    else
        echo "$HOME/bin"
        mkdir -p "$HOME/bin"
    fi
}

# Check if the directory is in PATH
check_path() {
    dir=$1
    os=$2
    
    # Different path separators for different OSes
    separator=":"
    if [ "$os" = "windows" ]; then
        separator=";"
    fi
    
    # Check if directory is in PATH using appropriate separator
    case "${separator}$PATH${separator}" in
        *"${separator}$dir${separator}"*) 
            printf '%b%s is already in your PATH%b\n' "${GREEN}" "$dir" "${NC}"
            ;;
        *)
            printf '%bWarning: %s is not in your PATH%b\n' "${YELLOW}" "$dir" "${NC}"
            
            # Provide instructions based on OS
            if [ "$os" = "macos" ] || [ "$os" = "linux" ]; then
                printf 'Add the following to your shell profile (~/.bashrc, ~/.zshrc, etc.):\n'
                printf "%bexport PATH=\"%s:\$PATH\"%b\n" "${GREEN}" "$dir" "${NC}"
            elif [ "$os" = "windows" ]; then
                printf 'Add this directory to your PATH by running this in PowerShell:\n'
                printf "%b\$env:Path += \"%s%s\"%b\n" "${GREEN}" "$separator" "$dir" "${NC}"
                printf 'To make it permanent, add it through Windows System Properties:\n'
                printf 'Control Panel → System → Advanced system settings → Environment Variables\n'
            fi
            ;;
    esac
}

# Internal download function with configurable error handling
_download_file_internal() {
    url=$1
    output_file=$2
    silent_fail=${3:-false}
    message=${4:-"Downloading %s to %s...\n"}
    
    if [ "$silent_fail" = "false" ]; then
        printf "$message" "$url" "$output_file"
    fi
    
    # Check if GITHUB_TOKEN is set and use it for authentication
    auth_header=""
    if [ -n "$GITHUB_TOKEN" ]; then
        auth_header="Authorization: Bearer $GITHUB_TOKEN"
    fi

    if command -v curl >/dev/null 2>&1; then
        if [ -n "$auth_header" ]; then
            if [ "$silent_fail" = "true" ]; then
                curl -fsSL -H "$auth_header" "$url" -o "$output_file" 2>/dev/null
                return $?
            else
                curl -fsSL -H "$auth_header" "$url" -o "$output_file"
                return $?
            fi
        else
            if [ "$silent_fail" = "true" ]; then
                curl -fsSL "$url" -o "$output_file" 2>/dev/null
                return $?
            else
                curl -fsSL "$url" -o "$output_file"
                return $?
            fi
        fi
    elif command -v wget >/dev/null 2>&1; then
        if [ -n "$auth_header" ]; then
            if [ "$silent_fail" = "true" ]; then
                wget --quiet --header="$auth_header" "$url" -O "$output_file" 2>/dev/null
                return $?
            else
                wget --quiet --header="$auth_header" "$url" -O "$output_file"
                return $?
            fi
        else
            if [ "$silent_fail" = "true" ]; then
                wget --quiet "$url" -O "$output_file" 2>/dev/null
                return $?
            else
                wget --quiet "$url" -O "$output_file"
                return $?
            fi
        fi
    else
        if [ "$silent_fail" = "true" ]; then
            return 1
        else
            printf '%bError: Neither curl nor wget is available for downloading files.%b\n' "${RED}" "${NC}"
            printf '\n%bNext steps:%b\n' "${CYAN}" "${NC}"
            printf '1. Install curl or wget using your system package manager:\n'
            printf '   - Ubuntu/Debian: sudo apt-get install curl\n'
            printf '   - CentOS/RHEL/Fedora: sudo yum install curl (or dnf install curl)\n'
            printf '   - macOS: curl is pre-installed, or install via Homebrew: brew install curl\n'
            printf '   - Windows: Download from https://curl.se/windows/ or use WSL\n\n'
            printf '2. Alternatively, download suiup manually:\n'
            printf '   - Visit: %s\n' "$url"
            printf '   - Download and save the file manually\n'
            printf '   - Continue with manual installation\n'
            exit 1
        fi
    fi
}

# Download a file with curl or wget
download_file() {
    url=$1
    output_file=$2
    
    _download_file_internal "$url" "$output_file" false "Downloading %s to %s...\n"
}

# Verify download integrity with checksum
verify_download_integrity() {
    os=$1
    arch=$2
    version=$3
    binary_file=$4
    checksum_file=$5
    
    # Skip if explicitly requested
    if [ "$SUIUP_SKIP_CHECKSUM" = "true" ]; then
        SKIP_INTEGRITY_CHECK=true
        INTEGRITY_SKIP_REASON="Explicitly skipped by user (SUIUP_SKIP_CHECKSUM=true)"
        return 0
    fi
    
    # Get checksum URL
    checksum_url=$(get_checksum_url "$os" "$arch" "$version")
    if [ -z "$checksum_url" ]; then
        SKIP_INTEGRITY_CHECK=true
        INTEGRITY_SKIP_REASON="No checksum URL available for this version"
        return 0
    fi
    
    # Download checksum file
    if ! download_checksum "$checksum_url" "$checksum_file"; then
        SKIP_INTEGRITY_CHECK=true
        INTEGRITY_SKIP_REASON="Failed to download checksum file"
        return 0
    fi
    
    # Verify file integrity - THIS IS THE KEY FUNCTION CALL
    if ! verify_file_integrity "$binary_file" "$checksum_file"; then
        printf '%bError: File integrity check failed. Aborting installation for security.%b\n' "${RED}" "${NC}"
        printf '\n%bPossible causes and solutions:%b\n' "${CYAN}" "${NC}"
        printf '1. Corrupted download:\n'
        printf '   - Try running the installer again\n'
        printf '   - Check your network connection\n\n'
        printf '2. Outdated checksum file:\n'
        printf '   - The release may have been updated recently\n'
        printf '   - Wait a few minutes and try again\n\n'
        printf '3. Security concern:\n'
        printf '   - The file may have been tampered with\n'
        printf '   - Only proceed if you trust the source\n\n'
        printf '4. Skip integrity check (not recommended):\n'
        printf '   - Set SUIUP_SKIP_CHECKSUM=true environment variable\n'
        printf '   - Re-run the installer: SUIUP_SKIP_CHECKSUM=true %s\n' "$0"
        printf '   - Only use this if you understand the security implications\n\n'
        printf '5. Manual verification:\n'
        printf '   - Download from: https://github.com/%s/releases\n' "$GITHUB_REPO"
        printf '   - Verify checksums manually before installation\n'
        return 1
    fi
    
    return 0
}

# Get the checksum download URL
get_checksum_url() {
    os=$1
    arch=$2
    version=$3
    
    # Construct the checksum filename based on OS and architecture (matching GitHub releases format)
    if [ "$os" = "macos" ]; then
        echo "${RELEASES_URL}/download/${version}/suiup-macOS-${arch}.tar.gz.sha256"
    elif [ "$os" = "linux" ]; then
        echo "${RELEASES_URL}/download/${version}/suiup-Linux-musl-${arch}.tar.gz.sha256"
    elif [ "$os" = "windows" ]; then
        echo "${RELEASES_URL}/download/${version}/suiup-Windows-msvc-${arch}.zip.sha256"
    else
        echo ""
    fi
}

# Download checksum file
download_checksum() {
    checksum_url=$1
    checksum_file=$2
    
    _download_file_internal "$checksum_url" "$checksum_file" true "Downloading checksum file...\n"
}


# Verify file integrity using standard checksum tools
verify_file_integrity() {
    binary_file=$1
    checksum_file=$2
    
    # Check if checksum file exists
    if [ ! -f "$checksum_file" ]; then
        SKIP_INTEGRITY_CHECK=true
        INTEGRITY_SKIP_REASON="Checksum file not found"
        return 0  # Continue installation but mark as skipped
    fi
    
    printf 'Verifying file integrity using standard checksum tools...\n'
    
    # Determine checksum type from file extension
    case "$checksum_file" in
        *.sha256)
            checksum_type="SHA256"
            ;;
        *.md5)
            checksum_type="MD5"
            ;;
        *)
            SKIP_INTEGRITY_CHECK=true
            INTEGRITY_SKIP_REASON="Unknown checksum file format"
            return 0
            ;;
    esac
    
    # Change to the directory containing the files for verification
    original_dir=$(pwd)
    file_dir=$(dirname "$binary_file")
    file_name=$(basename "$binary_file")
    checksum_name=$(basename "$checksum_file")
    
    cd "$file_dir" || {
        SKIP_INTEGRITY_CHECK=true
        INTEGRITY_SKIP_REASON="Cannot change to file directory"
        return 0
    }
    
    # Use standard checksum verification commands
    verification_result=1
    if [ "$checksum_type" = "SHA256" ]; then
        # Extract expected hash from checksum file
        expected_hash=$(head -n 1 "$checksum_name" | grep -o '[a-fA-F0-9]\{64\}' | tr '[:upper:]' '[:lower:]')
        
        if [ -z "$expected_hash" ]; then
            printf 'Warning: Could not extract SHA256 hash from checksum file\n'
            printf 'Checksum file content:\n'
            cat "$checksum_name" | head -3
            # Do NOT mark as passed. Treat as a skipped verification with a clear reason.
            SKIP_INTEGRITY_CHECK=true
            INTEGRITY_SKIP_REASON="Checksum file has unexpected format (missing SHA256)"
            cd "$original_dir" || true
            return 0
        elif command -v sha256sum >/dev/null 2>&1; then
            # Linux/GNU style - calculate and compare manually
            actual_hash=$(sha256sum "$file_name" | cut -d' ' -f1 | tr '[:upper:]' '[:lower:]')
            printf 'Expected SHA256: %s\n' "$expected_hash"
            printf 'Actual SHA256:   %s\n' "$actual_hash"
            if [ "$expected_hash" = "$actual_hash" ]; then
                verification_result=0
            fi
        elif command -v shasum >/dev/null 2>&1; then
            # macOS style - calculate and compare manually
            actual_hash=$(shasum -a 256 "$file_name" | cut -d' ' -f1 | tr '[:upper:]' '[:lower:]')
            printf 'Expected SHA256: %s\n' "$expected_hash"
            printf 'Actual SHA256:   %s\n' "$actual_hash"
            if [ "$expected_hash" = "$actual_hash" ]; then
                verification_result=0
            fi
        elif command -v powershell.exe >/dev/null 2>&1; then
            # Windows PowerShell - manual verification since no -c option
            expected_hash=$(head -n 1 "$checksum_name" | cut -d' ' -f1 | tr '[:upper:]' '[:lower:]')
            actual_hash=$(powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "(Get-FileHash -Path '$file_name' -Algorithm SHA256).Hash.ToLower()" | tr -d '\r\n' | tr '[:upper:]' '[:lower:]')
            if [ "$expected_hash" = "$actual_hash" ]; then
                verification_result=0
            fi
        elif command -v certutil >/dev/null 2>&1; then
            # Windows certutil - manual verification
            expected_hash=$(head -n 1 "$checksum_name" | cut -d' ' -f1 | tr '[:upper:]' '[:lower:]')
            actual_hash=$(certutil -hashfile "$file_name" SHA256 | grep -v "hash" | grep -v "CertUtil" | tr -d ' \r\n' | tr '[:upper:]' '[:lower:]')
            if [ "$expected_hash" = "$actual_hash" ]; then
                verification_result=0
            fi
        fi
    elif [ "$checksum_type" = "MD5" ]; then
        # Extract expected hash from checksum file
        expected_hash=$(head -n 1 "$checksum_name" | grep -o '[a-fA-F0-9]\{32\}' | tr '[:upper:]' '[:lower:]')
        
        if [ -z "$expected_hash" ]; then
            printf 'Warning: Could not extract MD5 hash from checksum file\n'
            printf 'Checksum file content:\n'
            cat "$checksum_name" | head -3
            # Do NOT mark as passed. Treat as a skipped verification with a clear reason.
            SKIP_INTEGRITY_CHECK=true
            INTEGRITY_SKIP_REASON="Checksum file has unexpected format (missing MD5)"
            cd "$original_dir" || true
            return 0
        elif command -v md5sum >/dev/null 2>&1; then
            # Linux/GNU style - calculate and compare manually
            actual_hash=$(md5sum "$file_name" | cut -d' ' -f1 | tr '[:upper:]' '[:lower:]')
            if [ "$expected_hash" = "$actual_hash" ]; then
                verification_result=0
            fi
        elif command -v md5 >/dev/null 2>&1; then
            # macOS style - calculate and compare manually
            actual_hash=$(md5 -q "$file_name" | tr '[:upper:]' '[:lower:]')
            if [ "$expected_hash" = "$actual_hash" ]; then
                verification_result=0
            fi
        fi
    fi
    
    cd "$original_dir" || true
    
    # Check verification result
    if [ $verification_result -eq 0 ]; then
        printf '%b%s integrity verification passed ✓%b\n' "${GREEN}" "$checksum_type" "${NC}"
        return 0
    else
        # Check if we have any verification tool available
        verification_tools_available=false
        if [ "$checksum_type" = "SHA256" ]; then
            if command -v sha256sum >/dev/null 2>&1 || command -v shasum >/dev/null 2>&1 || command -v powershell.exe >/dev/null 2>&1 || command -v certutil >/dev/null 2>&1; then
                verification_tools_available=true
            fi
        elif [ "$checksum_type" = "MD5" ]; then
            if command -v md5sum >/dev/null 2>&1 || command -v md5 >/dev/null 2>&1; then
                verification_tools_available=true
            fi
        fi
        
        if [ "$verification_tools_available" = "false" ]; then
            SKIP_INTEGRITY_CHECK=true
            INTEGRITY_SKIP_REASON="No suitable $checksum_type verification tool available"
            return 0
        else
            printf '%bError: %s integrity verification failed!%b\n' "${RED}" "$checksum_type" "${NC}"
            printf 'The downloaded file may be corrupted or tampered with.\n'
            printf 'Checksum file: %s\n' "$checksum_file"
            return 1
        fi
    fi
}

# Check for existing binaries that might conflict
check_existing_binaries() {
    local install_dir=$1
    local os=$2
    local found_binaries=""
    local binary

    # List of binaries to check
    for binary in sui mvr walrus; do
        # Check if binary exists in PATH
        if command -v "$binary" >/dev/null 2>&1; then
            # Get the full path of the existing binary
            existing_path=$(command -v "$binary")
            # Only warn if it's not in our installation directory
            if [ "$existing_path" != "$install_dir/$binary" ]; then
                if [ -n "$found_binaries" ]; then
                    found_binaries="$found_binaries, $binary"
                else
                    found_binaries="$binary"
                fi
            fi
        fi
    done

    if [ -n "$found_binaries" ]; then
        printf '\n%bWarning: The following binaries are already installed on your system:%b\n' "${YELLOW}" "${NC}"
        printf '  %s\n' "$found_binaries"
        printf '\n%s\n' "This might cause conflicts with suiup-installed tools."
        printf '%s\n' "You have two options:"
        printf '1. Uninstall the existing binaries\n'
        printf '2. Ensure %s is listed BEFORE other directories in your PATH\n' "$install_dir"
        
        if [ "$os" = "macos" ] || [ "$os" = "linux" ]; then
            printf '\nTo check your current PATH order, run:\n'
            printf '%becho $PATH | tr ":" "\\n" | nl%b\n' "${CYAN}" "${NC}"
            printf '\nTo modify your PATH order, edit your shell profile (~/.bashrc, ~/.zshrc, etc.)\n'
            printf 'and ensure this line appears BEFORE any other PATH modifications:\n'
            printf '%bexport PATH="%s:$PATH"%b\n' "${GREEN}" "$install_dir" "${NC}"
        elif [ "$os" = "windows" ]; then
            printf '\nTo check your current PATH order in PowerShell, run:\n'
            printf '%b$env:Path -split ";" | ForEach-Object { $i++; Write-Host "$i. $_" }%b\n' "${CYAN}" "${NC}"
            printf '\nTo modify your PATH order:\n'
            printf '1. Open System Properties (Win + Pause/Break)\n'
            printf '2. Click "Environment Variables"\n'
            printf '3. Under "User variables", find and select "Path"\n'
            printf '4. Click "Edit" and move %s to the top of the list\n' "$install_dir"
        fi
    fi
}

# Main installation function
install_suiup() {
    os=$(detect_os)
    arch=$(detect_arch)
    version=$(get_latest_version)
    
    if [ -z "$version" ]; then
        printf '%bError: Could not fetch latest version from GitHub%b\n' "${RED}" "${NC}"
        printf '\n%bPossible causes and solutions:%b\n' "${CYAN}" "${NC}"
        printf '1. Network connectivity issues:\n'
        printf '   - Check your internet connection\n'
        printf '   - Try again in a few minutes\n\n'
        printf '2. GitHub API rate limiting:\n'
        printf '   - Set GITHUB_TOKEN environment variable with your personal access token\n'
        printf '   - Create token at: https://github.com/settings/tokens\n\n'
        printf '3. GitHub service issues:\n'
        printf '   - Check GitHub status at: https://www.githubstatus.com/\n\n'
        printf '4. Manual installation:\n'
        printf '   - Visit: https://github.com/%s/releases\n' "$GITHUB_REPO"
        printf '   - Download the latest release manually\n'
        printf '   - Extract and install manually\n'
        exit 1
    fi
    

    
    download_url=$(get_download_url "$os" "$arch" "$version")
    
    if [ -z "$download_url" ]; then
        printf '%bError: Unsupported OS or architecture: %s/%s%b\n' "${RED}" "$os" "$arch" "${NC}"
        printf '\n%bSupported platforms:%b\n' "${CYAN}" "${NC}"
        printf '- Linux: x86_64 (amd64), arm64 (aarch64)\n'
        printf '- macOS: x86_64 (Intel), arm64 (Apple Silicon)\n'
        printf '- Windows: x86_64 (amd64), arm64 (aarch64)\n\n'
        printf '%bNext steps:%b\n' "${CYAN}" "${NC}"
        printf '1. Check if your architecture is supported:\n'
        printf '   - Run: uname -m\n'
        printf '   - Compare with supported architectures above\n\n'
        printf '2. Alternative installation methods:\n'
        printf '   - Install from source using Cargo:\n'
        printf '     cargo install --git https://github.com/%s.git --locked\n' "$GITHUB_REPO"
        printf '   - Check for community builds for your platform\n\n'
        printf '3. Report platform request:\n'
        printf '   - Open an issue at: https://github.com/%s/issues\n' "$GITHUB_REPO"
        printf '   - Request support for %s/%s\n' "$os" "$arch"
        exit 1
    fi
    
    printf 'Detected OS: %s\n' "$os"
    printf 'Detected architecture: %s\n' "$arch"
    printf 'Latest version: %s\n' "$version"
    printf 'Download URL: %s\n' "$download_url"
    
    # Create temporary directory
    tmp_dir=$(mktemp -d)
    trap 'rm -rf "$tmp_dir"' EXIT
    
    # Download the binary
    if [ "$os" = "windows" ]; then
        binary_file="$tmp_dir/suiup.zip"
        checksum_file="$tmp_dir/suiup.zip.sha256"
    else
        binary_file="$tmp_dir/suiup.tar.gz"
        checksum_file="$tmp_dir/suiup.tar.gz.sha256"
    fi
    
    download_file "$download_url" "$binary_file"
    
    # Download and verify checksum file (unless skipped)
    if ! verify_download_integrity "$os" "$arch" "$version" "$binary_file" "$checksum_file"; then
        exit 1
    fi
    
    # Extract the binary
    printf 'Extracting binary...\n'
    if [ "$os" = "windows" ]; then
        # Windows binary is in zip format
        if command -v unzip >/dev/null 2>&1; then
            unzip -q "$binary_file" -d "$tmp_dir"
            source_binary="$tmp_dir/suiup.exe"
        elif command -v powershell.exe >/dev/null 2>&1; then
            # Use PowerShell Expand-Archive as fallback
            printf 'Using PowerShell to extract zip file...\n'
            powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "Expand-Archive -Path '$binary_file' -DestinationPath '$tmp_dir' -Force"
            source_binary="$tmp_dir/suiup.exe"
        else
            printf '%bError: No zip extraction tool found!%b\n' "${RED}" "${NC}"
            printf 'To extract Windows binaries, you need one of the following:\n\n'
            printf '%b1. Install unzip:%b\n' "${CYAN}" "${NC}"
            printf '   - On WSL/MSYS2/Cygwin: sudo apt-get install unzip (or equivalent)\n'
            printf '   - On Windows with Chocolatey: choco install unzip\n'
            printf '   - On Windows with Scoop: scoop install unzip\n'
            printf '   - Download from: http://gnuwin32.sourceforge.net/packages/unzip.htm\n\n'
            printf '%b2. Use PowerShell (recommended):%b\n' "${CYAN}" "${NC}"
            printf '   PowerShell should be available on all modern Windows systems.\n'
            printf '   If PowerShell is not available, please install it from:\n'
            printf '   https://docs.microsoft.com/en-us/powershell/scripting/install/installing-powershell\n\n'
            printf '%b3. Manual extraction:%b\n' "${CYAN}" "${NC}"
            printf '   - Download the zip file manually from: %s\n' "$download_url"
            printf '   - Extract suiup.exe using Windows built-in zip support\n'
            printf '   - Place suiup.exe in a directory that is in your PATH\n'
            exit 1
        fi
    else
        tar -xzf "$binary_file" -C "$tmp_dir"
        source_binary="$tmp_dir/suiup"
    fi
    
    # Install to appropriate directory (allow user override via SUIUP_INSTALL_DIR)
    install_dir="${SUIUP_INSTALL_DIR:-$(get_install_dir "$os")}"
    installed_path="$install_dir/suiup"
    if [ "$os" = "windows" ]; then
        installed_path="$install_dir/suiup.exe"
    fi
    
    printf 'Installing to %s...\n' "$installed_path"
    
    # Ensure install directory exists
    mkdir -p "$install_dir"
    
    # Move binary to install directory
    mv "$source_binary" "$installed_path"
    
    printf '%bSuccessfully installed suiup to %s%b\n' "${GREEN}" "$installed_path" "${NC}"
    
    # Show critical security warning if integrity check was skipped
    if [ "$SKIP_INTEGRITY_CHECK" = "true" ]; then
        printf '\n%b⚠️  SECURITY WARNING ⚠️%b\n' "${RED}" "${NC}"
        printf '%b╔══════════════════════════════════════════════════════════════════════════════════╗%b\n' "${RED}" "${NC}"
        printf '%b║                       FILE INTEGRITY VERIFICATION SKIPPED                       ║%b\n' "${RED}" "${NC}"
        printf '%b║                                                                                  ║%b\n' "${RED}" "${NC}"
        printf '%b║ Reason: %-69s║%b\n' "${RED}" "$INTEGRITY_SKIP_REASON" "${NC}"
        printf '%b║                                                                                  ║%b\n' "${RED}" "${NC}"
        printf '%b║ This means the downloaded binary was NOT verified against official checksums.    ║%b\n' "${RED}" "${NC}"
        printf '%b║ The installation may be compromised or corrupted.                               ║%b\n' "${RED}" "${NC}"
        printf '%b║                                                                                  ║%b\n' "${RED}" "${NC}"
        printf '%b║ RECOMMENDED ACTIONS:                                                            ║%b\n' "${RED}" "${NC}"
        printf '%b║ • Verify the binary manually if possible                                        ║%b\n' "${RED}" "${NC}"
        printf '%b║ • Only use if you trust the source completely                                   ║%b\n' "${RED}" "${NC}"
        printf '%b║ • Consider reinstalling when the verification issue is resolved                 ║%b\n' "${RED}" "${NC}"
        printf '%b╚══════════════════════════════════════════════════════════════════════════════════╝%b\n' "${RED}" "${NC}"
        printf '\n'
    fi
    
    # Check PATH
    check_path "$install_dir" "$os"
    
    # Check for existing binaries
    check_existing_binaries "$install_dir" "$os"
    
    printf '\n'
    printf 'You can now run %bsuiup --help%b to get started.\n' "${CYAN}" "${NC}"
    printf 'For more information, visit: https://github.com/%s\n' "$GITHUB_REPO"
}

# Run the installer
install_suiup
