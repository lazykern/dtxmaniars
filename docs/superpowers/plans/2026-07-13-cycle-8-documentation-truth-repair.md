# Cycle 8 Documentation and Repository Truth Repair Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make player docs, contributor rules, ADRs, reference citations, crate handbooks, compatibility claims, tests, and executable repository state agree, with a deterministic local checker that prevents recurrence.

**Architecture:** Stable canonical documents replace dated notes as the current entry points, while historical notes remain evidence with dated corrections. Individual ADRs reconstruct only decisions supported by code/reference history. A small pure Rust `docs-check` package validates local links, repository paths, stale reference roots, and canonical-document relationships without network access or writes.

**Tech Stack:** Rust 1.95+, standard library, `regex`, `walkdir`, Cargo metadata/commands, Markdown, and the existing Rust workspace.

## Global Constraints

- CI/CD configuration and workflows are explicitly excluded.
- The live read-only reference root is `references/DTXmaniaNX/`.
- Resulting reference paths and cited line ranges must exist and support the stated behavior.
- Files inside `references/` are never modified.
- Dated notes/specs/plans remain historical evidence; false repository-state claims receive dated correction blocks.
- ADR reconstruction requires code, comment, commit, or reference evidence; unproven records stay `Unreconstructed`.
- The binding transition decision is 300 ms OutQuint; NX's 1500 ms linear snapshot remains comparison evidence only.
- Canonical compatibility wording is Supported, Degraded with Warning, and Rejected with Recovery.
- Documentation commands must be executed or mechanically validated; optional hardware/GUI checks are labelled manual.
- Test cleanup replaces assertions with observable contracts or removes tests that prove nothing; it is not a coverage-percentage exercise.
- `docs-check` is deterministic, network-free, read-only, and reports file, line, target, and reason.

---

## File structure

- Create `tools/docs-check/Cargo.toml`, `src/lib.rs`, and `src/main.rs`: local documentation validation.
- Create `tools/docs-check/tests/fixtures/`: broken-link, stale-root, and canonical-map fixtures.
- Create `docs/notes/2026-07-13-documentation-inventory.md`: generated baseline and repair ledger.
- Create `docs/roadmap.md`, `docs/player-guide.md`, `docs/data-and-persistence.md`, and `docs/contributing.md`.
- Modify `README.md`, `PRODUCT.md`, `AGENTS.md`, `docs/compatibility.md`, and `docs/decisions/README.md`.
- Create `docs/decisions/0001-drums-first-product-scope.md`.
- Create `docs/decisions/0002-gameplay-audio-clock-authority.md`.
- Create `docs/decisions/0003-read-only-reference-inputs.md`.
- Create `docs/decisions/0004-reference-first-mechanics-workflow.md`.
- Create `docs/decisions/0005-crate-layering.md`.
- Create `docs/decisions/0009-input-profiles-source-of-truth.md`.
- Create `docs/decisions/0010-port-mechanics-redesign-ux.md`.
- Create `docs/decisions/0014-outquint-screen-transitions.md`.
- Create `docs/decisions/0015-preview-crossfade-ownership.md`.
- Create `docs/decisions/0016-qualified-score-persistence.md`.
- Modify every non-reference file named by `rg -l 'DTXmaniaNX[-]BocuD' --glob '!references/**'` and record the exact list in the inventory.
- Modify the eleven existing crate `AGENTS.md` files under `crates/`.
- Modify behaviorless test files identified by the inventory, initially including `crates/dtx-core/tests/comprehensive.rs`, `crates/dtx-scoring/tests/comprehensive.rs`, `crates/dtx-scoring/tests/edge_cases.rs`, and `crates/dtx-ui/src/widget/pad_chips.rs` if Cycle 6 has not already replaced its placeholder.

### Task 1: Build the deterministic local documentation checker

**Files:**
- Create: `tools/docs-check/Cargo.toml`
- Create: `tools/docs-check/src/lib.rs`
- Create: `tools/docs-check/src/main.rs`
- Create: `tools/docs-check/tests/fixtures/valid/README.md`
- Create: `tools/docs-check/tests/fixtures/broken-link/README.md`
- Create: `tools/docs-check/tests/fixtures/stale-root/README.md`
- Create: `tools/docs-check/tests/fixtures/missing-canonical/README.md`
- Create: `tools/docs-check/tests/fixtures/false-missing/README.md`
- Modify: `Cargo.lock`

