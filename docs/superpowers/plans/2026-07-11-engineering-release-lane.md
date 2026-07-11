# Engineering Release Lane Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the release gate honest and green: pinned toolchain, clean fmt/clippy, CI that actually runs tests in low-memory package groups, repaired AGENTS.md links, removed Pure→Engine layer violations, and one versioned release artifact for a single supported target.

**Architecture:** Pure config/CI/doc work plus two small crate-boundary refactors. The layer fixes move code down the dependency graph (pure math into `dtx-core`, bindings into `dtx-input`) with re-exports so call sites keep compiling. Release packaging is a tag-triggered GitHub workflow, no new tooling crates.

**Tech Stack:** GitHub Actions, cargo workspaces, rust-toolchain.toml, serde/toml (existing).

**Source basis (verified 2026-07-11):**
- CI: `.github/workflows/ci.yml` (45 lines) — single job named `fmt + clippy + test` but tests intentionally omitted (lines 42-45: workspace test OOMs on 7 GB runners).
- No `rust-toolchain.toml`, no `rustfmt.toml` anywhere; CI uses floating `dtolnay/rust-toolchain@stable`. Local rustfmt drift is a known hazard (never run bare `cargo fmt --all` until pinned).
- `crates/dtx-audio` (lib.rs 706, preview.rs 692, crossfade.rs 149 lines) has zero `#[allow]`; clippy `-D warnings` failures reported by roadmap.
- AGENTS.md dead links: `docs/decisions/*` (dir does not exist), `docs/ROADMAP.md`, `docs/ARCHITECTURE.md`, `docs/BEVY_PATTERNS.md`, `docs/UX_UI_DESIGN.md`. Quickstart fixture path wrong (`tests/fixtures/dtx-core/minimal.dtx` → actually `crates/dtx-core/tests/fixtures/minimal.dtx`); `dtx-cli` binary is named `dtx`.
- Violation A: `crates/dtx-core/Cargo.toml` deps `dtx-timing` (Engine: bevy + kira + dtx-audio). Sole use: `crates/dtx-core/src/cdtx_config.rs:12` — `use dtx_timing::math::{chip_time_ms_with_bpm_changes, BpmChange};` (call at :101, test at :211). `dtx_timing::math` module itself has no bevy usage.
- Violation B: `crates/dtx-config/Cargo.toml` deps `dtx-input` (Engine: re-exports `bevy::input::keyboard::KeyCode` at `crates/dtx-input/src/lib.rs:32`). Sole production use: `crates/dtx-config/src/bindings.rs:11` — `use dtx_input::KeyCode;`, payload of `BindSource::Key(KeyCode)` at `bindings.rs:38`.
- Binaries: `dtxmaniars` (`app/dtxmaniars-desktop`, features default `["brp","midi"]`) and `dtx` (`tools/dtx-cli`). No tags, no release workflow, `[profile.release] lto="thin", codegen-units=1`.
- Workspace: 20 packages. Pure (no bevy): dtx-core, dtx-config, dtx-scoring, dtx-layout, xtask, dtx-cli. Engine/game: the rest.

**Prerequisite:** working tree must be clean (current mid-merge state committed/resolved first). Do not start any task on a dirty tree.

---

## Execution status (updated 2026-07-11, branch `worktree-eng-release-lane`)

Executed on top of a `main` that had since merged the input/profile registries and the midi-consumer fix. Outcomes diverged from the original task list:

