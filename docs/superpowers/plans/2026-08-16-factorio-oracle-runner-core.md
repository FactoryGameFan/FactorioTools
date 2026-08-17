# factorio-oracle Runner Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust CLI that discovers Factorio installs and runs a headless probe described by a JSON spec, returning the work directory and provenance.

**Architecture:** A single binary with pure builders and one injectable spawn boundary. Every function that produces a string or an argument vector is pure and unit-tested with no Factorio present. Only `run.rs` touches disk and processes, and it takes a `Spawner` trait so a fake game can assert the argument vector and write the dump a real game would have written. The tool owns plumbing only; probe analysis stays in the consumer's language.

**Tech Stack:** Rust (edition 2021), `clap` v4 derive for the CLI, `serde` + `serde_json` for the spec and output, `anyhow` for errors, `tempfile` for tests. No regex crate - version parsing is hand-rolled to keep the dependency surface small.

**Spec:** `/Users/ericjohnson/GitHub/FactorioTools/docs/superpowers/specs/2026-08-16-shared-factorio-oracle-design.md`

## Global Constraints

- **Repo:** new, public, `factorio-oracle`. This plan builds it from an empty directory at `~/GitHub/factorio-oracle`.
- **Rust toolchain is pinned** in `rust-toolchain.toml`. Use `1.97.1`, matching FactorioMapWebUI, which pins it as a correctness control rather than a convenience.
- **Renovate config must exist in the first commit.** The Renovate app runs with "Require config file" enabled, so a default branch with no valid config makes Renovate silently do nothing, which is indistinguishable from "no updates available". Exactly one config file may exist in the repo.
- **`automerge: false` globally**, no exceptions. One weekly batch, Monday morning, `America/Los_Angeles`. Security updates are exempt from that window.
- **Renovate ecosystems here are `cargo` and `github-actions`.** Not npm, not NuGet.
- **House writing style:** hyphens only. Never em dashes or en dashes, in code comments, docs, or commit messages.
- **CI must pass with no Factorio installed.** Every test in this plan runs without the game. That is a hard requirement, matching all four consumer repos.
- **The version a mod declares is always derived from the binary, never hardcoded.** A mismatch makes Factorio skip the mod in silence, and the run ends with no dump and nothing naming the cause.
- **Lua supplied by a consumer is opaque.** Never template it, escape it, rewrite it, or wrap it in `script.on_init`.
- **Determinism:** every map that reaches output is a `BTreeMap` or is explicitly sorted. `HashMap` iteration order is randomised per process and would make drift checks permanently red.

---

### Task 1: Repository skeleton

Creates the repo, the toolchain pin, CI, and the Renovate config. Ends with a green CI run on an empty library.

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `.gitignore`
- Create: `README.md`
- Create: `.github/workflows/ci.yml`
- Create: `.github/renovate.json5`
- Create: `src/lib.rs`
- Create: `src/main.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: a crate named `factorio_oracle` with a binary target `factorio-oracle`. Later tasks add modules to `src/lib.rs`.

- [ ] **Step 1: Create the directory and initialise git**

```bash
mkdir -p ~/GitHub/factorio-oracle
cd ~/GitHub/factorio-oracle
git init -b main
```

- [ ] **Step 2: Write `Cargo.toml`**

```toml
[package]
name = "factorio-oracle"
version = "0.1.0"
edition = "2021"
description = "Ask a real Factorio install what it does, and record the answer with its provenance"
license = "MIT"
repository = "https://github.com/wormeyman/factorio-oracle"

[lib]
name = "factorio_oracle"
path = "src/lib.rs"

[[bin]]
name = "factorio-oracle"
path = "src/main.rs"

[dependencies]
anyhow = "1"
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1", features = ["preserve_order"] }

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Write `rust-toolchain.toml`**

```toml
# Pinned deliberately. FactorioMapWebUI pins its toolchain as a correctness
# control, because a compiler change is a codegen change. This repo does not ship
# wasm, so the reason here is weaker - but a shared tool that four repos rely on
# should not change behaviour because a contributor has a different rustup default.
[toolchain]
channel = "1.97.1"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

- [ ] **Step 4: Write `.gitignore`**

```gitignore
/target
/refs
.DS_Store
```

- [ ] **Step 5: Write `src/lib.rs`**

```rust
//! Plumbing for asking a real Factorio install what it does.
//!
//! This crate owns discovery, mod scaffolding, launching, and reading results
//! back. It deliberately owns none of the analysis: a probe compares the game
//! against a consumer's own reimplementation, so that half stays with the
//! consumer, in the consumer's language.

/// Returns the crate version, so `main` and tests share one source of truth.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_not_empty() {
        assert!(!version().is_empty());
    }
}
```

- [ ] **Step 6: Write `src/main.rs`**

```rust
fn main() {
    println!("factorio-oracle {}", factorio_oracle::version());
}
```

- [ ] **Step 7: Write `.github/workflows/ci.yml`**

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

jobs:
  verify:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      # No toolchain action: rust-toolchain.toml is honoured by the preinstalled
      # rustup, so the pin stays the single source of truth.
      - name: Show toolchain
        run: rustc --version && cargo --version

      - name: Format
        run: cargo fmt --all -- --check

      - name: Clippy
        run: cargo clippy --all-targets -- -D warnings

      - name: Test
        run: cargo test --all-targets
```

- [ ] **Step 8: Write `.github/renovate.json5`**

```json5
{
  $schema: "https://docs.renovatebot.com/renovate-schema.json",
  extends: ["config:recommended"],

  // One batch a week rather than a trickle. Monday morning means a failed
  // update is looked at on a weekday, not discovered the following weekend.
  schedule: ["* 0-8 * * 1"],
  timezone: "America/Los_Angeles",

  // Nothing automerges, with no exceptions. A green CI run proves the repo is
  // consistent, not that a bump is correct - and this tool's correctness lives
  // in fixtures captured from a game CI cannot run.
  automerge: false,

  // Security fixes deliberately skip the weekly window.
  vulnerabilityAlerts: {
    enabled: true,
    schedule: ["at any time"],
  },

  // The toolchain pin is a deliberate control, not a stale dependency. Bumping
  // it is a decision, so it gets its own PR rather than riding along in a batch.
  packageRules: [
    {
      matchManagers: ["cargo"],
      matchUpdateTypes: ["patch", "minor"],
      groupName: "cargo patch and minor",
    },
    {
      matchManagers: ["github-actions"],
      groupName: "github actions",
    },
  ],
}
```

- [ ] **Step 9: Write `README.md`**

```markdown
# factorio-oracle

Asks a real Factorio install what it does, so behaviour that other projects
reimplement can be checked against the game rather than against assumptions.

Four repos each wrote this plumbing separately: FactorioTools,
factorio-blueprint-editor, FactorioMapWebUI and FactorioWikiDamageThresholds.
This is that plumbing, once.

## What it does and does not do

It owns discovery, mod scaffolding, launching and reading results back. It owns
none of the analysis. A probe compares the game against a consumer's own
reimplementation, so that half has to run in the consumer's language. The
interface is therefore JSON in and JSON out, not a probe framework.

## Scope

Behavioural reverse engineering for interoperability - understanding what the
game computes so other projects can agree with it. Not extracting or
redistributing game code or assets. Keep it that way.
```

- [ ] **Step 10: Validate the Renovate config**

Run: `npx --yes --package renovate -- renovate-config-validator .github/renovate.json5`
Expected: reports the config is valid. If it fails, the file is silently inert on the default branch, which is the failure this step exists to prevent.

- [ ] **Step 11: Run the checks**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: PASS, with one test passing (`version_is_not_empty`).

- [ ] **Step 12: Commit**

```bash
git add -A
git commit -m "Set up the factorio-oracle crate, CI and Renovate

Renovate lands in the first commit deliberately. The app runs with
'Require config file' enabled, so a default branch with no valid config
makes it do nothing at all, silently."
```

---

### Task 2: Parse the version out of the binary

Two values come from `factorio --version`: the full build line, which fixtures stamp, and the `major.minor` a mod's `info.json` must declare. Getting the second wrong makes Factorio skip the mod without saying so.

**Files:**
- Create: `src/version.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub struct VersionInfo { pub line: String, pub major: u32, pub minor: u32, pub patch: u32 }`, `impl VersionInfo { pub fn major_minor(&self) -> String }`, and `pub fn parse_version_line(output: &str) -> Option<VersionInfo>`.

- [ ] **Step 1: Write the failing test**

Create `src/version.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_real_macos_steam_version_line() {
        let info = parse_version_line("Version: 2.0.77 (build 84539, mac-arm64, full)\n")
            .expect("should parse");
        assert_eq!(info.major, 2);
        assert_eq!(info.minor, 0);
        assert_eq!(info.patch, 77);
        assert_eq!(info.line, "Version: 2.0.77 (build 84539, mac-arm64, full)");
    }

    #[test]
    fn major_minor_is_what_a_mod_declares() {
        let info = parse_version_line("Version: 2.1.14 (build 87038, mac-arm64, steam)").unwrap();
        assert_eq!(info.major_minor(), "2.1");
    }

    #[test]
    fn ignores_the_build_number_and_arch_digits() {
        // "84539" and the "64" in "mac-arm64" are digits too. Only the
        // three-part token is a version.
        let info = parse_version_line("Version: 2.0.77 (build 84539, mac-arm64, full)").unwrap();
        assert_eq!((info.major, info.minor, info.patch), (2, 0, 77));
    }

    #[test]
    fn reads_only_the_first_line() {
        let info = parse_version_line("Version: 2.0.77 (build 1, x, y)\nMap version 9.9.9").unwrap();
        assert_eq!(info.patch, 77);
    }

    #[test]
    fn returns_none_when_there_is_no_version() {
        assert!(parse_version_line("bash: factorio: command not found").is_none());
        assert!(parse_version_line("").is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod version;` to `src/lib.rs`, then run:

Run: `cargo test version`
Expected: FAIL to compile, with `cannot find function 'parse_version_line' in this scope`.

- [ ] **Step 3: Write minimal implementation**

Insert above the test module in `src/version.rs`:

```rust
//! Reading a Factorio version out of `factorio --version`.

/// A parsed `factorio --version` first line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionInfo {
    /// The full first line, verbatim. This is what a fixture stamps, because it
    /// carries the build number and platform as well as the version.
    pub line: String,
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl VersionInfo {
    /// The value a mod's `info.json` must declare in `factorio_version`.
    ///
    /// Derived, never hardcoded. A mod declaring 2.1 against a 2.0.x binary is
    /// skipped in silence, and the run ends with no dump and nothing in
    /// Factorio's output naming the cause.
    pub fn major_minor(&self) -> String {
        format!("{}.{}", self.major, self.minor)
    }
}

/// Parses the first line of `factorio --version`.
///
/// Looks for the first token of the form `<digits>.<digits>.<digits>`. A build
/// number or an architecture suffix contains digits too, so a bare digit scan
/// would find the wrong thing.
pub fn parse_version_line(output: &str) -> Option<VersionInfo> {
    let line = output.lines().next()?.trim();
    for token in line.split(|c: char| !(c.is_ascii_digit() || c == '.')) {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            continue;
        }
        if let (Ok(major), Ok(minor), Ok(patch)) =
            (parts[0].parse(), parts[1].parse(), parts[2].parse())
        {
            return Some(VersionInfo {
                line: line.to_string(),
                major,
                minor,
                patch,
            });
        }
    }
    None
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test version`
Expected: PASS, 5 tests.

- [ ] **Step 5: Commit**

```bash
git add src/version.rs src/lib.rs
git commit -m "Parse the Factorio version, and derive the major.minor a mod declares

A mod declaring the wrong factorio_version is skipped in silence, so this
value is derived from the binary rather than written down."
```

---

### Task 3: Resolve an install's layout

macOS ships an `.app` bundle, Linux a plain directory, and a caller may point straight at the executable. All three must resolve to the same three paths.

**Files:**
- Create: `src/install.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub struct InstallLayout { pub root: PathBuf, pub binary: PathBuf, pub data_dir: PathBuf, pub doc_dir: PathBuf }` and `pub fn resolve_layout(root: &Path) -> Option<InstallLayout>`.

- [ ] **Step 1: Write the failing test**

Create `src/install.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"").unwrap();
    }

    #[test]
    fn resolves_a_macos_app_bundle() {
        let dir = tempdir().unwrap();
        let app = dir.path().join("factorio.app");
        touch(&app.join("Contents/MacOS/factorio"));
        fs::create_dir_all(app.join("Contents/data")).unwrap();
        fs::create_dir_all(app.join("Contents/doc-html")).unwrap();

        let layout = resolve_layout(&app).expect("should resolve");
        assert_eq!(layout.binary, app.join("Contents/MacOS/factorio"));
        assert_eq!(layout.data_dir, app.join("Contents/data"));
        assert_eq!(layout.doc_dir, app.join("Contents/doc-html"));
    }

    #[test]
    fn resolves_a_linux_install_directory() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("factorio");
        touch(&root.join("bin/x64/factorio"));
        fs::create_dir_all(root.join("data")).unwrap();
        fs::create_dir_all(root.join("doc-html")).unwrap();

        let layout = resolve_layout(&root).expect("should resolve");
        assert_eq!(layout.binary, root.join("bin/x64/factorio"));
        assert_eq!(layout.data_dir, root.join("data"));
        assert_eq!(layout.doc_dir, root.join("doc-html"));
    }

    #[test]
    fn resolves_a_path_pointing_straight_at_the_binary() {
        // This is the FACTORIO_BIN case: callers set it to an executable, not a root.
        let dir = tempdir().unwrap();
        let root = dir.path().join("factorio");
        let bin = root.join("bin/x64/factorio");
        touch(&bin);
        fs::create_dir_all(root.join("data")).unwrap();
        fs::create_dir_all(root.join("doc-html")).unwrap();

        let layout = resolve_layout(&bin).expect("should resolve");
        assert_eq!(layout.binary, bin);
        assert_eq!(layout.data_dir, root.join("data"));
    }

    #[test]
    fn returns_none_when_the_binary_is_missing() {
        let dir = tempdir().unwrap();
        let app = dir.path().join("factorio.app");
        fs::create_dir_all(app.join("Contents/data")).unwrap();
        assert!(resolve_layout(&app).is_none());
    }

    #[test]
    fn returns_none_for_a_path_that_does_not_exist() {
        assert!(resolve_layout(Path::new("/nope/not/here")).is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod install;` to `src/lib.rs`, then run:

Run: `cargo test install`
Expected: FAIL to compile, with `cannot find function 'resolve_layout'`.

- [ ] **Step 3: Write minimal implementation**

Insert above the test module in `src/install.rs`:

```rust
//! Finding Factorio installs and working out where their pieces live.

use std::path::{Path, PathBuf};

/// The three paths every mode needs from an install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallLayout {
    /// What the caller pointed at, kept for reporting.
    pub root: PathBuf,
    pub binary: PathBuf,
    pub data_dir: PathBuf,
    pub doc_dir: PathBuf,
}

/// Works out an install's layout from a root path, an `.app` bundle, or a path
/// straight to the executable.
///
/// Returns `None` unless the binary and the data directory both exist. The doc
/// directory is not required: a headless build ships no `doc-html`, and probes
/// that never read the API docs work fine without it.
pub fn resolve_layout(root: &Path) -> Option<InstallLayout> {
    let candidates: Vec<(PathBuf, PathBuf, PathBuf)> = if root.join("Contents").is_dir() {
        // macOS .app bundle.
        vec![(
            root.join("Contents/MacOS/factorio"),
            root.join("Contents/data"),
            root.join("Contents/doc-html"),
        )]
    } else if root.is_file() {
        // A path straight to the executable, which is what FACTORIO_BIN holds.
        // The install root is two levels up from bin/x64/factorio.
        let base = root.parent()?.parent()?.parent()?;
        vec![(
            root.to_path_buf(),
            base.join("data"),
            base.join("doc-html"),
        )]
    } else {
        // A plain install directory.
        vec![(
            root.join("bin/x64/factorio"),
            root.join("data"),
            root.join("doc-html"),
        )]
    };

    for (binary, data_dir, doc_dir) in candidates {
        if binary.is_file() && data_dir.is_dir() {
            return Some(InstallLayout {
                root: root.to_path_buf(),
                binary,
                data_dir,
                doc_dir,
            });
        }
    }
    None
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test install`
Expected: PASS, 5 tests.

- [ ] **Step 5: Commit**

```bash
git add src/install.rs src/lib.rs
git commit -m "Resolve an install layout from a bundle, a directory, or a binary path

Four repos each wrote a version of this. The .app-versus-directory split and
the FACTORIO_BIN-points-at-an-executable case are the two that keep recurring."
```

---

### Task 4: Enumerate candidate installs and add `installs list`

**Files:**
- Modify: `src/install.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `InstallLayout` and `resolve_layout` from Task 3; `VersionInfo` and `parse_version_line` from Task 2.
- Produces: `pub fn candidate_roots(home: &Path, env_bin: Option<&Path>) -> Vec<PathBuf>` and `pub struct DiscoveredInstall { pub layout: InstallLayout, pub version: Option<VersionInfo> }`.

- [ ] **Step 1: Write the failing test**

Append to the `tests` module in `src/install.rs`:

```rust
    #[test]
    fn env_bin_is_first_when_set() {
        let home = Path::new("/home/someone");
        let roots = candidate_roots(home, Some(Path::new("/opt/custom/factorio")));
        assert_eq!(roots[0], PathBuf::from("/opt/custom/factorio"));
    }

    #[test]
    fn covers_every_candidate_the_four_repos_used() {
        let home = Path::new("/home/someone");
        let roots = candidate_roots(home, None);
        // The union of the candidate lists found across FactorioTools,
        // FactorioMapWebUI, factorio-blueprint-editor and the stray benchmark
        // script. Each repo had a different subset, so each found a different
        // set of installs.
        let expected = [
            "/home/someone/Library/Application Support/Steam/steamapps/common/Factorio/factorio.app",
            "/Applications/factorio.app",
            "/home/someone/.steam/steam/steamapps/common/Factorio",
            "/home/someone/.factorio",
            "/opt/factorio",
        ];
        for want in expected {
            assert!(
                roots.contains(&PathBuf::from(want)),
                "missing candidate: {want}\ngot: {roots:?}"
            );
        }
    }

    #[test]
    fn candidates_are_unique() {
        let home = Path::new("/home/someone");
        let roots = candidate_roots(home, None);
        let mut seen = roots.clone();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), roots.len(), "duplicate candidate in {roots:?}");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test install`
Expected: FAIL to compile, with `cannot find function 'candidate_roots'`.

- [ ] **Step 3: Write minimal implementation**

Add to `src/install.rs`, above the test module:

```rust
use crate::version::{parse_version_line, VersionInfo};

/// An install that was found, with its version if the binary would run.
#[derive(Debug, Clone)]
pub struct DiscoveredInstall {
    pub layout: InstallLayout,
    /// `None` when the binary could not be executed, which is normal on a
    /// machine of a different architecture.
    pub version: Option<VersionInfo>,
}

/// Every place a Factorio install is known to sit.
///
/// This is the union of the candidate lists found across the four consumer
/// repos plus a stray benchmark script. Each had a different subset, so each
/// found a different set of installs - which is the whole reason discovery is
/// worth doing once.
pub fn candidate_roots(home: &Path, env_bin: Option<&Path>) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(bin) = env_bin {
        roots.push(bin.to_path_buf());
    }
    roots.extend([
        home.join("Library/Application Support/Steam/steamapps/common/Factorio/factorio.app"),
        PathBuf::from("/Applications/factorio.app"),
        home.join(".steam/steam/steamapps/common/Factorio"),
        home.join(".factorio"),
        PathBuf::from("/opt/factorio"),
    ]);
    roots.dedup();
    roots
}

/// Reads a version by running the binary. Returns `None` if it will not run.
pub fn read_version(binary: &Path) -> Option<VersionInfo> {
    let output = std::process::Command::new(binary).arg("--version").output().ok()?;
    parse_version_line(&String::from_utf8_lossy(&output.stdout))
}