**Interfaces:**
- Consumes: a repository root path and local filesystem contents.
- Produces: `CheckFailure`, `CheckOptions`, `check_repository`, and a `docs-check` binary returning nonzero when failures exist.

- [ ] **Step 1: Write failing library tests for every checker contract**

```rust
#[test]
fn valid_fixture_has_no_failures() {
    assert!(check_repository(&fixture("valid"), CheckOptions::fixture()).unwrap().is_empty());
}

#[test]
fn failures_include_file_line_target_and_reason() {
    let failures = check_repository(&fixture("broken-link"), CheckOptions::fixture()).unwrap();
    assert_eq!(failures[0].file, PathBuf::from("README.md"));
    assert_eq!(failures[0].line, 3);
    assert_eq!(failures[0].target, "missing.md");
    assert_eq!(failures[0].reason, FailureReason::MissingLocalTarget);
}

#[test]
fn stale_reference_path_and_missing_canonical_doc_fail() {
    let stale = check_repository(&fixture("stale-root"), CheckOptions::fixture()).unwrap();
    assert!(stale.iter().any(|f| f.reason == FailureReason::ObsoleteReferenceRoot));
    let missing = check_repository(&fixture("missing-canonical"), CheckOptions::repository()).unwrap();
    assert!(missing.iter().any(|f| f.reason == FailureReason::MissingCanonicalDocument));
    let false_claim = check_repository(&fixture("false-missing"), CheckOptions::fixture()).unwrap();
    assert!(false_claim.iter().any(|f| f.reason == FailureReason::FalseMissingCanonicalClaim));
}
```

- [ ] **Step 2: Verify failure**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p docs-check`

Expected: FAIL because the package and checker do not exist.

- [ ] **Step 3: Implement focused read-only checks**

Use this package manifest:

```toml
[package]
name = "docs-check"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[lints]
workspace = true

[dependencies]
regex = "1"
walkdir = "2"
```

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckFailure {
    pub file: PathBuf,
    pub line: usize,
    pub target: String,
    pub reason: FailureReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureReason {
    MissingLocalTarget,
    ObsoleteReferenceRoot,
    ReferenceEscapesRoot,
    MissingCanonicalDocument,
    CanonicalDocumentNotLinked,
    FalseMissingCanonicalClaim,
}

#[derive(Debug, Clone)]
pub struct CheckOptions { pub enforce_canonical_map: bool }

impl CheckOptions {
    pub fn fixture() -> Self { Self { enforce_canonical_map: false } }
    pub fn repository() -> Self { Self { enforce_canonical_map: true } }
}

pub fn check_repository(root: &Path, options: CheckOptions) -> io::Result<Vec<CheckFailure>> {
    let files = discover_checked_files(root, &options)?;
    let mut failures = Vec::new();
    for file in files { check_file(root, &file, &options, &mut failures)?; }
    check_canonical_map(root, &options, &mut failures);
    failures.sort_by(|a, b| (&a.file, a.line, &a.target).cmp(&(&b.file, b.line, &b.target)));
    Ok(failures)
}
```

Use `walkdir` to discover Markdown, Rust comments/string path citations, TOML, and crate handbooks while skipping `.git`, `target`, and `references`. Use `regex` for Markdown links and repository-relative path tokens; strip anchors before filesystem checks. Check that reference paths remain under the reference root. Construct the obsolete-token detector from fragments so the checker source does not trigger itself; allow only the Cycle 8 design's historical explanation and the stale-root negative fixture. Also detect current documentation that claims a canonical file is missing. Do not perform HTTP requests or write files.

- [ ] **Step 4: Implement CLI output and verify exit behavior**

The binary accepts an optional root argument defaulting to the current directory. Print one line per failure as `file:line: target: reason`; print a checked-file/failed-count summary; return `ExitCode::FAILURE` for any finding or traversal error.