- **Task 1 (toolchain pin):** ✅ done — `rust-toolchain.toml` + CI action `@1.96.0`.
- **Task 2 (fmt commit):** ✅ done — pinned rustfmt across workspace; `fmt --check` clean.
- **Task 3 (clippy `-D warnings`):** ✅ done, but far larger than scoped. The 1.96.0 pin + main's merges exposed clippy debt across *every* previously-unreachable bevy crate (whack-a-mole: each fixed dep layer revealed the next). Fixed via crate-level `#![allow(too_many_arguments, type_complexity)]` for bevy false-positives (gameplay-drums, game-menu, game-results) + ~23 genuine fixes. **Full-workspace `clippy --all-targets -- -D warnings` is green.**
- **Task 4 (CI test package groups):** ⏭️ **skipped** — user deprioritized CI/CD ("don't need ci cd now"). Note: current `ci.yml` deliberately runs *format + clippy only* (tests OOM on 7 GB runners); merged AGENTS.md still claims "CI uses package tests" — a pre-existing main inconsistency left as-is.
- **Task 5 (AGENTS.md repair):** ✅ done — dead links + `docs/decisions/README.md`; correct fixture path preserved through rebase conflict.
- **Task 6 (dtx-core→dtx-timing):** ✅ **already achieved upstream** — the pure math now lives in `dtx-core::timing`; `dtx-timing` re-exports it as `math`. dtx-core deps are empty (no cycle). Reduced to one dangling doc-link fix in `beat_lines.rs`.
- **Task 7 (dtx-config→dtx-input):** ✅ **done** (after `feat/input-lane-profiles` merged to main). Moved `bindings.rs` + `profiles.rs` into dtx-input; the dependency edge reversed to Engine→Pure (no cycle). dtx-input gained dtx-config + dtx-persistence + toml; dtx-config dropped dtx-input + dtx-persistence. 9 gameplay-drums call sites updated. **`cargo tree -p dtx-config | grep bevy` = 0** — dtx-config is now genuinely Pure. Serde round-trip schema tests moved unmodified and pass (on-disk key/MIDI format unchanged); gameplay-drums 485 tests pass; workspace clippy gate green. Ported modules exempted from dtx-input's `#![warn(missing_docs)]`.
- **Task 8 (release.yml):** ⏭️ **skipped** — user deprioritized CI/CD.

---

### Task 1: Pin the toolchain (rustfmt drift killer)

**Files:**
- Create: `rust-toolchain.toml`
- Modify: `.github/workflows/ci.yml:18-20`

- [ ] **Step 1: Check local toolchain version**

Run: `rustc --version && cargo fmt --version`
Expected: rustc ≥ 1.95 (workspace `rust-version = "1.95"`). Note the exact rustc version X.Y.Z — use it below (do not invent a different one).

- [ ] **Step 2: Create rust-toolchain.toml**

```toml
[toolchain]
channel = "1.95.0"
components = ["rustfmt", "clippy"]
```

If Step 1 showed a newer installed stable (e.g. 1.96.x), pin that exact version instead — the point is CI and local agree byte-for-byte on rustfmt output.

- [ ] **Step 3: Point CI at the pinned toolchain**

In `.github/workflows/ci.yml`, replace:

```yaml
      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
```

with:

```yaml
      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@1.95.0
        with:
          components: rustfmt, clippy
```

(Match the version pinned in Step 2. `dtolnay/rust-toolchain` takes the version from the action ref, not from rust-toolchain.toml, so both must state it.)

- [ ] **Step 4: Verify toolchain resolution**

Run: `rustup show active-toolchain && cargo fmt --version`
Expected: active toolchain `1.95.0-x86_64-unknown-linux-gnu` (rustup auto-installs it from rust-toolchain.toml if missing).

- [ ] **Step 5: Commit**

```bash
git add rust-toolchain.toml .github/workflows/ci.yml
git commit -m "chore: pin toolchain to 1.95.0 for reproducible fmt/clippy"
```

---

### Task 2: One-time formatting commit (now safe)

With the toolchain pinned, a full-format commit no longer risks version-drift churn, and it makes `cargo fmt --all -- --check` green permanently. This must be an isolated, format-only commit.

**Files:**
- Modify: whatever `cargo fmt` touches (expect ~19 files incl. `crates/dtx-scoring/src/store.rs`, `crates/game-menu/src/song_select.rs`, `crates/gameplay-drums/**`)

