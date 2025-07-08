# Suiup - the installer and manager for CLI tools in the Sui ecosystem
`suiup` is a tool to install and manage different versions of CLI tools for working in the Sui ecosystem. It allows you to easily install and switch between different versions of `sui`, `mvr`, `walrus`.
After installation, run `suiup list` to find which binaries you can install. Check out the [Installation](#installation) to install and the [Quick Start](#quick-start) guide for how-to-use examples.

# Why suiup? 
The Sui CLI has long been available through tools like Homebrew, Chocolatey, or cargo install, or by downloading binaries manually. But with Sui releasing updates every two weeks, developers often need to upgrade frequently or use a specific version (e.g., for devnet or debug builds). Switching between versions or installing non-package-managers builds is not as simple as we want it to be.

As new tools like [mvr](https://docs.suins.io/move-registry#using-mvr) and [walrus](https://docs.wal.app/usage/setup.html) were introduced—without support from package managers—managing them became even harder due to manual installation steps.

`suiup` solves this by offering a simple way to install and manage multiple CLI tools and their versions in the Sui ecosystem.

With `suiup`, you can:

- Install specific versions of `sui`, `mvr`, or `walrus` (with more tools coming soon)
- Install from a branch in the repository to try unreleased features or fixes
- Get debug builds of `sui` (needed for commands like `sui move test --coverage`)
- List and switch between installed binaries for different networks (e.g., devnet vs mainnet)
- Simplify tool installation in CI environments

# Supported OS (for suiup tool, but not necessarily for the binaries it installs)

| OS       | Architecture      | Status         |
|----------|-------------------|----------------|
| Linux    | x86_64 (amd64)    | ✅ Supported   |
| Linux    | aarch64 (ARM64)   | ✅ Supported   |
| macOS    | x86_64 (amd64)    | ✅ Supported   |
| macOS    | aarch64 (ARM64)   | ✅ Supported   |
| Windows  | x86_64 (amd64)    | ✅ Supported   |
| Windows  | aarch64 (ARM64)   | Limited support (might or might not work) |

# Installation

### From Script
```bash
curl -sSfL https://raw.githubusercontent.com/Mystenlabs/suiup/main/install.sh | sh
```

### From Cargo
```bash
cargo install --git https://github.com/Mystenlabs/suiup.git --locked
```

### From Release

1. Download the latest release from [Releases](https://github.com/Mystenlabs/suiup/releases).
2. Unzip the downloaded file.
3. Add the `suiup` binary to folder that is on your `PATH` environment variable, or add that folder to the `PATH`.
4. (Optional) restart your terminal if you made changes to your `PATH`.

# Prerequisites

``Path Ordering Matters``

If you already have installed one of sui/mvr/walrus binaries before, you will need to make sure that you either remove those binaries
or set the `PATH` to the `/.local/bin` (MacOS/Linux or equivalent for Windows) before the `PATH` where those binaries are installed.

# Quick Start

It's recommended to read the whole quick start to familiarize yourself with the commands.

> [!TIP]
> Pass the `--yes (-y)` flag to skip confirmation prompts, thus accepting to updating the default binary to the one you are installing.

### Install `sui` -- this will install the latest available `testnet` release
```bash
suiup install sui@testnet
```

### Install `sui` with specific release (and version)
```bash
suiup install sui@devnet # this will install the latest available devnet release
suiup install sui@testnet-1.40.1 # this will install the testnet v1.40.1 release
```

> [!TIP]
> You can also use `==` or `=` to specify a version: `sui@testnet-1.44.2` is the same as `sui==testnet-1.44.2` or `sui=testnet-1.44.2`.

> [!NOTE]
> You can just pass the `@1.44.2` version instead of `sui@testnet-1.44.2` or omit it altogether `suiup install sui`, but you must remember
that the default will be testnet release for `sui/walrus`. It's recommended to pass the release for the network you want to install.

### Update `sui` to latest version
This will check for newer releases of those that are already installed, and then download the new ones. Recommended to specify which release to update.
```bash
suiup update sui@devnet # recommended
suiup update sui # alternative - not recommended, as it will update/install the latest testnet release
```

### Install `sui` binary to specific default directory
```bash
SUIUP_DEFAULT_BIN_DIR=/path/to/default_dir suiup install sui -y
```

### Install `walrus` (note that walrus release are available starting with v1.17.1 for devnet/testnet and v1.18.2 for mainnet)
```bash
suiup install walrus -y
```

### Install `mvr` (Move Registry CLI)
```bash
suiup install mvr
suiup install mvr@0.0.8 # this will install the MVR CLI v0.0.8 release
```

### List available binaries to install
```bash
suiup list
```

### Show installed versions
```bash
suiup show
```

### Switch between versions. Note that `default set` requires to specify a version!
```bash
suiup default get
suiup default set sui@testnet-1.40.0
suiup default set mvr@0.0.7
suiup default set sui@testnet-1.40.0 --debug # set the default version to be the sui-debug binary
```

### Show where the default binaries are installed
```bash
suiup which
```

# Advanced Usage

### Pre-requisites
- [Rust](https://www.rust-lang.org/tools/install) (if you want to install from branch)
- [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html) (if you want to install from branch)
- [Git](https://git-scm.com/downloads) (if you want to install from branch)

Installing a nightly version is highly experimental and might not work as expected. Avoid using it unless you really need to.

> [!IMPORTANT]
> `--nightly` will replace the current nightly binary, if any. Currently, there's no support for multiple nightly versions. Hope to add it in the future!

### Install from branch (requires cargo + rust installed!)


```bash
suiup install mvr --nightly # installs from main if branch name is omitted
suiup install mvr --nightly my_branch
```
> [!NOTE]
> There is a `--debug` flag that can be used in two ways:
> - for `sui` binary, it will install the `sui-debug` binary from the release archive which contains debug symbols and it's required to run `sui move test --coverage`.
> - for when using `--nightly`, it will build the binary from source with debug symbols. By default, `--nightly` builds in release mode as per `cargo install`'s defaults.

Note that installing from a branch and specifying a version are mutually exclusive (in other words, `suiup install sui@some-version --nightly some-branch` will cause an error).

### Install MVR from nightly in debug mode
```bash
suiup install mvr --nightly --debug
```

### Switch default versions
```bash
suiup default set sui --nightly
```

### Using it in CI
As the tool requires to download releases and files from GitHub, it is recommended to use a GitHub token to avoid rate limits. You can set the `GITHUB_TOKEN` environment variable to your GitHub token or pass in the `--github-token` argument.

In the CI environment, you can set the `GITHUB_TOKEN` environment variable to your GitHub token, then you can run the `suiup` command as usual:
```bash
env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

steps:
- name: Install sui
  run: |
    suiup install sui
```

Or if you're calling from shell:
```bash
GITHUB_TOKEN=your_github_token suiup install sui
```

## Paths used by the `suiup` tool

> [!TIP]
> Set `SUIUP_DEFAULT_BIN_DIR` environment variable to specify a different directory for storing the default binaries. The tool will warn if this folder is not on the path and suggest how to add it.

**[Unix/MacOS]**
The tool uses these environment variables to store data.
- `XDG_DATA_HOME`
- `XDG_CACHE_HOME`
- `XDG_CONFIG_HOME`
- `HOME/.local/bin` for storing default binaries to be used. Make sure this is on your `PATH` or set up `SUIUP_DEFAULT_BIN_DIR` env variable to point to a different directory.

**[Windows]**
- `LOCALAPPDATA` or `USERPROFILE\AppData\Local` for storing data
- `TEMP` or `USERPROFILE\AppData\Local\Temp` for caching
- `LOCALAPPDATA\bin` for storing default binaries to be used


## Known issues
- `suiup install mvr --nightly` might fail on **Windows** because of issues with compiling the `mvr-cli` crate from the repository. Just install the latest release instead.
- `suiup remove` does not work well. Do not use it.
- There is no cleanup functionality, so the cache might grow bigger.
## Troubleshooting

### `suiup` is not recognized as a command

Make sure the folder where the `suiup` binary is located is on the `PATH` environment variable. You can also add the folder where the `suiup` binary is located to the `PATH` environment variable.

### `suiup` is not working as expected

Please open an issue on the GitHub repository with the details of the problem you are facing.

### `suiup` is not downloading the binaries

Make sure you have an active internet connection. If you are behind a proxy, you might need to figure it out yourself. The tool does not support proxy settings yet.

### Cannot run the binaries, even though they are installed and set as default

Make sure the folder where the default binaries are stored is on the `PATH` environment variable. You can use `suiup which` to see where the default binaries are stored.

### It looks like it's not calling the right binaries, the binary version does not change

The order of the folders in the `PATH` environment variable matters. Make sure the folder where the default binaries are stored (see above) is before the folder where you might already have
some other versions of these binaries copied to. In Unix/MacOS use `which sui/mvr/walrus` to see the path of the binary that is being called.
Use `suiup which` to see where the default binaries are stored.

### Where are the default binaries copied to?

For Unix/MacOS they are copied to `$HOME/.local/bin` (or where your `SUIUP_DEFAULT_BIN_DIR` env var points to) and for Windows they are copied to `LOCALAPPDATA\bin`.
Make sure you have these folders on the `PATH`.


# Disclaimer

This software is provided “as is”, without warranty of any kind, express or implied.