Run: `cargo fmt --all && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p docs-check`

Expected: PASS for valid and negative fixtures.

```bash
git add tools/docs-check Cargo.lock
git commit -m "feat: add local documentation truth checker"
```

### Task 2: Inventory the repository and repair every stale reference path

**Files:**
- Create: `docs/notes/2026-07-13-documentation-inventory.md`
- Modify: every file listed by `rg -l 'DTXmaniaNX[-]BocuD' --glob '!references/**'`

**Interfaces:**
- Consumes: `docs-check`, the actual `references/DTXmaniaNX/` tree, and all non-reference stale-token matches.
- Produces: an exact repair ledger with old citation, resulting target, line validation, and disposition.

- [ ] **Step 1: Capture the exact baseline inventory**

Run: `rg -n 'DTXmaniaNX[-]BocuD' --glob '!references/**'`

Expected: every live/historical occurrence is visible; the observed pre-repair baseline is 81 affected non-reference files. Record each path in the inventory under Source comments, crate handbooks, plans/specs/notes, tests, or tooling.

- [ ] **Step 2: Repair live paths in small reviewed batches**

For each inventory row, replace the root only after confirming the resulting file exists. If a filename moved, locate the intended target with `rg --files references/DTXmaniaNX | rg '<basename>'`. Re-open cited line ranges and update them when the claimed behavior moved. Plans/specs retain their historical wording but get navigable paths. Notes that claimed the references were absent receive:

```markdown
> Correction (2026-07-13): The vendored reference is present at
> `references/DTXmaniaNX/`. The original observation below described the
> repository state visible during that earlier audit.
```

- [ ] **Step 3: Verify references without modifying the vendored tree**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo run -p docs-check`

Expected: no missing reference targets, escaped reference paths, or obsolete live paths. The only quoted obsolete token is the allowlisted design explanation and negative fixture.

Run: `git status --short references`

Expected: no output.

- [ ] **Step 4: Commit the mechanical/reference-evidence repair**

```bash
git add AGENTS.md crates app tools docs README.md PRODUCT.md
git commit -m "docs: repair vendored reference citations"
```

### Task 3: Create the stable roadmap and canonical document skeleton

**Files:**
- Create: `docs/roadmap.md`
- Create: `docs/player-guide.md`
- Create: `docs/data-and-persistence.md`
- Create: `docs/contributing.md`
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `docs/notes/2026-07-13-game-improvement-program.md`

**Interfaces:**
- Consumes: approved Cycle 0–8 specs/plans and the current implementation ledger.
- Produces: stable entry points and removal of the missing roadmap dependency.

- [ ] **Step 1: Write failing canonical-map tests in docs-check**

```rust
#[test]
fn canonical_docs_exist_and_entry_points_link_to_them() {
    let failures = check_repository(&repo_fixture(), CheckOptions::repository()).unwrap();
    assert!(!failures.iter().any(|f| matches!(f.reason,
        FailureReason::MissingCanonicalDocument | FailureReason::CanonicalDocumentNotLinked)));
}
```

Expected before document creation: FAIL for roadmap, player guide, data/persistence, and contributing.

- [ ] **Step 2: Create `docs/roadmap.md` as the current program index**

Include Cycle 0–8 status, links to every approved design/plan, cross-cycle acceptance, and the rule that the program becomes complete only after Cycle 8 verification. Do not duplicate task-level implementation instructions. Make README and root AGENTS link to this file instead of the missing `2026-07-11-game-improvement-roadmap-design.md`.

- [ ] **Step 3: Create canonical-document outlines with explicit ownership**

Each document begins with purpose, audience, maintained status, and links to neighboring canonical docs. `player-guide.md` owns controls/workflows, `data-and-persistence.md` owns paths/schemas/recovery, and `contributing.md` owns architecture/reference/local gates. Populate verified content in Task 5; do not publish unsupported claims in the skeleton.

- [ ] **Step 4: Verify links and commit**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p docs-check && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo run -p docs-check`

Expected: canonical files exist and link to one another; the missing roadmap claim is gone.

