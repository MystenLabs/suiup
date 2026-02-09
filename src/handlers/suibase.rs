// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result, anyhow, bail};
use std::env;
use std::ffi::OsStr;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const SUIBASE_REPO_URL: &str = "https://github.com/chainmovers/suibase.git";
const SUIBASE_DIR_ENV: &str = "SUIUP_SUIBASE_DIR";

#[derive(Clone, Copy, Debug, Default)]
pub struct ActionOptions {
    pub yes: bool,
    pub dry_run: bool,
}

pub fn install(opts: ActionOptions) -> Result<()> {
    let mut runner = SystemRunner;
    install_with_runner(opts, &mut runner, &suibase_dir()?)
}

pub fn update(opts: ActionOptions) -> Result<()> {
    let mut runner = SystemRunner;
    update_with_runner(opts, &mut runner, &suibase_dir()?)
}

pub fn uninstall(opts: ActionOptions) -> Result<()> {
    let mut runner = SystemRunner;
    uninstall_with_runner(opts, &mut runner, &suibase_dir()?)
}

pub fn doctor() -> Result<()> {
    ensure_supported_platform()?;

    let suibase_dir = suibase_dir()?;
    let local_bin = local_bin_dir()?;

    let mut failures = 0usize;

    println!("Checking suibase environment...");

    report_check(
        command_exists("git"),
        "git command available",
        "Install git and ensure it is in PATH",
        &mut failures,
    );
    report_check(
        command_exists("bash"),
        "bash command available",
        "Install bash and ensure it is in PATH",
        &mut failures,
    );

    let has_dir = suibase_dir.exists();
    report_check(
        has_dir,
        format!("suibase directory exists ({})", suibase_dir.display()),
        format!(
            "Run `suiup suibase install` to create {}",
            suibase_dir.display()
        ),
        &mut failures,
    );

    for script in ["install", "update", "uninstall"] {
        let script_path = suibase_dir.join(script);
        report_check(
            script_path.exists(),
            format!("{} script exists ({})", script, script_path.display()),
            format!("Run `suiup suibase install` to restore {}", script),
            &mut failures,
        );
    }

    let in_path = path_contains(&local_bin);
    report_warn(
        in_path,
        format!("PATH contains {}", local_bin.display()),
        format!(
            "Add to shell profile: export PATH=\"{}:$PATH\"",
            local_bin.display()
        ),
    );

    if failures > 0 {
        bail!(
            "Doctor found {failures} failing checks. Fix the FAIL items and rerun `suiup suibase doctor`."
        );
    }

    println!("Doctor completed with no failing checks.");
    Ok(())
}

fn install_with_runner(
    opts: ActionOptions,
    runner: &mut dyn Runner,
    suibase_dir: &Path,
) -> Result<()> {
    ensure_supported_platform()?;
    runner.ensure_available("git", opts.dry_run)?;
    runner.ensure_available("bash", opts.dry_run)?;

    if opts.yes {
        println!("Proceeding in non-interactive mode (--yes).");
    }

    if suibase_dir.exists() {
        println!(
            "Suibase already exists at {}. Pulling latest changes...",
            suibase_dir.display()
        );
        runner.run(
            "git",
            &[
                "-C".to_string(),
                suibase_dir.to_string_lossy().to_string(),
                "pull".to_string(),
                "--ff-only".to_string(),
            ],
            opts.dry_run,
            "Failed to pull suibase repository. Next step: inspect local changes under suibase directory.",
        )?;
    } else {
        println!("Cloning suibase into {} ...", suibase_dir.display());
        runner.run(
            "git",
            &[
                "clone".to_string(),
                SUIBASE_REPO_URL.to_string(),
                suibase_dir.to_string_lossy().to_string(),
            ],
            opts.dry_run,
            "Failed to clone suibase repository. Next step: verify network access and git credentials.",
        )?;
    }

    let install_script = suibase_dir.join("install");
    ensure_script_exists(
        &install_script,
        opts.dry_run,
        "Suibase install script not found. Next step: run `suiup suibase install --dry-run` to inspect command flow.",
    )?;
    println!("Running {} ...", install_script.display());
    runner.run(
        "bash",
        &[install_script.to_string_lossy().to_string()],
        opts.dry_run,
        "Suibase install failed. Next step: run the install script directly for full output.",
    )
}