- [ ] **Step 1: Confirm tree is clean**

Run: `git status --porcelain`
Expected: empty output. If not empty, STOP — commit or stash first.

- [ ] **Step 2: Format everything with the pinned toolchain**

Run: `cargo fmt --all`
Then: `cargo fmt --all -- --check`
Expected: second command exits 0 with no output.

- [ ] **Step 3: Sanity-compile**

Run: `cargo check --workspace -j 2`
Expected: success (formatting must not change semantics; this catches accidental damage).

- [ ] **Step 4: Commit format-only**

```bash
git add -u
git commit -m "style: apply pinned rustfmt across workspace (format-only)"
```

---

### Task 3: Fix dtx-audio clippy under -D warnings

**Files:**
- Modify: `crates/dtx-audio/src/lib.rs`, `crates/dtx-audio/src/preview.rs`, `crates/dtx-audio/src/crossfade.rs` (as clippy dictates)

- [ ] **Step 1: Capture the actual failure list**

Run: `cargo clippy -p dtx-audio --all-targets -- -D warnings 2>&1 | grep -E '^(error|warning)' | sort | uniq -c`
Expected: non-empty list of lints (roadmap asserts failures exist). Record each lint name + location.

- [ ] **Step 2: Fix each finding with the smallest justified change**

Rules:
- Prefer the code change clippy suggests (`cargo clippy --fix -p dtx-audio --allow-dirty` is acceptable for mechanical lints, then review the diff hunk-by-hunk).
- A targeted `#[allow(clippy::lint_name)]` with a one-line justification comment is acceptable ONLY when the suggested rewrite would hurt clarity (e.g. intentional `as` truncation of a clamped ms value). No crate-level or module-level blanket allows — the crate currently has zero and must stay that way.
- Likely hot spots from static scan: `as u64`/`as u32` casts in `crossfade.rs:51,70,107` (cast_possible_truncation-family), `.unwrap()`/`.clone()` density in `preview.rs`.

- [ ] **Step 3: Verify the whole workspace still passes clippy**

Run: `cargo clippy --workspace --all-targets -j 2 -- -D warnings`
Expected: exit 0. (CI runs workspace-wide; fixing dtx-audio must not merely move the failure.)

- [ ] **Step 4: Run dtx-audio tests**

Run: `cargo test -p dtx-audio -j 2`
Expected: PASS (behavior unchanged).

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-audio
git commit -m "fix: resolve clippy -D warnings findings in dtx-audio"
```

---

### Task 4: CI actually runs tests, in low-memory package groups

Workspace-wide `cargo test` OOMs on 7 GB runners (linking many bevy binaries concurrently). Split into a matrix of package groups, `-j 2`, heaviest crate isolated.

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Rename the existing job honestly and add a test matrix job**

Replace the full `ci.yml` `jobs:` section with:

```yaml
jobs:
  check:
    name: fmt + clippy
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@1.95.0
        with:
          components: rustfmt, clippy

      - name: Install system dependencies
        run: sudo apt-get update && sudo apt-get install -y --no-install-recommends libwayland-dev libxkbcommon-dev libx11-dev libasound2-dev libudev-dev

      - uses: Swatinem/rust-cache@v2

      - name: Format
        run: cargo fmt --all -- --check

      - name: Clippy
        run: cargo clippy --workspace --all-targets -j 2 -- -D warnings

  test:
    name: test (${{ matrix.group.name }})
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        group:
          - name: pure
            packages: "-p dtx-core -p dtx-config -p dtx-scoring -p dtx-layout -p dtx-cli"
          - name: engine
            packages: "-p dtx-timing -p dtx-audio -p dtx-input -p dtx-assets -p dtx-library -p dtx-bga -p dtx-ui"
          - name: game
            packages: "-p game-menu -p game-shell -p game-results -p gameplay-guitar -p dev-tools"
          - name: drums
            packages: "-p gameplay-drums"
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@1.95.0

      - name: Install system dependencies
        run: sudo apt-get update && sudo apt-get install -y --no-install-recommends libwayland-dev libxkbcommon-dev libx11-dev libasound2-dev libudev-dev

      - uses: Swatinem/rust-cache@v2
        with:
          key: test-${{ matrix.group.name }}

      - name: Test
        run: cargo test ${{ matrix.group.packages }} -j 2
