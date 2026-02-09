#!/bin/sh
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
        printf '%bError: Neither curl nor wget is available. Please install one of them.%b\n' "${RED}" "${NC}"
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
    
    # POSIX-compliant way to check if directory is in PATH
    case ":$PATH:" in
        *":$dir:"*) 
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

# Download a file with curl or wget
download_file() {
    url=$1
    output_file=$2
    
    printf 'Downloading %s to %s...\n' "$url" "$output_file"
    
    # Check if GITHUB_TOKEN is set and use it for authentication
    auth_header=""
    if [ -n "$GITHUB_TOKEN" ]; then
        auth_header="Authorization: Bearer $GITHUB_TOKEN"
    fi

    if command -v curl >/dev/null 2>&1; then
        if [ -n "$auth_header" ]; then
            curl -fsSL -H "$auth_header" "$url" -o "$output_file"
        else
            curl -fsSL "$url" -o "$output_file"
        fi
    elif command -v wget >/dev/null 2>&1; then
        if [ -n "$auth_header" ]; then
            wget --quiet --header="$auth_header" "$url" -O "$output_file"
        else
            wget --quiet "$url" -O "$output_file"
        fi
    else
        printf '%bError: Neither curl nor wget is available. Please install one of them.%b\n' "${RED}" "${NC}"
        exit 1
    fi
}

# Check for existing binaries that might conflict
check_existing_binaries() {
    local install_dir=$1
    local os=$2
    local found_binaries=""
    local binary
    local binary_to_check

    # List of binaries to check
    for binary in sui mvr walrus site-builder; do
        # Add .exe extension for Windows binaries
        if [ "$os" = "windows" ]; then
            binary_to_check="${binary}.exe"
        else
            binary_to_check="$binary"
        fi

        # Check if binary exists in PATH
        if command -v "$binary_to_check" >/dev/null 2>&1; then
            # Binary exists, add it to the warning list
            if [ -n "$found_binaries" ]; then
                found_binaries="$found_binaries, $binary"
            else
                found_binaries="$binary"
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
        printf '%bError: Could not fetch latest version%b\n' "${RED}" "${NC}"
        exit 1
    fi
    
    download_url=$(get_download_url "$os" "$arch" "$version")
    
    if [ -z "$download_url" ]; then
        printf '%bError: Unsupported OS or architecture: %s/%s%b\n' "${RED}" "$os" "$arch" "${NC}"
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
    else
        binary_file="$tmp_dir/suiup.tar.gz"
    fi

    download_file "$download_url" "$binary_file"

    # Extract the binary
    printf 'Extracting binary...\n'
    if [ "$os" = "windows" ]; then
        unzip -q "$binary_file" -d "$tmp_dir"
    else
        tar -xzf "$binary_file" -C "$tmp_dir"
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
    if [ "$os" = "windows" ]; then
        mv "$tmp_dir/suiup.exe" "$installed_path"
    else
        mv "$tmp_dir/suiup" "$installed_path"
    fi
    
    printf '%bSuccessfully installed suiup to %s%b\n' "${GREEN}" "$installed_path" "${NC}"
    
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
