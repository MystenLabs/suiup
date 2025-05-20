#!/bin/bash
set -e

# Configuration
GITHUB_REPO="MystenLabs/suiup"
BINARY_NAME="suiup"
RELEASES_URL="https://github.com/${GITHUB_REPO}/releases"

# Set up colors for output
CYAN='\033[0;36m'
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

printf "${CYAN}suiup installer script${NC}\n"
printf "This script will install the suiup binary to your system.\n"

# Get latest version from GitHub
get_latest_version() {
    # Check if GITHUB_TOKEN is set and use it for authentication
    local auth_header=""
    if [ -n "$GITHUB_TOKEN" ]; then
        auth_header="Authorization: Bearer $GITHUB_TOKEN"
    fi

    if command -v curl &>/dev/null; then
        if [ -n "$auth_header" ]; then
            curl -fsSL -H "$auth_header" "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/'
        else
            curl -fsSL "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/'
        fi
    elif command -v wget &>/dev/null; then
        if [ -n "$auth_header" ]; then
            wget --quiet --header="$auth_header" -O- "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/'
        else
            wget --quiet -O- "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/'
        fi
    else
        echo -e "${RED}Error: Neither curl nor wget is available. Please install one of them.${NC}"
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
    local arch=$(uname -m)
    case "$arch" in
        x86_64|amd64) echo "x86_64" ;;
        arm64|aarch64) echo "arm64" ;;
        *)          echo "unknown" ;;
    esac
}

# Get the appropriate download URL
get_download_url() {
    local os=$1
    local arch=$2
    local version=$3
    
    # Construct the filename based on OS and architecture
    if [ "$os" = "macos" ]; then
        echo "${RELEASES_URL}/download/${version}/suiup-macOS-${arch}.tar.gz"
    elif [ "$os" = "linux" ]; then
        echo "${RELEASES_URL}/download/${version}/suiup-Linux-musl-${arch}.tar.gz"
    elif [ "$os" = "windows" ]; then
        echo "${RELEASES_URL}/download/${version}/suiup-x86_64-pc-windows-msvc"  # Adjust if needed
    else
        echo ""
    fi
}

# Determine installation directory
get_install_dir() {
    local os=$1
    
    if [ "$os" = "macos" ] || [ "$os" = "linux" ]; then
        # Use ~/.local/bin on Unix-like systems if it exists or can be created
        local local_bin="$HOME/.local/bin"
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
        local win_dir="$HOME/.local/bin"
        mkdir -p "$win_dir"
        echo "$win_dir"
    else
        echo "$HOME/bin"
        mkdir -p "$HOME/bin"
    fi
}

# Check if the directory is in PATH
check_path() {
    local dir=$1
    local os=$2
    
    # Different path separators for different OSes
    local separator=":"
    if [ "$os" = "windows" ]; then
        separator=";"
    fi
    
    if [[ ":$PATH:" != *":$dir:"* ]]; then
        printf "${YELLOW}Warning: $dir is not in your PATH${NC}\n"
        
        # Provide instructions based on OS
        if [ "$os" = "macos" ] || [ "$os" = "linux" ]; then
            printf "Add the following to your shell profile (~/.bashrc, ~/.zshrc, etc.):\n"
            printf "${GREEN}export PATH=\"$dir:\$PATH\"${NC}\n"
        elif [ "$os" = "windows" ]; then
            printf "Add this directory to your PATH by running this in PowerShell:\n"
            printf "${GREEN}\$env:Path += \"$separator$dir\"${NC}\n"
            printf "To make it permanent, add it through Windows System Properties:\n"
            printf "Control Panel → System → Advanced system settings → Environment Variables\n"
        fi
    else
        printf "${GREEN}$dir is already in your PATH${NC}\n"
    fi
}

# Download a file with curl or wget
download_file() {
    local url=$1
    local output_file=$2
    
    echo "Downloading $url to $output_file..."
    
    # Check if GITHUB_TOKEN is set and use it for authentication
    local auth_header=""
    if [ -n "$GITHUB_TOKEN" ]; then
        auth_header="Authorization: Bearer $GITHUB_TOKEN"
    fi

    if command -v curl &>/dev/null; then
        if [ -n "$auth_header" ]; then
            curl -fsSL -H "$auth_header" "$url" -o "$output_file"
        else
            curl -fsSL "$url" -o "$output_file"
        fi
    elif command -v wget &>/dev/null; then
        if [ -n "$auth_header" ]; then
            wget --quiet --header="$auth_header" "$url" -O "$output_file"
        else
            wget --quiet "$url" -O "$output_file"
        fi
    else
        echo -e "${RED}Error: Neither curl nor wget is available. Please install one of them.${NC}"
        exit 1
    fi
}

# Main installation function
install_suiup() {
    local os=$(detect_os)
    local arch=$(detect_arch)
    local version=$(get_latest_version)
    
    if [ -z "$version" ]; then
        printf "${RED}Error: Could not fetch latest version${NC}\n"
        exit 1
    fi
    
    local download_url=$(get_download_url "$os" "$arch" "$version")
    
    if [ -z "$download_url" ]; then
        printf "${RED}Error: Unsupported OS or architecture: $os/$arch${NC}\n"
        exit 1
    fi
    
    printf "Detected OS: $os\n"
    printf "Detected architecture: $arch\n"
    printf "Latest version: $version\n"
    printf "Download URL: $download_url\n"
    
    # Create temporary directory
    local tmp_dir=$(mktemp -d)
    trap 'rm -rf "$tmp_dir"' EXIT
    
    # Download the binary
    local binary_file="$tmp_dir/suiup.tar.gz"
    
    download_file "$download_url" "$binary_file"
    
    # Extract the binary
    printf "Extracting binary...\n"
    if [ "$os" = "windows" ]; then
        printf "No extraction needed for Windows binaries.\n"
    else
        tar -xzf "$binary_file" -C "$tmp_dir"
    fi
    
    # Install to appropriate directory
    local install_dir=$(get_install_dir "$os")
    local installed_path="$install_dir/suiup"
    if [ "$os" = "windows" ]; then
        installed_path="$install_dir/suiup.exe"
    fi
    
    printf "Installing to $installed_path...\n"
    
    # Ensure install directory exists
    mkdir -p "$install_dir"
    
    # Move binary to install directory
    mv "$tmp_dir/suiup" "$installed_path"
    
    printf "${GREEN}Successfully installed suiup to $installed_path${NC}\n"
    
    # Check PATH
    check_path "$install_dir" "$os"
    
    printf "\n"
    printf "You can now run ${CYAN}suiup --help${NC} to get started.\n"
    printf "For more information, visit: https://github.com/$GITHUB_REPO\n"
}

# Run the installer
install_suiup