fn update_with_runner(
    opts: ActionOptions,
    runner: &mut dyn Runner,
    suibase_dir: &Path,
) -> Result<()> {
    ensure_supported_platform()?;
    runner.ensure_available("bash", opts.dry_run)?;

    if opts.yes {
        println!("Proceeding in non-interactive mode (--yes).");
    }

    let update_script = suibase_dir.join("update");
    ensure_script_exists(
        &update_script,
        opts.dry_run,
        "Suibase update script not found. Next step: run `suiup suibase install` first.",
    )?;
    println!("Running {} ...", update_script.display());
    runner.run(
        "bash",
        &[update_script.to_string_lossy().to_string()],
        opts.dry_run,
        "Suibase update failed. Next step: retry after `suiup suibase doctor` passes.",
    )
}

fn uninstall_with_runner(
    opts: ActionOptions,
    runner: &mut dyn Runner,
    suibase_dir: &Path,
) -> Result<()> {
    ensure_supported_platform()?;
    runner.ensure_available("bash", opts.dry_run)?;

    if !opts.yes && !opts.dry_run && !confirm_uninstall()? {
        println!("Uninstall cancelled by user.");
        return Ok(());
    }

    let uninstall_script = suibase_dir.join("uninstall");
    ensure_script_exists(
        &uninstall_script,
        opts.dry_run,
        "Suibase uninstall script not found. Next step: verify suibase installation path.",
    )?;
    println!("Running {} ...", uninstall_script.display());
    runner.run(
        "bash",
        &[uninstall_script.to_string_lossy().to_string()],
        opts.dry_run,
        "Suibase uninstall failed. Next step: run the uninstall script directly for details.",
    )
}

fn confirm_uninstall() -> Result<bool> {
    print!("This will run suibase uninstall. Continue? [y/N] ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let normalized = input.trim().to_lowercase();
    Ok(matches!(normalized.as_str(), "y" | "yes"))
}

fn report_check(
    ok: bool,
    label: impl AsRef<str>,
    suggestion: impl AsRef<str>,
    failures: &mut usize,
) {
    if ok {
        println!("[OK] {}", label.as_ref());
    } else {
        println!("[FAIL] {}", label.as_ref());
        println!("  -> {}", suggestion.as_ref());
        *failures += 1;
    }
}

fn report_warn(ok: bool, label: impl AsRef<str>, suggestion: impl AsRef<str>) {
    if ok {
        println!("[OK] {}", label.as_ref());
    } else {
        println!("[WARN] {}", label.as_ref());
        println!("  -> {}", suggestion.as_ref());
    }
}

fn ensure_supported_platform() -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        bail!("suibase is supported on Linux/macOS/WSL2. Native Windows is not supported.");
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(())
    }
}

fn command_exists(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

trait Runner {
    fn ensure_available(&mut self, command: &str, dry_run: bool) -> Result<()>;
    fn run(&mut self, program: &str, args: &[String], dry_run: bool, context: &str) -> Result<()>;
}

struct SystemRunner;

impl Runner for SystemRunner {
    fn ensure_available(&mut self, command: &str, dry_run: bool) -> Result<()> {
        if dry_run {
            println!("[dry-run] check command: {} --version", command);
            return Ok(());
        }

        let output = Command::new(command)
            .arg("--version")
            .output()
            .with_context(|| format!("Failed to execute `{command} --version`"))?;

        if !output.status.success() {
            bail!("{command} is not installed or not available in PATH");
        }

        Ok(())
    }

    fn run(&mut self, program: &str, args: &[String], dry_run: bool, context: &str) -> Result<()> {
        if dry_run {
            println!("[dry-run] {}", format_command(program, args));
            return Ok(());
        }

        let status = Command::new(program).args(args).status().with_context(|| {
            format!(
                "Failed to execute command `{}`",
                format_command(program, args)
            )
        })?;

        if !status.success() {
            bail!("{context}");
        }
        Ok(())
    }
}

fn ensure_script_exists(path: &Path, dry_run: bool, err_msg: &str) -> Result<()> {
    if path.exists() {
        return Ok(());
    }

    if dry_run {
        println!("[dry-run] script missing: {}", path.display());
        return Ok(());
    }

    bail!("{err_msg}")
}

fn format_command(program: &str, args: &[String]) -> String {
    if args.is_empty() {
        return program.to_string();
    }
    format!("{} {}", program, args.join(" "))
}

fn suibase_dir() -> Result<PathBuf> {
    let home = home_dir()?;
    let override_dir = env::var_os(SUIBASE_DIR_ENV).map(PathBuf::from);
    Ok(resolve_suibase_dir(home, override_dir))
}

fn resolve_suibase_dir(home: PathBuf, override_dir: Option<PathBuf>) -> PathBuf {
    override_dir.unwrap_or_else(|| home.join("suibase"))
}

fn local_bin_dir() -> Result<PathBuf> {
    Ok(home_dir()?.join(".local").join("bin"))
}

fn home_dir() -> Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| anyhow!("Unable to determine home directory"))
}