```

Keep the existing `name:`, `on:`, and `env:` blocks (lines 1-10) unchanged. Delete the lines 42-45 comment about omitted tests — it is no longer true. `-j 2` bounds concurrent rustc/link processes, which is what OOMed before; isolating `gameplay-drums` (13 integration-test binaries, all linking bevy) keeps peak RSS per job down. `fail-fast: false` so one group's failure still reports the others.

- [ ] **Step 2: Validate workflow syntax locally**

Run: `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/ci.yml')); print('ok')"`
Expected: `ok`.

- [ ] **Step 3: Verify the group partition covers every testable package exactly once**

Run:
```bash
cargo metadata --no-deps --format-version 1 | python3 -c "
import json,sys
names = sorted(p['name'] for p in json.load(sys.stdin)['packages'])
listed = set('dtx-core dtx-config dtx-scoring dtx-layout dtx-cli dtx-timing dtx-audio dtx-input dtx-assets dtx-library dtx-bga dtx-ui game-menu game-shell game-results gameplay-guitar dev-tools gameplay-drums'.split())
print('missing from CI groups:', [n for n in names if n not in listed])
"
```
Expected: `missing from CI groups: ['dtxmaniars-desktop', 'xtask']` — acceptable: the desktop app has no tests worth a 7 GB link on CI (it is covered by the release build in Task 7) and xtask is a stub. Anything ELSE in the missing list must be added to a group.

- [ ] **Step 4: Smoke-run the lightest group locally**

Run: `cargo test -p dtx-core -p dtx-config -p dtx-scoring -p dtx-layout -p dtx-cli -j 2`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: run tests in low-memory package groups with -j 2"
```

- [ ] **Step 6: Verify on CI**

Push (or open a PR) and confirm all five jobs (`fmt + clippy` + 4 test groups) go green. If a group is killed (exit 143), split it further — move `game-menu` into its own group first, it links bevy for its integration tests.

---

### Task 5: Repair AGENTS.md links and facts

Replace, don't restore: point every dead link at a file that exists, and correct stale claims. Do not write new architecture prose in this task.

**Files:**
- Modify: `AGENTS.md`
- Create: `docs/decisions/README.md`

- [ ] **Step 1: Create a decisions index that captures the ADRs cited by code**

`docs/decisions/README.md`:

```markdown
# Decision Records

Original ADR files predate this repo snapshot and were lost. The decisions
below are reconstructed from code comments and remain binding. Link new code
comments to this index.

- **ADR-0002 — Never judge on `Time::delta()`.** The gameplay clock free-runs
  and uses the BGM position only for drift correction, never as a gate.
  Implementation: `GameplayClock::tick` in `crates/gameplay-drums/src/resources.rs`,
  doc header of `crates/dtx-timing/src/lib.rs`.
- **ADR-0008 — Reference-first workflow.** Port behavior from
  `references/DTXmaniaNX-BocuD/` first; deviate only with a written reason.
- Other ADR numbers cited in code comments (0003, 0004, 0009, 0010, 0014,
  0015) refer to lost documents; when you touch code citing one, record the
  decision here from the surrounding comment.