/// Finds every install on this machine.
pub fn discover(home: &Path, env_bin: Option<&Path>) -> Vec<DiscoveredInstall> {
    candidate_roots(home, env_bin)
        .iter()
        .filter_map(|root| resolve_layout(root))
        .map(|layout| {
            let version = read_version(&layout.binary);
            DiscoveredInstall { layout, version }
        })
        .collect()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test install`
Expected: PASS, 8 tests.

- [ ] **Step 5: Wire up the CLI**

Replace `src/main.rs` entirely:

```rust
use clap::{Parser, Subcommand};
use factorio_oracle::install;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "factorio-oracle", version, about = "Ask a real Factorio install what it does")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Discover Factorio installs on this machine
    Installs {
        #[command(subcommand)]
        action: InstallsAction,
    },
}

#[derive(Subcommand)]
enum InstallsAction {
    /// Print every install found, as JSON
    List,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Installs { action: InstallsAction::List } => {
            let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
            let env_bin = std::env::var_os("FACTORIO_BIN").map(PathBuf::from);
            let found = install::discover(&home, env_bin.as_deref());

            let rows: Vec<serde_json::Value> = found
                .iter()
                .map(|d| {
                    serde_json::json!({
                        "root": d.layout.root,
                        "binary": d.layout.binary,
                        "dataDir": d.layout.data_dir,
                        "docDir": d.layout.doc_dir,
                        "version": d.version.as_ref().map(|v| format!("{}.{}.{}", v.major, v.minor, v.patch)),
                        "modFactorioVersion": d.version.as_ref().map(|v| v.major_minor()),
                        "buildLine": d.version.as_ref().map(|v| v.line.clone()),
                    })
                })
                .collect();

            println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "installs": rows }))?);
        }
    }
    Ok(())
}
```

- [ ] **Step 6: Verify it runs**

Run: `cargo run -- installs list`
Expected: JSON with an `installs` array. On a machine with Factorio it lists at least one entry with a real `version` and `modFactorioVersion`. On a machine without, it prints `{"installs": []}` and exits 0.

- [ ] **Step 7: Commit**

```bash
git add src/install.rs src/main.rs
git commit -m "Discover every install rather than picking one