```bash
git add docs/roadmap.md docs/player-guide.md docs/data-and-persistence.md docs/contributing.md README.md AGENTS.md docs/notes/2026-07-13-game-improvement-program.md
git commit -m "docs: establish canonical repository guides"
```

### Task 4: Reconstruct evidence-backed ADRs and resolve contradictions

**Files:**
- Create: `docs/decisions/0001-drums-first-product-scope.md`
- Create: `docs/decisions/0002-gameplay-audio-clock-authority.md`
- Create: `docs/decisions/0003-read-only-reference-inputs.md`
- Create: `docs/decisions/0004-reference-first-mechanics-workflow.md`
- Create: `docs/decisions/0005-crate-layering.md`
- Create: `docs/decisions/0009-input-profiles-source-of-truth.md`
- Create: `docs/decisions/0010-port-mechanics-redesign-ux.md`
- Create: `docs/decisions/0014-outquint-screen-transitions.md`
- Create: `docs/decisions/0015-preview-crossfade-ownership.md`
- Create: `docs/decisions/0016-qualified-score-persistence.md`
- Modify: `docs/decisions/README.md`
- Modify: `crates/game-shell/AGENTS.md`

**Interfaces:**
- Consumes: current runtime/source behavior, reference evidence, code comments, and approved designs.
- Produces: one binding record per proven decision and explicit Unreconstructed index entries for unsupported ADR-0007/ADR-0011 history.

- [ ] **Step 1: Verify the evidence set before writing each record**

Use these binding sources:

- ADR-0001: `PRODUCT.md`, `game_shell::EGameMode`, and the approved drums scope.
- ADR-0002: `GameplayClock::tick` and `crates/dtx-timing/src/lib.rs`.
- ADR-0003/0004: root AGENTS reference policy and read-only vendored tree.
- ADR-0005: root Cargo members/dependencies and documented layer boundaries.
- ADR-0009: input/lane profile registries, migrations, and Customize transactions.
- ADR-0010: mechanics citations plus independent UI design specs.
- ADR-0014: `SCREEN_TRANSITION_MS = 300`, OutQuint transition code, and UX audit.
- ADR-0015: preview/crossfade source ownership and per-chart fallback.
- ADR-0016: speed, practice, No Fail, and future modifier qualification gates.

- [ ] **Step 2: Write each ADR with the same complete schema**

```markdown
# ADR-NNNN — Title

Status: Accepted (reconstructed 2026-07-13)

## Context
## Decision
## Evidence
## Consequences
## Supersedes / Superseded By
```

Do not invent an original date. Link exact source/reference paths in Evidence. Index ADR-0007 and ADR-0011 as `Unreconstructed` unless independent evidence establishes their original decisions.

- [ ] **Step 3: Resolve the transition contradiction**

Rewrite `crates/game-shell/AGENTS.md` so it states:

```markdown
- Reference comparison: DTXManiaNX uses a 1500 ms linear snapshot transition.
- Product decision: DTXManiaRS uses a 300 ms OutQuint overlay.
- Boundary: stage mechanics remain reference-first; transition UX follows ADR-0014.
```

- [ ] **Step 4: Verify ADR citations and commit**

Run: `rg -o 'ADR-[0-9]{4}' --glob '!references/**' | sort -u`