```

- [ ] **Step 2: Fix AGENTS.md**

Apply these edits (grep for each string, exact lines have drifted):

1. Every `docs/decisions/00XX-*.md` link → `docs/decisions/README.md`.
2. `docs/ROADMAP.md` (2 occurrences) → `docs/superpowers/specs/2026-07-11-game-improvement-roadmap-design.md`.
3. `docs/ARCHITECTURE.md`, `docs/BEVY_PATTERNS.md`, `docs/UX_UI_DESIGN.md` — delete the links; if the sentence loses meaning, delete the sentence. Do not create empty stub docs.
4. Quickstart: `cargo run -p dtx-cli -- validate tests/fixtures/dtx-core/minimal.dtx` → `cargo run -p dtx-cli -- validate crates/dtx-core/tests/fixtures/minimal.dtx`, and note the binary is named `dtx`.
5. Layer table (lines ~60-67): add the missing crates — Pure: `dtx-layout`, `xtask`; Engine: `dtx-bga`, `gameplay-guitar`. Leave dtx-core/dtx-config classification as Pure (Tasks 6-7 make that true).
6. Add one line to the test-commands section: "CI runs tests in package groups with `-j 2` (see `.github/workflows/ci.yml`); never `cargo test --workspace` on <16 GB RAM."

- [ ] **Step 3: Verify no dead docs/ links remain**

Run:
```bash
grep -oE 'docs/[A-Za-z0-9_./-]+\.md' AGENTS.md | sort -u | while read f; do [ -f "$f" ] || echo "DEAD: $f"; done
```
Expected: no `DEAD:` lines.

- [ ] **Step 4: Commit**

```bash
git add AGENTS.md docs/decisions/README.md
git commit -m "docs: repair AGENTS.md links, quickstart paths, and layer table"
```

---

### Task 6: Remove layer violation A (dtx-core → dtx-timing)

Move the pure chart-time math down into `dtx-core`; `dtx-timing` re-exports it so all Engine call sites keep compiling unchanged.

**Files:**
- Create: `crates/dtx-core/src/chart_time.rs` (content = current `crates/dtx-timing/src/math.rs`, minus nothing)
- Modify: `crates/dtx-core/src/lib.rs`, `crates/dtx-core/src/cdtx_config.rs:12`, `crates/dtx-core/Cargo.toml`
- Modify: `crates/dtx-timing/src/lib.rs` (or `math.rs`), `crates/dtx-timing/Cargo.toml`

- [ ] **Step 1: Confirm the math module is pure**

Run: `grep -n 'use bevy\|use bevy_kira\|use dtx_audio' crates/dtx-timing/src/math.rs`
Expected: no matches. (If matches appear, STOP and re-scope — split only the pure functions instead.)
Note: if `math` is an inline `mod math { ... }` inside `crates/dtx-timing/src/lib.rs` rather than a separate file, the "move" below means cutting that module block; adjust paths accordingly.

- [ ] **Step 2: Move the module into dtx-core**

```bash
git mv crates/dtx-timing/src/math.rs crates/dtx-core/src/chart_time.rs   # file case
```

In `crates/dtx-core/src/lib.rs` add:

```rust
pub mod chart_time;
```

In dtx-timing, replace the old module declaration with a re-export so `dtx_timing::math::*` paths still work everywhere:

```rust
pub use dtx_core::chart_time as math;
```

- [ ] **Step 3: Fix the dependency edges**

- `crates/dtx-core/Cargo.toml`: delete the `dtx-timing = { workspace = true }` line.
- `crates/dtx-timing/Cargo.toml`: add `dtx-core = { workspace = true }` if not already present.
- `crates/dtx-core/src/cdtx_config.rs:12`: change
  `use dtx_timing::math::{chip_time_ms_with_bpm_changes, BpmChange};`
  to
  `use crate::chart_time::{chip_time_ms_with_bpm_changes, BpmChange};`

- [ ] **Step 4: Verify no dependency cycle and everything builds**

Run: `cargo check -p dtx-core -j 2 && cargo check -p dtx-timing -j 2 && cargo check --workspace -j 2`
Expected: success. dtx-core must build without bevy in its tree:

Run: `cargo tree -p dtx-core | grep -c bevy`
Expected: `0`.

- [ ] **Step 5: Run the moved tests**

The math unit tests move with the module. Run: `cargo test -p dtx-core -j 2 && cargo test -p dtx-timing -j 2`
Expected: PASS, including `chip_time_at_120bpm`, `bpm_change_*`, `single_scaled_measure_is_sticky` (now under dtx-core).

- [ ] **Step 6: Commit**

```bash
git add -A crates/dtx-core crates/dtx-timing
git commit -m "refactor: move pure chart-time math from dtx-timing into dtx-core"
```

---

### Task 7: Remove layer violation B (dtx-config → dtx-input)

`bindings.rs` serializes bevy's `KeyCode` — it is input-domain code living in a Pure crate. Move the whole module to `dtx-input`, keeping the serialized schema byte-identical (same types, same serde derives).

**Files:**
- Move: `crates/dtx-config/src/bindings.rs` → `crates/dtx-input/src/bindings.rs`
- Modify: `crates/dtx-config/src/lib.rs`, `crates/dtx-config/Cargo.toml`, `crates/dtx-input/src/lib.rs`, `crates/dtx-input/Cargo.toml`, all `dtx_config::bindings` call sites

- [ ] **Step 1: Inventory call sites**

Run: `grep -rn 'dtx_config::bindings\|use dtx_config::.*[Bb]ind' crates/ app/ tools/ --include='*.rs' | grep -v 'crates/dtx-config/'`
Record every hit — each needs its import updated in Step 3.

- [ ] **Step 2: Move the module**

```bash
git mv crates/dtx-config/src/bindings.rs crates/dtx-input/src/bindings.rs
```

- In `crates/dtx-config/src/lib.rs`: delete `pub mod bindings;` (and any `pub use bindings::...`; note what was re-exported).
- In `crates/dtx-input/src/lib.rs`: add `pub mod bindings;` plus the same re-exports under `dtx_input::`.
- In `crates/dtx-input/src/bindings.rs`: change `use dtx_input::KeyCode;` to `use crate::KeyCode;`. If the module used other `dtx_config` items (e.g. a shared path helper), import them from `dtx_config` — dtx-input may depend on dtx-config only if dtx-config is the lower layer; if that would create a cycle, copy the tiny helper instead and note it.
- `crates/dtx-config/Cargo.toml`: delete `dtx-input = { path = "../dtx-input" }`. Also delete a now-unused `serde` feature only if nothing else in dtx-config needs it (it does — config serde stays).
- `crates/dtx-input/Cargo.toml`: ensure `serde`, `toml`/`serde_json` (whichever bindings.rs uses — check its imports) are dependencies.

- [ ] **Step 3: Update call sites**

For every hit from Step 1: `dtx_config::bindings::X` → `dtx_input::bindings::X`. Crates that used it (expect `gameplay-drums`, possibly `game-shell`/app) must have `dtx-input` in their Cargo.toml (gameplay-drums already does).

- [ ] **Step 4: Prove the serialized schema is unchanged**

The moved module's own tests (`bindings.rs:363-436`, serde round-trips with `KeyCode` payloads) move with it and must pass unmodified — they are the schema regression test:

Run: `cargo test -p dtx-input -j 2 bindings`
Expected: PASS with zero test-body edits (import lines only).

- [ ] **Step 5: Verify purity and full build**

Run: `cargo tree -p dtx-config | grep -c bevy`
Expected: `0`.
Run: `cargo check --workspace -j 2 && cargo test -p dtx-config -j 2`
Expected: success.

- [ ] **Step 6: Commit**

```bash
git add -A crates/dtx-config crates/dtx-input crates/gameplay-drums
git commit -m "refactor: move key bindings from dtx-config into dtx-input"
```

---

### Task 8: One supported release target + versioned artifact

Supported target: `x86_64-unknown-linux-gnu` (the only target the team runs and CI exercises). Tag-triggered workflow builds `dtxmaniars` + `dtx`, packages a tarball, attaches it to a GitHub release. No platform matrix — that is explicit roadmap scope.

**Files:**
- Create: `.github/workflows/release.yml`
- Modify: `AGENTS.md` (one line documenting the supported target)

- [ ] **Step 1: Check what runtime files the binary needs**

Run: `ls assets/ 2>/dev/null || echo "no assets dir"` and `grep -rn 'AssetPlugin' app/dtxmaniars-desktop/src/main.rs`
The app sets `unapproved_path_mode: Allow` and loads chart assets by absolute path, but built-in UI assets (fonts, sounds) live under the Bevy asset root. If an `assets/` dir exists at repo root, it ships in the tarball; if not, drop it from the tar command below.

- [ ] **Step 2: Create the release workflow**

`.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags: ["v*"]