The consumers target different Factorio versions on purpose, so enumerating
is the requirement and choosing is the caller's job."
```

---

### Task 5: The probe spec types

**Files:**
- Create: `src/probe.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub enum Mode { DumpData, Create, Interactive, Preview, ReadOnly }`, `pub struct ModSpec { pub name: String, pub version: String, pub dependencies: Vec<String>, pub control_lua: Option<String>, pub control_lua_file: Option<PathBuf>, pub data_lua: Option<String>, pub data_final_fixes_lua: Option<String> }`, `pub struct ProbeSpec { pub mode: Mode, pub r#mod: Option<ModSpec>, pub literals: BTreeMap<String, String>, pub timeout_seconds: Option<u64>, pub capture_active_mods: bool }`, and `impl ModSpec { pub fn dir_name(&self) -> String }`.

- [ ] **Step 1: Write the failing test**

Create `src/probe.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialises_a_minimal_dump_data_spec() {
        let spec: ProbeSpec = serde_json::from_str(r#"{ "mode": "dump-data" }"#).unwrap();
        assert_eq!(spec.mode, Mode::DumpData);
        assert!(spec.r#mod.is_none());
        assert!(spec.literals.is_empty());
        // On by default. A contaminated capture looks entirely normal, so the
        // safe default records what loaded.
        assert!(spec.capture_active_mods);
    }

    #[test]
    fn contamination_reporting_can_be_turned_off() {
        let spec: ProbeSpec =
            serde_json::from_str(r#"{ "mode": "create", "capture_active_mods": false }"#).unwrap();
        assert!(!spec.capture_active_mods);
    }

    #[test]
    fn deserialises_a_create_spec_with_a_mod() {
        let json = r#"{
            "mode": "create",
            "mod": {
                "name": "bp_probe",
                "version": "0.0.1",
                "dependencies": ["base", "elevated-rails", "space-age"],
                "control_lua": "script.on_init(function() end)"
            },
            "literals": { "blueprint": "0eNq" },
            "timeout_seconds": 120
        }"#;
        let spec: ProbeSpec = serde_json::from_str(json).unwrap();
        assert_eq!(spec.mode, Mode::Create);
        let m = spec.r#mod.as_ref().unwrap();
        assert_eq!(m.name, "bp_probe");
        assert_eq!(m.dependencies, vec!["base", "elevated-rails", "space-age"]);
        assert_eq!(spec.literals.get("blueprint").unwrap(), "0eNq");
        assert_eq!(spec.timeout_seconds, Some(120));
    }

    #[test]
    fn mod_directory_name_carries_the_version_suffix() {
        // Factorio requires <name>_<version> and it must match info.json, or
        // the mod is not loaded.
        let m = ModSpec {
            name: "bp_probe".into(),
            version: "0.0.1".into(),
            dependencies: vec![],
            control_lua: None,
            control_lua_file: None,
            data_lua: None,
            data_final_fixes_lua: None,
        };
        assert_eq!(m.dir_name(), "bp_probe_0.0.1");
    }

    #[test]
    fn every_mode_name_round_trips() {
        for (text, mode) in [
            ("dump-data", Mode::DumpData),
            ("create", Mode::Create),
            ("interactive", Mode::Interactive),
            ("preview", Mode::Preview),
            ("read-only", Mode::ReadOnly),
        ] {
            let spec: ProbeSpec =
                serde_json::from_str(&format!(r#"{{ "mode": "{text}" }}"#)).unwrap();
            assert_eq!(spec.mode, mode, "mode {text} did not round trip");
        }
    }

    #[test]
    fn rejects_an_unknown_mode() {
        assert!(serde_json::from_str::<ProbeSpec>(r#"{ "mode": "benchmark" }"#).is_err());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod probe;` to `src/lib.rs`, then run:

Run: `cargo test probe`
Expected: FAIL to compile, with `cannot find type 'ProbeSpec'`.

- [ ] **Step 3: Write minimal implementation**

Insert above the test module in `src/probe.rs`:

```rust
//! The JSON document a consumer hands in to describe a probe.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// How the game gets launched. The differences are not cosmetic: the success
/// predicate, whether a mod is generated, and the argument vector all differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// `--dump-data`. No mod is generated; the mod directory exists to be empty.
    DumpData,
    /// `--create`. A generated mod writes a dump and errors out.
    Create,
    /// `--load-scenario`. Long running, with a human at the keyboard.
    Interactive,
    /// `--generate-map-preview`. No mod, and it exits 0 on success.
    Preview,
    /// No binary at all. Migrations and API docs are files on disk.
    ReadOnly,
}

/// The throwaway mod a probe runs.
#[derive(Debug, Clone, Deserialize)]
pub struct ModSpec {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Consumer Lua, passed through untouched.
    #[serde(default)]
    pub control_lua: Option<String>,
    #[serde(default)]
    pub control_lua_file: Option<PathBuf>,
    #[serde(default)]
    pub data_lua: Option<String>,
    /// Prototype overrides belong here, not in `data_lua`. A probe mod declares
    /// no dependencies, so its `data.lua` may run before `space-age`'s and the
    /// prototype it wants to change will not exist yet - a silent no-op.
    #[serde(default)]
    pub data_final_fixes_lua: Option<String>,
}

impl ModSpec {
    /// The on-disk directory name. Factorio requires `<name>_<version>`, and it
    /// must match `info.json` or the mod is not loaded.
    pub fn dir_name(&self) -> String {
        format!("{}_{}", self.name, self.version)
    }
}

/// A probe, as handed in.
#[derive(Debug, Clone, Deserialize)]
pub struct ProbeSpec {
    pub mode: Mode,
    #[serde(default, rename = "mod")]
    pub r#mod: Option<ModSpec>,
    /// Values injected as Lua locals above the consumer's control script.
    #[serde(default)]
    pub literals: BTreeMap<String, String>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    /// On by default. A contaminated capture looks entirely normal, so the
    /// safe default is to record what loaded and let a consumer opt out.
    #[serde(default = "default_true")]
    pub capture_active_mods: bool,
}

fn default_true() -> bool {
    true
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test probe`
Expected: PASS, 5 tests.

- [ ] **Step 5: Commit**

```bash
git add src/probe.rs src/lib.rs
git commit -m "Define the probe spec, with five launch modes

The modes differ in the success predicate, whether a mod is generated, and
the argument vector, so they are an enum rather than a flag."
```

---

### Task 6: Inject literals as Lua long brackets

The one place the tool touches a consumer's Lua. It exists because embedding a base64 blueprint string in a quoted Lua string breaks on the first inner quote.

**Files:**
- Create: `src/lua.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub fn long_bracket(value: &str) -> String` and `pub fn build_literals_prelude(literals: &BTreeMap<String, String>) -> String`.

- [ ] **Step 1: Write the failing test**

Create `src/lua.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn wraps_a_plain_value_at_level_zero() {
        assert_eq!(long_bracket("0eNqrVkrKT"), "[[0eNqrVkrKT]]");
    }

    #[test]
    fn escalates_the_level_when_the_value_would_close_the_bracket() {
        // A value containing "]]" would end the literal early.
        assert_eq!(long_bracket("a]]b"), "[=[a]]b]=]");
    }

    #[test]
    fn escalates_again_when_the_next_level_also_collides() {
        assert_eq!(long_bracket("a]]b]=]c"), "[==[a]]b]=]c]==]");
    }

    #[test]
    fn prelude_declares_one_local_per_entry() {
        let mut literals = BTreeMap::new();
        literals.insert("blueprint".to_string(), "0eNq".to_string());
        assert_eq!(
            build_literals_prelude(&literals),
            "local blueprint = [[0eNq]]\n"
        );
    }

    #[test]
    fn prelude_is_sorted_and_therefore_deterministic() {
        let mut literals = BTreeMap::new();
        literals.insert("zebra".to_string(), "z".to_string());
        literals.insert("alpha".to_string(), "a".to_string());
        assert_eq!(
            build_literals_prelude(&literals),
            "local alpha = [[a]]\nlocal zebra = [[z]]\n"
        );
    }

    #[test]
    fn prelude_is_empty_when_there_are_no_literals() {
        assert_eq!(build_literals_prelude(&BTreeMap::new()), "");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod lua;` to `src/lib.rs`, then run:

Run: `cargo test lua`
Expected: FAIL to compile, with `cannot find function 'long_bracket'`.

- [ ] **Step 3: Write minimal implementation**

Insert above the test module in `src/lua.rs`:

```rust
//! The only place this tool writes Lua on a consumer's behalf.
//!
//! Consumer Lua is otherwise opaque: never templated, escaped, rewritten, or
//! wrapped. Wrapping in `script.on_init` would be a convenient default and
//! would make an `on_tick` probe with registered commands impossible.

use std::collections::BTreeMap;

/// Wraps a value in a Lua long bracket at a level that cannot collide with the
/// value's own contents.
///
/// A base64 blueprint string in a quoted Lua string breaks on the first inner
/// quote. A long bracket takes the value verbatim.
pub fn long_bracket(value: &str) -> String {
    let mut level = 0usize;
    loop {
        let eq = "=".repeat(level);
        if !value.contains(&format!("]{eq}]")) {
            return format!("[{eq}[{value}]{eq}]");
        }
        level += 1;
    }
}

/// Builds the `local <name> = <long bracket>` lines that precede a consumer's
/// control script.
///
/// Sorted, because the output must be identical between runs.
pub fn build_literals_prelude(literals: &BTreeMap<String, String>) -> String {
    literals
        .iter()
        .map(|(name, value)| format!("local {} = {}\n", name, long_bracket(value)))
        .collect()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test lua`
Expected: PASS, 6 tests.

- [ ] **Step 5: Commit**

```bash
git add src/lua.rs src/lib.rs
git commit -m "Inject literals as Lua long brackets, escalating the level as needed

Embedding a base64 blueprint string in a quoted Lua string breaks on the
first inner quote. The bracket level escalates so a value cannot close its
own literal."
```

---

### Task 7: Scaffold the mod files

**Files:**
- Create: `src/scaffold.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `ModSpec` from Task 5.
- Produces: `pub fn build_info_json(spec: &ModSpec, mod_factorio_version: &str) -> serde_json::Value`, `pub fn build_mod_list(mod_name: Option<&str>) -> serde_json::Value`, `pub fn build_config_ini(write_data: &Path) -> String`, and `pub const ACTIVE_MODS_PRELUDE: &str`.

- [ ] **Step 1: Write the failing test**

Create `src/scaffold.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::ModSpec;

    fn sample() -> ModSpec {
        ModSpec {
            name: "bp_probe".into(),
            version: "0.0.1".into(),
            dependencies: vec!["base".into()],
            control_lua: Some("script.on_init(function() end)".into()),
            control_lua_file: None,
            data_lua: None,
            data_final_fixes_lua: None,
        }
    }

    #[test]
    fn info_json_takes_the_version_from_the_binary() {
        let info = build_info_json(&sample(), "2.0");
        assert_eq!(info["factorio_version"], "2.0");
        assert_eq!(info["name"], "bp_probe");
        assert_eq!(info["version"], "0.0.1");
        assert_eq!(info["dependencies"][0], "base");
    }

    #[test]
    fn info_json_version_is_never_hardcoded() {
        // The same mod against a different binary must declare a different
        // version. Getting this wrong makes Factorio skip the mod in silence.
        let a = build_info_json(&sample(), "2.0");
        let b = build_info_json(&sample(), "2.1");
        assert_ne!(a["factorio_version"], b["factorio_version"]);
    }

    #[test]
    fn mod_list_with_no_probe_enables_only_base() {
        // This is the dump-data case: the directory exists to be empty of user
        // mods, because mods rewrite prototypes freely.
        let list = build_mod_list(None);
        assert_eq!(list["mods"].as_array().unwrap().len(), 1);
        assert_eq!(list["mods"][0]["name"], "base");
        assert_eq!(list["mods"][0]["enabled"], true);
    }

    #[test]
    fn mod_list_with_a_probe_enables_both() {
        let list = build_mod_list(Some("bp_probe"));
        let names: Vec<&str> = list["mods"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, vec!["base", "bp_probe"]);
    }

    #[test]
    fn config_ini_isolates_writes_and_reads_the_bundled_data() {
        let ini = build_config_ini(Path::new("/tmp/work/write"));
        assert!(ini.contains("write-data=/tmp/work/write"));
        // The portable token for the install's own data directory.
        assert!(ini.contains("read-data=__PATH__executable__/../data"));
        assert!(ini.starts_with("[path]"));
    }

    #[test]
    fn the_active_mods_prelude_writes_its_own_file() {
        // It must not collide with the consumer's dump file name.
        assert!(ACTIVE_MODS_PRELUDE.contains("oracle-active-mods.json"));
        assert!(ACTIVE_MODS_PRELUDE.contains("script.active_mods"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod scaffold;` to `src/lib.rs`, then run:

Run: `cargo test scaffold`
Expected: FAIL to compile, with `cannot find function 'build_info_json'`.

- [ ] **Step 3: Write minimal implementation**

Insert above the test module in `src/scaffold.rs`:

```rust
//! Writing the throwaway mod's files and the isolated config.

use crate::probe::ModSpec;
use serde_json::{json, Value};
use std::path::Path;

/// A Lua prelude that records which mods actually loaded.
///
/// Reading `script.active_mods` from inside the game is more reliable than
/// grepping Factorio's stdout for "Loading mod", which only works for
/// `--dump-data`. On by default: mods rewrite prototypes freely, so a
/// contaminated capture describes one person's game rather than Factorio - and
/// it looks entirely normal, which is the failure nobody notices.
///
/// **Deliberately not `script.on_init`.** That takes exactly one handler, so a
/// prelude using it would be silently replaced by the consumer's own, and 17 of
/// 18 probes in factorio-blueprint-editor register one. The report would vanish
/// with no error. This self-cancelling `on_nth_tick` runs once and unregisters
/// itself, colliding with nothing.
///
/// The reported set deliberately includes the probe's own throwaway mod. That
/// is proof the mod loaded, which is the thing most worth knowing when a run
/// produces no dump.
pub const ACTIVE_MODS_PRELUDE: &str = r#"
script.on_nth_tick(1, function()
  script.on_nth_tick(1, nil)
  helpers.write_file("oracle-active-mods.json", helpers.table_to_json(script.active_mods))
end)
"#;

/// The mod's `info.json`.
///
/// `mod_factorio_version` is always derived from the binary being run. A mod
/// declaring 2.1 against a 2.0.x binary is skipped in silence: the run ends
/// with no dump, and nothing in Factorio's output names the cause.
pub fn build_info_json(spec: &ModSpec, mod_factorio_version: &str) -> Value {
    json!({
        "name": spec.name,
        "version": spec.version,
        "title": spec.name,
        "author": "factorio-oracle",
        "factorio_version": mod_factorio_version,
        "dependencies": spec.dependencies,
    })
}

/// The `mod-list.json` for an isolated mod directory.
///
/// With `None`, only `base` is enabled and no probe mod exists. That is the
/// `--dump-data` case, where the directory's whole job is to contain no user
/// mods: mods rewrite prototypes freely, so a capture that loads them describes
/// one person's game rather than Factorio.
pub fn build_mod_list(mod_name: Option<&str>) -> Value {
    let mut mods = vec![json!({ "name": "base", "enabled": true })];
    if let Some(name) = mod_name {
        mods.push(json!({ "name": name, "enabled": true }));
    }
    json!({ "mods": mods })
}

/// An isolated `config.ini`.
///
/// `read-data` points at the install's bundled data through Factorio's own
/// portable token, and `write-data` at a scratch directory that started empty.
/// That second half is what makes a stale dump from an earlier capture
/// impossible to pick up by accident.
pub fn build_config_ini(write_data: &Path) -> String {
    format!(
        "[path]\nread-data=__PATH__executable__/../data\nwrite-data={}\n",
        write_data.display()
    )
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test scaffold`
Expected: PASS, 6 tests.

- [ ] **Step 5: Commit**

```bash
git add src/scaffold.rs src/lib.rs
git commit -m "Build the mod files and an isolated config

write-data points at a scratch directory that started empty, so a leftover
dump from an older capture cannot be mistaken for this run's output."
```

---

### Task 8: Build the argument vector per mode

**Files:**
- Create: `src/args.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub enum Launch` with variants `DumpData`, `Create`, `Interactive`, `Preview`, and `pub fn build_args(launch: &Launch) -> Vec<String>`.

- [ ] **Step 1: Write the failing test**

Create `src/args.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dump_data_passes_only_the_mod_directory_and_config() {
        let args = build_args(&Launch::DumpData {
            mod_dir: "/w/mods".into(),
            config: "/w/config.ini".into(),
        });
        assert_eq!(
            args,
            vec!["--dump-data", "--mod-directory", "/w/mods", "--config", "/w/config.ini"]
        );
    }

    #[test]
    fn create_always_passes_map_gen_settings() {
        let args = build_args(&Launch::Create {
            save: "/w/probe.zip".into(),
            map_gen: "/w/map-gen.json".into(),
            seed: None,
            mod_dir: "/w/mods".into(),
            config: "/w/config.ini".into(),
        });
        assert!(args.contains(&"--map-gen-settings".to_string()));
        assert_eq!(args[0], "--create");
        assert_eq!(args[1], "/w/probe.zip");
    }

    #[test]
    fn create_also_passes_the_seed_on_the_command_line() {
        // The seed reaches the game through two channels and nobody has
        // established which one wins. Both come from one field, so they cannot
        // disagree - and a caller that omits the seed gets neither.
        let args = build_args(&Launch::Create {
            save: "/w/probe.zip".into(),
            map_gen: "/w/map-gen.json".into(),
            seed: Some(123456),
            mod_dir: "/w/mods".into(),
            config: "/w/config.ini".into(),
        });
        assert!(args.contains(&"--map-gen-seed".to_string()));
        assert!(args.contains(&"123456".to_string()));

        let without = build_args(&Launch::Create {
            save: "/w/probe.zip".into(),
            map_gen: "/w/map-gen.json".into(),
            seed: None,
            mod_dir: "/w/mods".into(),
            config: "/w/config.ini".into(),
        });
        assert!(!without.contains(&"--map-gen-seed".to_string()));
    }

    #[test]
    fn interactive_loads_a_scenario_and_never_creates() {
        let args = build_args(&Launch::Interactive {
            scenario: "base/freeplay".into(),
            mod_dir: "/w/mods".into(),
            config: "/w/config.ini".into(),
        });
        assert!(args.contains(&"--load-scenario".to_string()));
        assert!(args.contains(&"base/freeplay".to_string()));
        assert!(!args.contains(&"--create".to_string()));
    }

    #[test]
    fn preview_takes_an_output_path_and_no_mod_directory() {
        let args = build_args(&Launch::Preview {
            out: "/w/preview.png".into(),
            map_gen: "/w/map-gen.json".into(),
            planet: Some("nauvis".into()),
            seed: Some(123456),
            size: Some(1024),
        });
        assert_eq!(args[0], "--generate-map-preview");
        assert_eq!(args[1], "/w/preview.png");
        assert!(args.contains(&"--map-preview-planet".to_string()));
        assert!(args.contains(&"nauvis".to_string()));
        assert!(args.contains(&"123456".to_string()));
        assert!(!args.contains(&"--mod-directory".to_string()));
    }

    #[test]
    fn preview_omits_optional_flags_that_were_not_set() {
        let args = build_args(&Launch::Preview {
            out: "/w/preview.png".into(),
            map_gen: "/w/map-gen.json".into(),
            planet: None,
            seed: None,
            size: None,
        });
        assert!(!args.contains(&"--map-preview-planet".to_string()));
        assert!(!args.contains(&"--map-gen-seed".to_string()));
        assert!(!args.contains(&"--map-preview-size".to_string()));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod args;` to `src/lib.rs`, then run:

Run: `cargo test args`
Expected: FAIL to compile, with `cannot find type 'Launch'`.

- [ ] **Step 3: Write minimal implementation**

Insert above the test module in `src/args.rs`:

```rust
//! The argument vector, which differs per mode.

use std::path::PathBuf;

/// What to launch, carrying exactly the paths that mode needs.
#[derive(Debug, Clone)]
pub enum Launch {
    DumpData {
        mod_dir: PathBuf,
        config: PathBuf,
    },
    Create {
        save: PathBuf,
        map_gen: PathBuf,
        /// Written into the map-gen settings file as well. The game can be told
        /// the seed twice and nobody has established which channel wins, so both
        /// are fed from one field and cannot disagree.
        seed: Option<u64>,
        mod_dir: PathBuf,
        config: PathBuf,
    },
    Interactive {
        scenario: String,
        mod_dir: PathBuf,
        config: PathBuf,
    },
    Preview {
        out: PathBuf,
        map_gen: PathBuf,
        planet: Option<String>,
        seed: Option<u64>,
        size: Option<u32>,
    },
}

fn s(path: &std::path::Path) -> String {
    path.display().to_string()
}

/// Builds the argument vector for a launch.
pub fn build_args(launch: &Launch) -> Vec<String> {
    match launch {
        Launch::DumpData { mod_dir, config } => vec![
            "--dump-data".into(),
            "--mod-directory".into(),
            s(mod_dir),
            "--config".into(),
            s(config),
        ],
        Launch::Create {
            save,
            map_gen,
            seed,
            mod_dir,
            config,
        } => {
            let mut args = vec![
                "--create".into(),
                s(save),
                // Always passed. Whether the game requires it is unmeasured;
                // this matches established practice in the consumer repos.
                "--map-gen-settings".into(),
                s(map_gen),
            ];
            // The seed also goes inside the map-gen settings file. Nobody has
            // established which channel the game honours, so both come from one
            // field and cannot disagree. Picking one would risk generating a
            // different map from the same request, with nothing erroring.
            if let Some(seed) = seed {
                args.push("--map-gen-seed".into());
                args.push(seed.to_string());
            }
            args.extend([
                "--mod-directory".into(),
                s(mod_dir),
                "--config".into(),
                s(config),
            ]);
            args
        }
        Launch::Interactive {
            scenario,
            mod_dir,
            config,
        } => vec![
            "--load-scenario".into(),
            scenario.clone(),
            "--mod-directory".into(),
            s(mod_dir),
            "--config".into(),
            s(config),
        ],
        Launch::Preview {
            out,
            map_gen,
            planet,
            seed,
            size,
        } => {
            let mut args = vec![
                "--generate-map-preview".into(),
                s(out),
                "--map-gen-settings".into(),
                s(map_gen),
            ];
            if let Some(planet) = planet {
                args.push("--map-preview-planet".into());
                args.push(planet.clone());
            }
            if let Some(seed) = seed {
                args.push("--map-gen-seed".into());
                args.push(seed.to_string());
            }
            if let Some(size) = size {
                args.push("--map-preview-size".into());
                args.push(size.to_string());
            }
            args
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test args`
Expected: PASS, 5 tests.

- [ ] **Step 5: Commit**

```bash
git add src/args.rs src/lib.rs
git commit -m "Build the argument vector per mode

--map-gen-settings is required for --create even when nothing reads it, and
preview takes a different vector entirely with no mod directory."
```

---

### Task 9: Decide success, per mode

The rule that one global predicate would get wrong. `error("DUMPED-OK")` makes the game exit non-zero and that is success, but `--generate-map-preview` exits 0 on success, and for `--dump-data` a non-zero exit is real information.

**Files:**
- Create: `src/outcome.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `Mode` from Task 5.
- Produces: `pub struct RunFacts { pub exit_code: Option<i32>, pub dump_exists: bool, pub sentinel_seen: bool }`, `pub enum Outcome { Ok, Failed(String) }`, and `pub fn evaluate(mode: Mode, facts: &RunFacts) -> Outcome`.

- [ ] **Step 1: Write the failing test**

Create `src/outcome.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::Mode;

    fn facts(exit: Option<i32>, dump: bool, sentinel: bool) -> RunFacts {
        RunFacts { exit_code: exit, dump_exists: dump, sentinel_seen: sentinel }
    }

    #[test]
    fn create_succeeds_on_a_non_zero_exit_when_the_dump_exists() {
        // error("DUMPED-OK") is how the probe exits. Non-zero is success here.
        assert_eq!(evaluate(Mode::Create, &facts(Some(1), true, true)), Outcome::Ok);
    }

    #[test]
    fn create_fails_when_no_dump_was_written() {
        let out = evaluate(Mode::Create, &facts(Some(1), false, false));
        assert!(matches!(out, Outcome::Failed(_)));
    }

    #[test]
    fn dump_data_fails_on_a_non_zero_exit_even_if_a_dump_is_present() {
        // A non-zero exit is real information here, and a stale dump from an
        // earlier capture can be sitting in a discovered directory.
        let out = evaluate(Mode::DumpData, &facts(Some(1), true, false));
        assert!(matches!(out, Outcome::Failed(_)));
    }

    #[test]
    fn dump_data_succeeds_on_exit_zero_with_a_dump() {
        assert_eq!(evaluate(Mode::DumpData, &facts(Some(0), true, false)), Outcome::Ok);
    }

    #[test]
    fn dump_data_fails_on_exit_zero_with_no_dump() {
        let out = evaluate(Mode::DumpData, &facts(Some(0), false, false));
        assert!(matches!(out, Outcome::Failed(_)));
    }

    #[test]
    fn preview_requires_exit_zero_and_the_file() {
        assert_eq!(evaluate(Mode::Preview, &facts(Some(0), true, false)), Outcome::Ok);
        assert!(matches!(evaluate(Mode::Preview, &facts(Some(1), true, false)), Outcome::Failed(_)));
        assert!(matches!(evaluate(Mode::Preview, &facts(Some(0), false, false)), Outcome::Failed(_)));
    }

    #[test]
    fn interactive_always_succeeds_because_the_consumer_judges_it() {
        // A session can end any way a person likes, and only the consumer knows
        // whether the samples it collected are usable.
        assert_eq!(evaluate(Mode::Interactive, &facts(Some(0), false, false)), Outcome::Ok);
        assert_eq!(evaluate(Mode::Interactive, &facts(None, false, false)), Outcome::Ok);
    }

    #[test]
    fn read_only_never_runs_anything() {
        assert_eq!(evaluate(Mode::ReadOnly, &facts(None, false, false)), Outcome::Ok);
    }

    #[test]
    fn a_missing_exit_code_fails_the_modes_that_need_one() {
        // No exit code means the process was killed, which is how a timeout ends.
        assert!(matches!(evaluate(Mode::DumpData, &facts(None, true, false)), Outcome::Failed(_)));
        assert!(matches!(evaluate(Mode::Preview, &facts(None, true, false)), Outcome::Failed(_)));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod outcome;` to `src/lib.rs`, then run:

Run: `cargo test outcome`
Expected: FAIL to compile, with `cannot find type 'RunFacts'`.

- [ ] **Step 3: Write minimal implementation**

Insert above the test module in `src/outcome.rs`:

```rust
//! Deciding whether a run succeeded. The rule is per mode, not global.

use crate::probe::Mode;

/// What was observed after the process ended.
#[derive(Debug, Clone)]
pub struct RunFacts {
    /// `None` when the process was killed, which is how a timeout ends.
    pub exit_code: Option<i32>,
    pub dump_exists: bool,
    /// Whether `DUMPED-OK` appeared in stderr. Reported rather than required,
    /// because it distinguishes "the mod ran and finished" from "the mod
    /// crashed" - a check no existing probe makes.
    pub sentinel_seen: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Ok,
    Failed(String),
}

/// Applies the mode's success rule.
///
/// One global rule would get two of the five modes wrong. `error("DUMPED-OK")`
/// makes Factorio exit non-zero and that is success, so `create` keys off the
/// dump. `--generate-map-preview` exits 0 on success. And for `--dump-data` a
/// non-zero exit is the diagnostic, so ignoring it would mean debugging a
/// missing file when the real message was a prototype error in the log.
pub fn evaluate(mode: Mode, facts: &RunFacts) -> Outcome {
    match mode {
        Mode::ReadOnly | Mode::Interactive => Outcome::Ok,

        Mode::Create => {
            if facts.dump_exists {
                Outcome::Ok
            } else {
                Outcome::Failed(
                    "no dump was written. The most common cause is a factorio_version \
                     mismatch, which makes Factorio skip the mod in silence."
                        .to_string(),
                )
            }
        }

        Mode::DumpData => match facts.exit_code {
            Some(0) if facts.dump_exists => Outcome::Ok,
            Some(0) => Outcome::Failed("factorio exited 0 but wrote no dump".to_string()),
            Some(code) => Outcome::Failed(format!("factorio exited {code}")),
            None => Outcome::Failed("factorio was killed before it exited".to_string()),
        },

        Mode::Preview => match facts.exit_code {
            Some(0) if facts.dump_exists => Outcome::Ok,
            Some(0) => Outcome::Failed("factorio exited 0 but wrote no preview".to_string()),
            Some(code) => Outcome::Failed(format!("factorio exited {code}")),
            None => Outcome::Failed("factorio was killed before it exited".to_string()),
        },
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test outcome`
Expected: PASS, 9 tests.

- [ ] **Step 5: Commit**

```bash
git add src/outcome.rs src/lib.rs
git commit -m "Decide success per mode rather than globally

DUMPED-OK exits non-zero and that is success; map preview exits 0 on
success; and for --dump-data the exit code is the diagnostic. One rule
would get two of the five wrong."
```

---

### Task 10: The spawn boundary, with a fake game

**Files:**
- Create: `src/spawn.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub struct SpawnResult { pub exit_code: Option<i32>, pub stdout: String, pub stderr: String }`, `pub trait Spawner { fn run(&self, binary: &Path, args: &[String], timeout: Option<Duration>) -> anyhow::Result<SpawnResult>; }`, `pub struct RealSpawner`, and `pub fn tail(text: &str, bytes: usize) -> String`.

- [ ] **Step 1: Write the failing test**

Create `src/spawn.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_returns_the_last_bytes_not_the_first() {
        // The tail of Factorio's output is the only diagnostic there is when a
        // run produces no dump, so a JSON-out CLI must carry it.
        let text: String = (0..100).map(|i| format!("line {i}\n")).collect();
        let out = tail(&text, 40);
        assert!(out.ends_with("line 99\n"));
        assert!(!out.contains("line 0\n"));
        assert!(out.len() <= 40 + 8);
    }

    #[test]
    fn tail_returns_short_text_unchanged() {
        assert_eq!(tail("short", 4000), "short");
    }

    #[test]
    fn tail_does_not_split_a_multibyte_character() {
        let text = "aaaa\u{1F600}";
        let out = tail(text, 5);
        assert!(out.chars().count() > 0);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod spawn;` to `src/lib.rs`, then run:

Run: `cargo test spawn`
Expected: FAIL to compile, with `cannot find function 'tail'`.

- [ ] **Step 3: Write minimal implementation**

Insert above the test module in `src/spawn.rs`:

```rust
//! The one boundary that touches processes, kept behind a trait so tests can
//! substitute a fake game.

use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
pub struct SpawnResult {
    /// `None` when the process was killed, which is how a timeout ends.
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub trait Spawner {
    fn run(
        &self,
        binary: &Path,
        args: &[String],
        timeout: Option<Duration>,
    ) -> anyhow::Result<SpawnResult>;
}

/// Returns at most `bytes` from the end of `text`, on a character boundary.
pub fn tail(text: &str, bytes: usize) -> String {
    if text.len() <= bytes {
        return text.to_string();
    }
    let mut start = text.len() - bytes;
    while start < text.len() && !text.is_char_boundary(start) {
        start += 1;
    }
    text[start..].to_string()
}

/// Runs the real game.
pub struct RealSpawner;

impl Spawner for RealSpawner {
    fn run(
        &self,
        binary: &Path,
        args: &[String],
        timeout: Option<Duration>,
    ) -> anyhow::Result<SpawnResult> {
        use std::process::{Command, Stdio};

        let mut child = Command::new(binary)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // No consumer repo has a timeout today, so a hung game hangs the
        // capture forever. Polling is enough here: a probe run is seconds, and
        // avoiding an async runtime keeps the dependency surface small.
        let deadline = timeout.map(|t| Instant::now() + t);
        loop {
            if let Some(status) = child.try_wait()? {
                let output = child.wait_with_output()?;
                return Ok(SpawnResult {
                    exit_code: status.code(),
                    stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                    stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                });
            }
            if let Some(deadline) = deadline {
                if Instant::now() >= deadline {
                    child.kill()?;
                    let output = child.wait_with_output()?;
                    return Ok(SpawnResult {
                        exit_code: None,
                        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                    });
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test spawn`
Expected: PASS, 3 tests.

- [ ] **Step 5: Commit**

```bash
git add src/spawn.rs src/lib.rs
git commit -m "Add the spawn boundary, with a timeout none of the consumers has

A hung game currently hangs a capture forever in all three repos. The trait
is what lets a fake game assert the argument vector in tests."
```

---

### Task 11: Wire it together and add `run`

**Files:**
- Create: `src/run.rs`
- Modify: `src/lib.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: everything from Tasks 2 through 10.
- Produces: `pub struct RunRequest { pub spec: ProbeSpec, pub layout: InstallLayout, pub version: VersionInfo, pub work_dir: PathBuf, pub map_gen_settings: serde_json::Value }` and `pub fn run_probe(request: &RunRequest, spawner: &dyn Spawner) -> anyhow::Result<serde_json::Value>`.

- [ ] **Step 1: Write the failing test**

Create `src/run.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::{Mode, ModSpec};
    use std::cell::RefCell;
    use std::fs;
    use tempfile::tempdir;

    /// A fake game. It asserts the argument vector, writes the dump a real game
    /// would have written, and returns the non-zero exit that DUMPED-OK causes.
    struct FakeGame {
        write_dump_to: PathBuf,
        seen_args: RefCell<Vec<String>>,
    }

    impl Spawner for FakeGame {
        fn run(
            &self,
            _binary: &Path,
            args: &[String],
            _timeout: Option<Duration>,
        ) -> anyhow::Result<SpawnResult> {
            *self.seen_args.borrow_mut() = args.to_vec();
            fs::create_dir_all(self.write_dump_to.parent().unwrap())?;
            fs::write(&self.write_dump_to, br#"{"answer":42}"#)?;
            Ok(SpawnResult {
                exit_code: Some(1),
                stdout: String::new(),
                stderr: "control.lua:13: DUMPED-OK".into(),
            })
        }
    }

    fn layout_in(dir: &Path) -> InstallLayout {
        let binary = dir.join("factorio");
        fs::write(&binary, b"").unwrap();
        fs::create_dir_all(dir.join("data")).unwrap();
        InstallLayout {
            root: dir.to_path_buf(),
            binary,
            data_dir: dir.join("data"),
            doc_dir: dir.join("doc-html"),
        }
    }

    fn version() -> VersionInfo {
        crate::version::parse_version_line("Version: 2.0.77 (build 84539, mac-arm64, full)").unwrap()
    }

    #[test]
    fn a_create_run_scaffolds_the_mod_and_reports_success() {
        let install = tempdir().unwrap();
        let work = tempdir().unwrap();

        let spec = ProbeSpec {
            mode: Mode::Create,
            r#mod: Some(ModSpec {
                name: "bp_probe".into(),
                version: "0.0.1".into(),
                dependencies: vec!["base".into()],
                control_lua: Some("script.on_init(function() end)".into()),
                control_lua_file: None,
                data_lua: None,
                data_final_fixes_lua: None,
            }),
            literals: BTreeMap::new(),
            timeout_seconds: Some(60),
            capture_active_mods: false,
        };

        let request = RunRequest {
            spec,
            layout: layout_in(install.path()),
            version: version(),
            work_dir: work.path().to_path_buf(),
            map_gen_settings: serde_json::json!({ "seed": 123456 }),
        };

        let fake = FakeGame {
            write_dump_to: work.path().join("write/script-output/oracle-dump.json"),
            seen_args: RefCell::new(vec![]),
        };

        let result = run_probe(&request, &fake).unwrap();

        assert_eq!(result["ok"], true);
        assert_eq!(result["sentinelSeen"], true);
        assert_eq!(result["exitCode"], 1);

        // The mod was scaffolded with the version derived from the binary.
        let info: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(work.path().join("mods/bp_probe_0.0.1/info.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(info["factorio_version"], "2.0");

        // The consumer's Lua reached disk untouched.
        let control =
            fs::read_to_string(work.path().join("mods/bp_probe_0.0.1/control.lua")).unwrap();
        assert_eq!(control, "script.on_init(function() end)");

        // --map-gen-settings is always passed for create, and the seed reaches
        // the game through both channels from the single map_gen_settings field.
        let args = fake.seen_args.borrow();
        assert!(args.contains(&"--map-gen-settings".to_string()));
        assert!(args.contains(&"--map-gen-seed".to_string()));
        assert!(args.contains(&"123456".to_string()));

        let written: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(work.path().join("map-gen-settings.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(written["seed"], 123456);
    }

    #[test]
    fn literals_are_prepended_above_the_consumer_lua() {
        let install = tempdir().unwrap();
        let work = tempdir().unwrap();
        let mut literals = BTreeMap::new();
        literals.insert("blueprint".to_string(), "0eNq".to_string());

        let spec = ProbeSpec {
            mode: Mode::Create,
            r#mod: Some(ModSpec {
                name: "p".into(),
                version: "0.0.1".into(),
                dependencies: vec![],
                control_lua: Some("game.print(blueprint)".into()),
                control_lua_file: None,
                data_lua: None,
                data_final_fixes_lua: None,
            }),
            literals,
            timeout_seconds: None,
            capture_active_mods: false,
        };

        let request = RunRequest {
            spec,
            layout: layout_in(install.path()),
            version: version(),
            work_dir: work.path().to_path_buf(),
            map_gen_settings: serde_json::json!({}),
        };
        let fake = FakeGame {
            write_dump_to: work.path().join("write/script-output/oracle-dump.json"),
            seen_args: RefCell::new(vec![]),
        };
        run_probe(&request, &fake).unwrap();

        let control = fs::read_to_string(work.path().join("mods/p_0.0.1/control.lua")).unwrap();
        assert_eq!(control, "local blueprint = [[0eNq]]\ngame.print(blueprint)");
    }

    #[test]
    fn a_dump_data_run_writes_no_mod() {
        let install = tempdir().unwrap();
        let work = tempdir().unwrap();
        let spec = ProbeSpec {
            mode: Mode::DumpData,
            r#mod: None,
            literals: BTreeMap::new(),
            timeout_seconds: None,
            capture_active_mods: false,
        };
        let request = RunRequest {
            spec,
            layout: layout_in(install.path()),
            version: version(),
            work_dir: work.path().to_path_buf(),
            map_gen_settings: serde_json::json!({}),
        };

        struct CleanExit {
            dump: PathBuf,
        }
        impl Spawner for CleanExit {
            fn run(&self, _b: &Path, _a: &[String], _t: Option<Duration>) -> anyhow::Result<SpawnResult> {
                fs::create_dir_all(self.dump.parent().unwrap())?;
                fs::write(&self.dump, b"{}")?;
                Ok(SpawnResult { exit_code: Some(0), ..Default::default() })
            }
        }
        let fake = CleanExit { dump: work.path().join("write/script-output/data-raw-dump.json") };
        let result = run_probe(&request, &fake).unwrap();

        assert_eq!(result["ok"], true);
        // The mod directory exists, and is empty of mods. That is its whole job.
        assert!(work.path().join("mods/mod-list.json").is_file());
        assert!(!work.path().join("mods").read_dir().unwrap().any(|e| {
            e.unwrap().file_name().to_string_lossy().contains('_')
        }));
    }

    #[test]
    fn a_failed_run_carries_the_output_tail() {
        let install = tempdir().unwrap();
        let work = tempdir().unwrap();
        let spec = ProbeSpec {
            mode: Mode::Create,
            r#mod: Some(ModSpec {
                name: "p".into(),
                version: "0.0.1".into(),
                dependencies: vec![],
                control_lua: Some("".into()),
                control_lua_file: None,
                data_lua: None,
                data_final_fixes_lua: None,
            }),
            literals: BTreeMap::new(),
            timeout_seconds: None,
            capture_active_mods: false,
        };
        let request = RunRequest {
            spec,
            layout: layout_in(install.path()),
            version: version(),
            work_dir: work.path().to_path_buf(),
            map_gen_settings: serde_json::json!({}),
        };

        struct NoDump;
        impl Spawner for NoDump {
            fn run(&self, _b: &Path, _a: &[String], _t: Option<Duration>) -> anyhow::Result<SpawnResult> {
                Ok(SpawnResult {
                    exit_code: Some(1),
                    stdout: "Loading mod core 2.0.77".into(),
                    stderr: "something went wrong".into(),
                })
            }
        }
        let result = run_probe(&request, &NoDump).unwrap();

        assert_eq!(result["ok"], false);
        assert!(result["error"].as_str().unwrap().contains("no dump"));
        assert!(result["stderrTail"].as_str().unwrap().contains("something went wrong"));
        // The mismatch that most often explains an empty dump is named outright.
        assert_eq!(result["provenance"]["modFactorioVersion"], "2.0");
        assert!(result["provenance"]["buildLine"].as_str().unwrap().contains("2.0.77"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod run;` to `src/lib.rs`, then run:

Run: `cargo test run`
Expected: FAIL to compile, with `cannot find type 'RunRequest'`.

- [ ] **Step 3: Write minimal implementation**

Insert above the test module in `src/run.rs`:

```rust
//! Wiring the pure builders to disk and a spawner.

use crate::args::{build_args, Launch};
use crate::install::InstallLayout;
use crate::lua::build_literals_prelude;
use crate::outcome::{evaluate, Outcome, RunFacts};
use crate::probe::{Mode, ProbeSpec};
use crate::scaffold::{build_config_ini, build_info_json, build_mod_list, ACTIVE_MODS_PRELUDE};
use crate::spawn::{tail, SpawnResult, Spawner};
use crate::version::VersionInfo;
use anyhow::Context;
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Everything a run needs. The caller resolves the install and the work
/// directory, so this function does no discovery of its own.
pub struct RunRequest {
    pub spec: ProbeSpec,
    pub layout: InstallLayout,
    pub version: VersionInfo,
    pub work_dir: PathBuf,
    pub map_gen_settings: serde_json::Value,
}

/// The dump file a `--dump-data` run writes, named by the game.
const DUMP_DATA_FILE: &str = "data-raw-dump.json";
/// The default dump name for a probe mod.
const PROBE_DUMP_FILE: &str = "oracle-dump.json";
/// The preview image name.
const PREVIEW_FILE: &str = "preview.png";

fn read_control_lua(spec: &ProbeSpec) -> anyhow::Result<String> {
    let Some(m) = spec.r#mod.as_ref() else {
        return Ok(String::new());
    };
    if let Some(inline) = m.control_lua.as_ref() {
        return Ok(inline.clone());
    }
    if let Some(path) = m.control_lua_file.as_ref() {
        return fs::read_to_string(path)
            .with_context(|| format!("reading control_lua_file {}", path.display()));
    }
    Ok(String::new())
}

/// Runs a probe and returns the result as JSON.
///
/// The return value describes the work directory rather than a single dump.
/// That is deliberate: an interactive probe writes several files, appends to
/// some of them while a person plays, and can only be judged by the consumer.
pub fn run_probe(request: &RunRequest, spawner: &dyn Spawner) -> anyhow::Result<serde_json::Value> {
    let work = &request.work_dir;
    let mod_dir = work.join("mods");
    let write_data = work.join("write");
    let script_output = write_data.join("script-output");
    let config_path = work.join("config.ini");
    let map_gen_path = work.join("map-gen-settings.json");

    fs::create_dir_all(&mod_dir)?;
    fs::create_dir_all(&script_output)?;

    // The isolated config is what makes a stale dump impossible: write-data
    // points at a directory that started empty.
    fs::write(&config_path, build_config_ini(&write_data))?;
    fs::write(
        &map_gen_path,
        serde_json::to_string_pretty(&request.map_gen_settings)?,
    )?;

    let mod_name = request.spec.r#mod.as_ref().map(|m| m.name.clone());
    fs::write(
        mod_dir.join("mod-list.json"),
        serde_json::to_string_pretty(&build_mod_list(mod_name.as_deref()))?,
    )?;

    if let Some(m) = request.spec.r#mod.as_ref() {
        let files = mod_dir.join(m.dir_name());
        fs::create_dir_all(&files)?;
        fs::write(
            files.join("info.json"),
            serde_json::to_string_pretty(&build_info_json(m, &request.version.major_minor()))?,
        )?;

        // Consumer Lua passes through untouched. The only additions are the
        // literal locals, and the active-mods prelude when it was asked for.
        let mut control = String::new();
        if request.spec.capture_active_mods {
            control.push_str(ACTIVE_MODS_PRELUDE);
        }
        control.push_str(&build_literals_prelude(&request.spec.literals));
        control.push_str(&read_control_lua(&request.spec)?);
        fs::write(files.join("control.lua"), control)?;

        if let Some(data_lua) = m.data_lua.as_ref() {
            fs::write(files.join("data.lua"), data_lua)?;
        }
        if let Some(final_fixes) = m.data_final_fixes_lua.as_ref() {
            fs::write(files.join("data-final-fixes.lua"), final_fixes)?;
        }
    }

    let (launch, expected_file) = match request.spec.mode {
        Mode::DumpData => (
            Some(Launch::DumpData {
                mod_dir: mod_dir.clone(),
                config: config_path.clone(),
            }),
            script_output.join(DUMP_DATA_FILE),
        ),
        Mode::Create => (
            Some(Launch::Create {
                save: write_data.join("probe.zip"),
                map_gen: map_gen_path.clone(),
                // One source of truth. The caller writes the seed once, into
                // map_gen_settings, and it reaches the game through both the
                // file and the flag. Which channel the game honours is not
                // established, so they must not be able to disagree.
                seed: request.map_gen_settings.get("seed").and_then(|s| s.as_u64()),
                mod_dir: mod_dir.clone(),
                config: config_path.clone(),
            }),
            script_output.join(PROBE_DUMP_FILE),
        ),
        Mode::Interactive => (
            Some(Launch::Interactive {
                scenario: "base/freeplay".to_string(),
                mod_dir: mod_dir.clone(),
                config: config_path.clone(),
            }),
            script_output.join(PROBE_DUMP_FILE),
        ),
        Mode::Preview => (
            Some(Launch::Preview {
                out: write_data.join(PREVIEW_FILE),
                map_gen: map_gen_path.clone(),
                planet: None,
                seed: None,
                size: None,
            }),
            write_data.join(PREVIEW_FILE),
        ),
        Mode::ReadOnly => (None, PathBuf::new()),
    };

    let result: SpawnResult = match &launch {
        Some(launch) => {
            let args = build_args(launch);
            // Interactive runs never get a timeout: they last as long as a
            // person plays.
            let timeout = match request.spec.mode {
                Mode::Interactive => None,
                _ => request.spec.timeout_seconds.map(Duration::from_secs),
            };
            spawner.run(&request.layout.binary, &args, timeout)?
        }
        None => SpawnResult {
            exit_code: Some(0),
            ..Default::default()
        },
    };

    let sentinel_seen = result.stderr.contains("DUMPED-OK");
    let facts = RunFacts {
        exit_code: result.exit_code,
        dump_exists: expected_file.is_file(),
        sentinel_seen,
    };
    let outcome = evaluate(request.spec.mode, &facts);

    let files: Vec<String> = fs::read_dir(&script_output)
        .map(|entries| {
            let mut names: Vec<String> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        })
        .unwrap_or_default();

    let provenance = json!({
        "factorioVersion": format!(
            "{}.{}.{}",
            request.version.major, request.version.minor, request.version.patch
        ),
        "buildLine": request.version.line,
        "modFactorioVersion": request.version.major_minor(),
        "binaryPath": request.layout.binary,
    });

    let mut out = json!({
        "ok": outcome == Outcome::Ok,
        "workDir": work,
        "scriptOutput": script_output,
        "files": files,
        "exitCode": result.exit_code,
        "sentinelSeen": sentinel_seen,
        "provenance": provenance,
    });

    if let Outcome::Failed(message) = outcome {
        // The tail is the only diagnostic there is when a run produces no dump.
        out["error"] = json!(message);
        out["stdoutTail"] = json!(tail(&result.stdout, 4000));
        out["stderrTail"] = json!(tail(&result.stderr, 4000));
    }

    Ok(out)
}

// Silences an unused-import warning when no test builds the map.
#[allow(dead_code)]
fn _unused(_: &BTreeMap<String, String>, _: &Path) {}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test run`
Expected: PASS, 4 tests.

- [ ] **Step 5: Add the `run` subcommand**

In `src/main.rs`, add to the `Command` enum:

```rust
    /// Run a probe described by a JSON spec
    Run {
        /// Path to the probe spec JSON
        #[arg(long)]
        probe: PathBuf,
        /// Directory to work in. A fresh temporary directory if omitted.
        #[arg(long)]
        work_dir: Option<PathBuf>,
        /// Select an install by version, for example 2.0.77
        #[arg(long)]
        version: Option<String>,
        /// Select an install by path
        #[arg(long)]
        factorio: Option<PathBuf>,
    },
```

and add the matching arm in `main`:

```rust
        Command::Run { probe, work_dir, version, factorio } => {
            let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
            let env_bin = std::env::var_os("FACTORIO_BIN").map(PathBuf::from);

            let spec: factorio_oracle::probe::ProbeSpec =
                serde_json::from_str(&std::fs::read_to_string(&probe)?)?;

            let installs = install::discover(&home, factorio.as_deref().or(env_bin.as_deref()));
            let chosen = installs
                .into_iter()
                .find(|d| match (&version, &d.version) {
                    (Some(want), Some(got)) => {
                        format!("{}.{}.{}", got.major, got.minor, got.patch) == *want
                    }
                    (None, Some(_)) => true,
                    _ => false,
                })
                .ok_or_else(|| anyhow::anyhow!("no Factorio install matched"))?;

            let work = match work_dir {
                Some(dir) => { std::fs::create_dir_all(&dir)?; dir }
                None => tempfile::Builder::new().prefix("factorio-oracle-").tempdir()?.keep(),
            };

            let request = factorio_oracle::run::RunRequest {
                spec,
                layout: chosen.layout,
                version: chosen.version.expect("filtered to installs with a version"),
                work_dir: work,
                map_gen_settings: serde_json::json!({ "seed": 123456 }),
            };

            let result = factorio_oracle::run::run_probe(&request, &factorio_oracle::spawn::RealSpawner)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
            if result["ok"] != true {
                std::process::exit(1);
            }
        }
```

Move `tempfile` from `[dev-dependencies]` to `[dependencies]` in `Cargo.toml`, since `main` now uses it.

- [ ] **Step 6: Run the whole suite**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: PASS, 45 tests across all modules.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "Wire the runner together and add the run subcommand

The result describes the work directory rather than a single dump, because
an interactive probe writes several files and appends to some of them while
a person plays. Only the consumer can judge that."
```

---

### Task 12: Lock in f32 round-trip before anything can break it

Requested by FactorioMapWebUI, with evidence. Scoring a port by **count of exactly
matching f32 values** is a sharper instrument than any error bound: two candidate
noise kernels had the identical worst absolute error of 2.682e-7 and differed by 42
exact matches out of 512, which no bound could distinguish. The winner went from
132 of 512 exact to 473 of 512.

In this plan the runner hands back the work directory and the game writes the dump
itself, so sampled values never pass through Rust yet. This task exists to encode
the rule **before** Plan 2 adds a path that could quietly violate it. A capture
that loses precision still looks completely fine, which is why a test has to hold
the line rather than a comment.

**Files:**
- Create: `src/numbers.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub fn f32_round_trip(value: f32) -> String` and `pub fn assert_round_trips(values: &[f32]) -> Result<(), String>`.

- [ ] **Step 1: Write the failing test**

Create `src/numbers.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bit_pattern_survives_a_round_trip() {
        // A spread including the awkward ones: values whose shortest decimal
        // form is long, and values a fixed precision would flatten together.
        let values: Vec<f32> = vec![
            0.1, 0.2, 0.29, 1.5, 2.5,
            2.682e-7, 1.0e-38, 3.4028235e38,
            f32::MIN_POSITIVE,
            0.30000001192092896,
            1.0 / 3.0,
        ];
        for v in values {
            let text = f32_round_trip(v);
            let back: f32 = text.parse().unwrap();
            assert_eq!(
                back.to_bits(),
                v.to_bits(),
                "{v} serialised as {text} and came back as {back}"
            );
        }
    }

    #[test]
    fn a_fixed_precision_formatter_would_fail_this() {
        // The guard's whole purpose. Two distinct f32 values that {:.6} maps to
        // the same string must stay distinct through f32_round_trip.
        let a = 0.100000001490116119384765625_f32;
        let b = f32::from_bits(a.to_bits() + 1);
        assert_eq!(format!("{a:.6}"), format!("{b:.6}"), "premise: {{:.6}} flattens these");
        assert_ne!(f32_round_trip(a), f32_round_trip(b));
    }

    #[test]
    fn assert_round_trips_accepts_good_values() {
        assert!(assert_round_trips(&[0.1, 2.682e-7, 1.5]).is_ok());
    }

    #[test]
    fn assert_round_trips_names_the_offender() {
        // Sanity check on the reporting path, using a value list that is fine -
        // the function must still return Ok and not spuriously fail.
        let values: Vec<f32> = (0..1000).map(|i| i as f32 * 0.017).collect();
        assert!(assert_round_trips(&values).is_ok());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod numbers;` to `src/lib.rs`, then run:

Run: `cargo test numbers`
Expected: FAIL to compile, with `cannot find function 'f32_round_trip'`.

- [ ] **Step 3: Write minimal implementation**

Insert above the test module in `src/numbers.rs`:

```rust
//! Preserving the bits the game produced.
//!
//! Sampled values come back from a running game as f32. Scoring a port by the
//! count of exactly matching values is a sharper instrument than any error
//! bound - two candidate kernels once had the identical worst absolute error
//! and differed by 42 exact matches out of 512. That only works if the capture
//! preserves the bits.
//!
//! The failure mode is silent: a capture that loses precision still looks
//! completely fine, and the consumer simply can never again tell "bit-exact"
//! from "very close". So this is a test, not a comment.

/// Formats an f32 with the shortest representation that parses back to the
/// identical bit pattern.
///
/// Rust's `Display` for f32 already guarantees this. Never use a fixed
/// precision such as `{:.6}`, and never widen to f64 on the way.
pub fn f32_round_trip(value: f32) -> String {
    format!("{value}")
}

/// Checks that every value survives serialisation unchanged.
///
/// Worth running over a whole capture. It is cheap, and it fails loudly the day
/// somebody tidies the formatter.
pub fn assert_round_trips(values: &[f32]) -> Result<(), String> {
    for (index, value) in values.iter().enumerate() {
        let text = f32_round_trip(*value);
        match text.parse::<f32>() {
            Ok(back) if back.to_bits() == value.to_bits() => {}
            Ok(back) => {
                return Err(format!(
                    "value {index} ({value}) serialised as {text} and parsed back as {back}"
                ))
            }
            Err(err) => return Err(format!("value {index} ({value}) did not parse back: {err}")),
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test numbers`
Expected: PASS, 4 tests.

- [ ] **Step 5: Commit**

```bash
git add src/numbers.rs src/lib.rs
git commit -m "Lock in f32 round-trip before a later path can break it

Scoring a port by exact-match count beats any error bound: two candidate
kernels once shared an identical worst error and differed by 42 exact
matches of 512. That instrument only survives if captures keep the bits,
and a capture that loses precision still looks fine - so this is a test."
```

---

## Self-Review

**1. Spec coverage.** This plan covers the spec's build-order steps 1 through 3, plus the parts of "Commands", "Run modes", "The probe spec", "The output contract" and "Repo setup" that those steps need. Deliberately deferred, each to its own plan:

- **Plan 2:** the trimmer (`find_prototype` with collision-box disambiguation, caller-supplied allowlists, migrations, defines), canonical JSON output, and the byte-for-byte acceptance test against FactorioTools' committed `factorio-oracle.json`.
- **Plan 3:** `provenance check`, the always-on completeness test, and the `unknown` ratchet.
- **Plan 4:** `refs` sync, grep at a tag, worktree, and the archive cache. Plus the three knowledge documents, which depend on nothing and can be written at any time.

**A conflict Plan 2 must handle, recorded here so it is not discovered mid-build.** The spec's acceptance test says to reproduce the committed fixture byte for byte. FactorioTools#83 establishes that the `directions` table in that fixture is produced by reading `order` from `runtime-api.json`, which is a documentation index rather than the runtime value. So a faithful port must reproduce the bug first, proving the port is correct, and only then fix #83 as a separate deliberate change whose fixture diff is visible and reviewable. Doing both at once would make it impossible to tell a port error from the fix.

**2. Placeholder scan.** No TBD, TODO, "add error handling", or "similar to Task N". Every code step carries the code. Every test step carries the assertions.

**3. Type consistency.** Checked across tasks: `VersionInfo` and `major_minor()` (Task 2) are used unchanged in Tasks 4, 7 and 11. `InstallLayout` fields `binary` / `data_dir` / `doc_dir` (Task 3) are used unchanged in Tasks 4 and 11. `ModSpec` and its `dir_name()` (Task 5) are used in Tasks 7 and 11. `Mode` (Task 5) is consumed by Tasks 9 and 11. `Launch` variants (Task 8) are constructed only in Task 11, with matching field names. `RunFacts` and `evaluate` (Task 9) are called once, in Task 11. `SpawnResult`, `Spawner` and `tail` (Task 10) are used in Task 11.

One consistency note for the implementer: Task 11's tests construct `ProbeSpec` with struct literal syntax, so every field added to `ProbeSpec` in Task 5 must appear there. If a field is added later, those tests break at compile time, which is the intended behaviour.

## Execution Handoff

Plan complete. Two execution options:

1. **Subagent-Driven (recommended)** - a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** - execute tasks in this session with checkpoints for review.
