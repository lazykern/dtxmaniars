# Fast Build and Worktree Workflow Design

## Goal

Keep Bevy edit/build/test feedback fast while multiple git worktrees build concurrently. Reduce disk growth without weakening release verification.

## Diagnosis

- `$HOME/.cargo/config.toml` sends every project and worktree to one shared target directory: `$HOME/.cache/cargo-target`.
- Current shared target uses about 140 GiB. Roughly 109 GiB is test executables; Bevy-heavy integration tests are often near 1 GiB each when statically linked with full debug information.
- Concurrent Cargo processes sharing one target directory can wait on build-directory locks.
- Changing `rustflags`, linker, features, profile, or toolchain creates new artifact variants. The current uncommitted BFD-to-mold change therefore causes a broad rebuild.
- Existing Bevy dev optimization settings already match Bevy 0.19 guidance.

## Design

### Cache isolation

Use Cargo's default per-worktree `target/` directory. Do not set one global `build.target-dir` for concurrent worktrees.

Use local `sccache` as the cross-worktree compilation cache. Configure `SCCACHE_BASEDIRS` with the common worktree parent so absolute checkout paths normalize. Keep Cargo incremental compilation enabled: edited workspace crates retain local incremental builds, while non-incremental dependencies can use sccache.

```text
worktree A/target ─┐
worktree B/target ─┼── local sccache
worktree C/target ─┘
```

Removing a completed worktree also removes its isolated build artifacts. Cache cleanup no longer depends on manually pruning one unbounded global target directory.

### Build profiles

Keep existing Bevy runtime optimization:

```toml
[profile.dev]
opt-level = 1
debug = "line-tables-only"

[profile.dev.package."*"]
opt-level = 3
debug = false

[profile.debugging]
inherits = "dev"
debug = true
```

Normal builds retain line tables for panic backtraces but avoid large dependency debug data. Full debugger information remains available through `cargo build --profile debugging`.

### Inner development loop

Run only changed package and smallest relevant test target:

```sh
cargo check -p <package>
cargo test -p <package> --lib
cargo test -p <package> --test <integration-test>
```

For Bevy application runs and Bevy-heavy tests, enable dynamic linking only on command line:

```sh
cargo run -p dtxmaniars-desktop --features bevy/dynamic_linking
cargo test -p gameplay-drums --features bevy/dynamic_linking --test <integration-test>
```

Use installed `cargo-nextest` when running multiple test binaries. It improves execution scheduling but does not reduce compilation cost:

```sh
cargo nextest run -p <package>
```

### Verification tiers

1. During edits: package-scoped `cargo check` and one relevant test target.
2. Before handing off a feature: all tests for changed packages.
3. Before merge: `cargo check --workspace`, strict workspace clippy, then required workspace/package tests.
4. If parallel linking exhausts memory, run the full gate with `CARGO_BUILD_JOBS=1`; do not constrain every inner-loop command by default.
5. Release builds omit `bevy/dynamic_linking`, sccache assumptions, and dev-only profiles.

### Linker policy

Do not switch linkers casually because any `rustflags` change invalidates build artifacts. Benchmark current committed BFD and local mold using the same representative build before changing tracked config. Keep mold only if it passes existing plugin-heavy builds and materially improves link time.

### Documentation

Update root `AGENTS.md` with:

- package-scoped inner-loop commands;
- dev-only dynamic-linking commands;
- verification tiers;
- per-worktree target and sccache policy;
- warning that linker/rustflag changes invalidate caches;
- `CARGO_BUILD_JOBS=1` fallback for full workspace gates.

## Migration

1. Add reduced-debug profile settings and debugger profile.
2. Update `AGENTS.md`.
3. Install/configure sccache locally.
4. Remove global `build.target-dir` after active Cargo processes stop.
5. Rebuild once in each active worktree as needed.
6. After explicit confirmation, delete the old 140 GiB shared target and stale nested target directory.
7. Benchmark mold versus committed BFD separately; preserve current uncommitted linker change until resolved.

## Non-goals

- No nightly Cranelift configuration initially.
- No new task runner or wrapper scripts.
- No permanent `dynamic_linking` Cargo feature.
- No automatic destructive cache cleanup.
- No workspace-wide test run after every edit.

## Success Criteria

- Concurrent worktrees no longer contend for one Cargo build directory.
- Bevy integration-test binaries and link time shrink through reduced debug info and optional dynamic linking.
- New worktrees reuse dependency compilations through sccache.
- Inner-loop commands target changed packages; full validation remains available before merge.
- Release artifacts remain statically linked and independently runnable.