env:
  CARGO_TERM_COLOR: always

jobs:
  release:
    name: build + package (x86_64-unknown-linux-gnu)
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@1.95.0

      - name: Install system dependencies
        run: sudo apt-get update && sudo apt-get install -y --no-install-recommends libwayland-dev libxkbcommon-dev libx11-dev libasound2-dev libudev-dev

      - uses: Swatinem/rust-cache@v2
        with:
          key: release

      - name: Build release binaries
        run: cargo build --release -j 2 -p dtxmaniars-desktop -p dtx-cli

      - name: Package
        run: |
          version="${GITHUB_REF_NAME}"
          pkg="dtxmaniars-${version}-x86_64-unknown-linux-gnu"
          mkdir -p "dist/${pkg}"
          cp target/release/dtxmaniars target/release/dtx "dist/${pkg}/"
          [ -d assets ] && cp -r assets "dist/${pkg}/assets"
          cp README.md "dist/${pkg}/" 2>/dev/null || true
          tar -C dist -czf "dist/${pkg}.tar.gz" "${pkg}"
          sha256sum "dist/${pkg}.tar.gz" > "dist/${pkg}.tar.gz.sha256"

      - name: Create GitHub release
        uses: softprops/action-gh-release@v2
        with:
          files: |
            dist/*.tar.gz
            dist/*.tar.gz.sha256
          generate_release_notes: true
```

- [ ] **Step 3: Validate syntax**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml')); print('ok')"`
Expected: `ok`.

- [ ] **Step 4: Verify a local release build works**

Run: `cargo build --release -j 2 -p dtxmaniars-desktop -p dtx-cli && ls -la target/release/dtxmaniars target/release/dtx`
Expected: both binaries exist. (Slow — thin-LTO, codegen-units=1. Run it once; do not repeat per-step.)

- [ ] **Step 5: Document the supported target**

In AGENTS.md, add to the quickstart/build section:

```markdown
Supported release target: `x86_64-unknown-linux-gnu` only. Tag `vX.Y.Z` to
produce a release tarball via `.github/workflows/release.yml`.
```

- [ ] **Step 6: Commit and cut the first tag**

```bash
git add .github/workflows/release.yml AGENTS.md
git commit -m "ci: add tag-triggered release packaging for x86_64-linux"
```

Then (after the branch merges to main, with the user's go-ahead — tags are outward-facing):

```bash
git tag v0.1.0 && git push origin v0.1.0
```

Verify the Release workflow goes green and the tarball appears on the GitHub release page.

---

## Verification (whole plan)

1. `cargo fmt --all -- --check` — exit 0.
2. `cargo clippy --workspace --all-targets -j 2 -- -D warnings` — exit 0.
3. CI shows 5 green jobs including 4 test groups.
4. `cargo tree -p dtx-core | grep -c bevy` and `cargo tree -p dtx-config | grep -c bevy` — both `0`.
5. AGENTS.md dead-link grep (Task 5 Step 3) — clean.
6. Tag build produces `dtxmaniars-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` with sha256.

## Ordering / dependencies

Task 1 → Task 2 (pin before mass-format). Tasks 3-8 are independent of each other but all assume Task 2's green fmt baseline. Task 8 Step 6 (tagging) requires user confirmation.
