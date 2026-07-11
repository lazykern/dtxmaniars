# Fast Build and Worktree Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reuse Bevy artifacts across worktrees, shrink test artifacts, and document a fast verification workflow that serializes only heavy Cargo jobs.

**Architecture:** Worktrees retain the existing shared Cargo target because measured isolated targets require 30+ GiB cold builds per Bevy worktree. A user-local sccache installation provides secondary reuse across cleans and compatible variants. Repo profiles reduce debug-data generation; `AGENTS.md` defines package-scoped commands, serialized heavy gates, and dev-only Bevy dynamic linking.

**Tech Stack:** Cargo 1.96, Rust 1.96, Bevy 0.19, mold/BFD, sccache, cargo-nextest, git worktrees.

## Global Constraints

- Keep Bevy `dynamic_linking` command-line-only and absent from release builds.
- Preserve existing uncommitted `.cargo/config.toml` BFD-to-mold change.
- Do not delete `$HOME/.cache/cargo-target` or nested targets without explicit confirmation.
- Keep incremental compilation enabled for workspace/path crates.
- Do not add Cranelift, task runners, wrapper scripts, or permanent dynamic-linking features.
- Use package-scoped checks/tests during edits; retain workspace gates before merge.

---

### Task 1: Reduce dev and test debug artifacts

**Files:**
- Modify: `Cargo.toml`

**Interfaces:**
- Consumes: Cargo built-in `dev` and `test` profile inheritance.
- Produces: compact default debug artifacts plus opt-in `debugging` profile.

- [ ] **Step 1: Record current profile behavior**

Run:

```sh
cargo metadata --no-deps --format-version 1 >/dev/null
rg -n '^\[profile\.|^(opt-level|debug|inherits)' Cargo.toml
```

Expected: existing `profile.dev` and dependency optimization settings; no `profile.debugging`.

- [ ] **Step 2: Add reduced debug information and debugger profile**

Set profile section to:

```toml
[profile.dev]
opt-level = 1
# Keep line tables for useful panic backtraces without full debug-data cost.
debug = "line-tables-only"

[profile.dev.package."*"]
opt-level = 3
debug = false

[profile.debugging]
inherits = "dev"
debug = true

[profile.release]
lto = "thin"
codegen-units = 1
```

- [ ] **Step 3: Validate Cargo profiles**

Run:

```sh
cargo metadata --no-deps --format-version 1 >/dev/null
cargo check -p dtx-core
```

Expected: both commands exit 0.

- [ ] **Step 4: Commit repo profile change**

```sh
git add Cargo.toml
git commit -m "build: reduce dev debug artifacts"
```

### Task 2: Document fast worktree workflow

**Files:**
- Modify: `AGENTS.md`

**Interfaces:**
- Consumes: commands supported by Cargo, Bevy 0.19, and installed cargo-nextest.
- Produces: canonical developer/agent build and verification policy.

- [ ] **Step 1: Replace broad quickstart guidance with tiered commands**

Document these exact inner-loop commands:

```sh
cargo check -p <changed-package>
cargo test -p <changed-package> --lib
cargo test -p <changed-package> --test <integration-test>
cargo nextest run -p <changed-package>
```

Document these Bevy-specific commands:

```sh
cargo run -p dtxmaniars-desktop --features bevy/dynamic_linking
cargo test -p gameplay-drums --features bevy/dynamic_linking --test <integration-test>
```

Document these pre-merge gates:

```sh
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
CARGO_BUILD_JOBS=1 cargo test --workspace --all-targets
```

State that package tests should replace the full workspace test when the known full-suite memory ceiling remains unresolved, matching CI policy.

- [ ] **Step 2: Add worktree cache policy**

Document:

```text
- Worktrees share the configured target directory to avoid repeated 30+ GiB Bevy cold builds.
- Parallelize editing and small package checks; serialize Bevy-heavy and workspace-wide Cargo commands.
- sccache provides secondary reuse across cleans and compatible variants.
- SCCACHE_BASEDIRS must include the common absolute worktree parent.
- Linker, rustflags, toolchain, profile, and feature changes invalidate artifacts.
- Never clean the shared target while Cargo is active or without confirmation.
```

- [ ] **Step 3: Validate documentation commands and formatting**

Run:

```sh
rg -n 'dynamic_linking|sccache|CARGO_BUILD_JOBS|nextest|target/' AGENTS.md
cargo metadata --no-deps --format-version 1 >/dev/null
```

Expected: each policy term appears; Cargo metadata exits 0.

- [ ] **Step 4: Commit documentation**

```sh
git add AGENTS.md
git commit -m "docs: define fast worktree workflow"
```

### Task 3: Configure user-local cross-worktree caching

**Files:**
- Modify: `$HOME/.cargo/config.toml` (user-local, never commit)

**Interfaces:**
- Consumes: installed `sccache` executable and worktrees under `/home/lazykern/lab`.
- Produces: retained shared Cargo artifacts plus normalized sccache keys.

- [ ] **Step 1: Install sccache without changing repo manifests**

Prefer prebuilt cargo-binstall when available:

```sh
cargo binstall --no-confirm sccache
```

Fallback:

```sh
cargo install --locked sccache
```

Expected: `sccache --version` exits 0.

- [ ] **Step 2: Add sccache without removing shared target**

Set `$HOME/.cargo/config.toml` to:

```toml
[build]
target-dir = "/home/lazykern/.cache/cargo-target"
rustc-wrapper = "sccache"
```

Set sccache path normalization in `$HOME/.config/sccache/config`:

```toml
basedirs = ["/home/lazykern/lab"]
```

Do not modify repo `.cargo/config.toml`; its current uncommitted mold choice remains user-owned.

- [ ] **Step 3: Verify shared target and sccache startup**

Run from repo root:

```sh
cargo metadata --no-deps --format-version 1
TMPDIR=/tmp sccache --start-server
sccache --show-stats
```

Expected: metadata reports `/home/lazykern/.cache/cargo-target`; sccache reports a local cache without server errors. Start persistent sccache from a host shell, not an ephemeral context-mode sandbox.

- [ ] **Step 4: Warm smallest cache path**

Run:

```sh
cargo check -p dtx-core
sccache --show-stats
```

Expected: check exits 0; stats show compile requests. A first run may have zero hits.

### Task 4: Verify Bevy development path and report cleanup candidates

**Files:**
- No file changes.

**Interfaces:**
- Consumes: repo profiles, documented commands, shared target, sccache.
- Produces: build evidence and non-destructive disk report.

- [ ] **Step 1: Check Bevy package with dynamic linking**

Run:

```sh
cargo check -p dtxmaniars-desktop --features bevy/dynamic_linking
```

Expected: exit 0. Do not ship this feature.

- [ ] **Step 2: Run representative pure tests**

Run:

```sh
cargo nextest run -p dtx-core
```

Expected: exit 0 with all `dtx-core` tests passing.

- [ ] **Step 3: Verify repo-wide type checking under bounded jobs**

Run:

```sh
CARGO_BUILD_JOBS=1 cargo check --workspace
```

Expected: exit 0. If failure is code-related, report exact diagnostics; do not call workflow complete.

- [ ] **Step 4: Report cache cleanup without deleting**

Run:

```sh
du -sh "$HOME/.cache/cargo-target" crates/*/target 2>/dev/null || true
```

Report reclaimable paths and sizes. Ask separately before deleting them.

- [ ] **Step 5: Commit any remaining tracked workflow files only**

```sh
git status --short
git diff --check
```

Expected: no unintended tracked changes; `$HOME/.cargo/config.toml` remains untracked by repo.