Expected: every accepted cited number links from the index; unreconstructed numbers are explicitly labelled and not described as binding.

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo run -p docs-check`

Expected: PASS.

```bash
git add docs/decisions crates/game-shell/AGENTS.md
git commit -m "docs: reconstruct binding architecture decisions"
```

### Task 5: Write verified player, compatibility, data, and contributor documentation

**Files:**
- Modify: `README.md`
- Modify: `PRODUCT.md`
- Modify: `docs/player-guide.md`
- Modify: `docs/compatibility.md`
- Modify: `docs/data-and-persistence.md`
- Modify: `docs/contributing.md`

**Interfaces:**
- Consumes: Cargo metadata, desktop/config/library/input/practice/results behavior, Cycle 6 accessibility, and Cycle 7 executable compatibility matrix.
- Produces: a player/contributor path whose claims and commands are locally reproducible.

- [ ] **Step 1: Verify every command and platform prerequisite before documenting it**

Run: `cargo metadata --no-deps --format-version 1`

Run: `cargo install --path app/dtxmaniars-desktop --bin dtxmaniars --locked --root /tmp/dtxmaniars-doc-install`

Run: `cargo build --release -p dtxmaniars-desktop`

Expected: commands resolve to real package/binary names. Confirm FFmpeg/video and audio prerequisites from Cargo features/build errors or upstream crate documentation already vendored/locked; list only verified operating systems.

- [ ] **Step 2: Expand README as the shortest successful path**

Cover Rust 1.95+, prerequisites, install/run/release commands, song root, archive import, chart/media summary, keyboard/MIDI setup, calibration, normal play/practice/results, score qualification, data locations, common recovery, contributor quickstart, and links to detailed canonical docs. Keep details in their owned document.

- [ ] **Step 3: Complete maintained detailed guides**

`player-guide.md` covers search/sort/discovery, input profiles/system binds, calibration, pause, practice transport, results analysis, and recommended loops. `compatibility.md` mirrors Cycle 7 matrix outcomes and exact support vocabulary. `data-and-persistence.md` documents config, score store, score.ini, library preferences, input/lane profiles, layout, version fields, atomicity actually present, backup, and safe deletion/recovery. `contributing.md` covers layers, reference-first evidence, package/test selection, formatting/check/Clippy gates, and read-only references.

- [ ] **Step 4: Update PRODUCT accessibility commitments and verify docs**

Replace the obsolete “No additional accessibility requirements” statement with the implemented independent controls, distance readability, non-color state signals, and assisted-run transparency. Do not promise platform/accessibility behavior that was not verified.

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo run -p docs-check`

Expected: PASS.

```bash
git add README.md PRODUCT.md docs/player-guide.md docs/compatibility.md docs/data-and-persistence.md docs/contributing.md
git commit -m "docs: publish verified player and contributor guides"
```

### Task 6: Refresh every crate handbook against current code

**Files:**
- Modify: `crates/dtx-assets/AGENTS.md`
- Modify: `crates/dtx-audio/AGENTS.md`
- Modify: `crates/dtx-bga/AGENTS.md`
- Modify: `crates/dtx-core/AGENTS.md`
- Modify: `crates/dtx-input/AGENTS.md`
- Modify: `crates/dtx-library/AGENTS.md`
- Modify: `crates/dtx-timing/AGENTS.md`
- Modify: `crates/game-results/AGENTS.md`
- Modify: `crates/game-shell/AGENTS.md`
- Modify: `crates/gameplay-drums/AGENTS.md`
- Modify: `crates/gameplay-guitar/AGENTS.md`

**Interfaces:**
- Consumes: root AGENTS, Cargo manifests, current public modules/types, ADRs, and package test targets.
- Produces: local-only scope/boundary/reference/test guidance for each documented crate.

- [ ] **Step 1: Audit each handbook against Cargo and source**

For each file, compare package description, dependencies, exported modules, implemented/deferred support, references, ownership boundaries, and listed test commands. Remove milestone-era claims such as “M6+” when the feature now exists. Do not repeat root-wide rules.

- [ ] **Step 2: Replace stale commands and types with verified ones**

Run every package command named by a handbook using the smallest safe target. Commands must name real packages and integration-test binaries. Cite actual reference paths and the ADR that owns cross-crate behavior.