fn path_contains(target: &Path) -> bool {
    let path_var = match env::var_os("PATH") {
        Some(value) => value,
        None => return false,
    };

    path_contains_from(path_var.as_os_str(), target)
}

fn path_contains_from(path_var: &OsStr, target: &Path) -> bool {
    env::split_paths(path_var).any(|entry| entry == target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use tempfile::TempDir;

    #[derive(Clone)]
    struct FakeRunner {
        entries: Rc<RefCell<Vec<String>>>,
    }

    impl FakeRunner {
        fn new() -> Self {
            Self {
                entries: Rc::new(RefCell::new(Vec::new())),
            }
        }

        fn entries(&self) -> Vec<String> {
            self.entries.borrow().clone()
        }
    }

    impl Runner for FakeRunner {
        fn ensure_available(&mut self, command: &str, dry_run: bool) -> Result<()> {
            self.entries
                .borrow_mut()
                .push(format!("check:{}:{}", command, dry_run));
            Ok(())
        }

        fn run(
            &mut self,
            program: &str,
            args: &[String],
            dry_run: bool,
            _context: &str,
        ) -> Result<()> {
            self.entries.borrow_mut().push(format!(
                "run:{}:{}:{}",
                program,
                dry_run,
                args.join(" ")
            ));
            Ok(())
        }
    }

    #[test]
    fn test_resolve_suibase_dir_prefers_override() {
        let home = PathBuf::from("/tmp/home");
        let override_dir = Some(PathBuf::from("/opt/suibase-custom"));
        let path = resolve_suibase_dir(home, override_dir);
        assert_eq!(path, PathBuf::from("/opt/suibase-custom"));
    }

    #[test]
    fn test_path_contains_from_split_paths() {
        let target = PathBuf::from("/tmp/a");
        let path_var = env::join_paths([
            PathBuf::from("/tmp/z"),
            target.clone(),
            PathBuf::from("/tmp/b"),
        ])
        .unwrap();
        assert!(path_contains_from(path_var.as_os_str(), &target));
    }

    #[test]
    fn test_install_dry_run_clone_path() {
        let temp = TempDir::new().unwrap();
        let suibase_dir = temp.path().join("suibase");
        let mut runner = FakeRunner::new();

        install_with_runner(
            ActionOptions {
                yes: true,
                dry_run: true,
            },
            &mut runner,
            &suibase_dir,
        )
        .unwrap();

        let entries = runner.entries();
        assert!(entries.iter().any(|line| line == "check:git:true"));
        assert!(entries.iter().any(|line| line == "check:bash:true"));
        assert!(entries.iter().any(|line| {
            line.contains("run:git:true:clone https://github.com/chainmovers/suibase.git")
        }));
        assert!(entries.iter().any(|line| line.contains("run:bash:true:")));
    }

    #[test]
    fn test_install_dry_run_pull_path_when_repo_exists() {
        let temp = TempDir::new().unwrap();
        let suibase_dir = temp.path().join("suibase");
        std::fs::create_dir_all(&suibase_dir).unwrap();

        let mut runner = FakeRunner::new();
        install_with_runner(
            ActionOptions {
                yes: false,
                dry_run: true,
            },
            &mut runner,
            &suibase_dir,
        )
        .unwrap();

        let entries = runner.entries();
        assert!(
            entries
                .iter()
                .any(|line| line.contains("run:git:true:-C") && line.contains("pull --ff-only"))
        );
    }
}