- [ ] **Step 3: Verify handbook-local links and commit**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo run -p docs-check`

Expected: every handbook path resolves; no handbook contradicts root layers, transition policy, compatibility, or support status.

```bash
git add crates/*/AGENTS.md
git commit -m "docs: refresh crate ownership handbooks"
```

### Task 7: Replace or remove behaviorless tests

**Files:**
- Modify: `crates/dtx-core/tests/comprehensive.rs`
- Modify: `crates/dtx-scoring/tests/comprehensive.rs`
- Modify: `crates/dtx-scoring/tests/edge_cases.rs`
- Modify: `crates/dtx-ui/src/widget/pad_chips.rs` if its placeholder survives Cycle 6
- Modify: every additional test path recorded by the suspect-test inventory.

**Interfaces:**
- Consumes: public behavior contracts and the inventory's suspect assertions.
- Produces: tests that fail on an observable regression or removal of tests with no contract.

- [ ] **Step 1: Generate and record the suspect-test inventory**

Run:

```bash
rg -n --pcre2 'assert_eq!\(([^,;]+),\s*\1\s*\)|assert!\([^)]*\|\|[^)]*\)|#\[ignore|placeholder' crates app tools --glob '*.rs'
```

Classify each match as behavior-bearing, compile-surface contract, or behaviorless. Record retained compile-surface cases with their protected public API.

- [ ] **Step 2: Replace the known tautologies with observable assertions**

- Replace `wav_cache.is_empty() || wav_cache.is_empty()` by asserting the expected slot/path mapping for `with_bgm.dtx`, or remove it if the cache is not part of the public contract and an asset-resolution test already covers the behavior.
- Remove `JudgmentKind::Perfect == JudgmentKind::Perfect` and `Rank::S == Rank::S`; retain boundary classification, display, ordering, hashing, and score behavior tests that can fail independently.
- Remove the `assert_eq!(5, 5)` pad test. If Cycle 6 has not implemented pad feedback, replace the placeholder primitive with a pure `PadFlashState` reducer and assert trigger/120 ms decay/reduced-flash outline behavior; otherwise rely on Cycle 6's behavior tests.

- [ ] **Step 3: Run changed-package tests and repeat the scan**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-core --test comprehensive && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-scoring --tests && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-ui`

Expected: PASS.

Repeat the suspect scan. Expected: no behaviorless assertion remains; every retained match is listed with a public API contract.

- [ ] **Step 4: Commit the isolated test-quality repair**

```bash
git add crates/dtx-core/tests/comprehensive.rs crates/dtx-scoring/tests crates/dtx-ui/src/widget/pad_chips.rs docs/notes/2026-07-13-documentation-inventory.md
git commit -m "test: replace behaviorless assertions"
```

### Task 8: Run final truth gates and close the improvement program

**Files:**
- Modify: `docs/roadmap.md`
- Modify: `docs/notes/2026-07-13-game-improvement-program.md`
- Modify: `docs/notes/2026-07-13-documentation-inventory.md`

- [ ] **Step 1: Run documentation and command gates**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p docs-check && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo run -p docs-check`

Expected: PASS with zero broken local links, false paths, stale live roots, missing canonical docs, or canonical-link failures.

Execute every safe command documented in README, contributing, and crate handbooks. Mechanically validate commands requiring MIDI hardware, audio output, or GUI interaction and label them manual.

- [ ] **Step 2: Run formatting, package, and workspace gates**

Run: `cargo fmt --all -- --check && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo check --workspace && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo clippy --workspace --all-targets -- -D warnings && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test --workspace --lib`

Expected: PASS.

- [ ] **Step 3: Audit cross-cycle acceptance and repository scope**

Verify each cross-cycle acceptance item against its tests/manual evidence. Run `git diff --check`, `git status --short`, and `git status --short references`. Confirm no CI/CD file changed and the reference tree is clean.

- [ ] **Step 4: Mark Cycle 8 and the program complete only after all evidence passes**

Update roadmap/program/inventory with commands, counts, manual checks, and any documented optional limitations. If any gate fails, leave the cycle In Progress and record the exact failure instead of declaring completion.

```bash
git add docs/roadmap.md docs/notes/2026-07-13-game-improvement-program.md docs/notes/2026-07-13-documentation-inventory.md
git commit -m "docs: close verified game improvement program"
```

## Plan self-review

The eight tasks cover the canonical document map, missing roadmap, all stale reference roots, path/line validation, historical corrections, evidence-backed ADR reconstruction, the 300 ms transition contradiction, verified README/player/compatibility/data/contributor content, every crate handbook, deterministic local checking, known and newly inventoried behaviorless tests, local release gates, and program closure. The checker is introduced before bulk repair and canonical docs; ADRs precede handbook updates; executable compatibility remains the source for published claims. References, unsupported claims, network checks, coverage targets, unrelated refactors, and CI/CD are outside scope.
