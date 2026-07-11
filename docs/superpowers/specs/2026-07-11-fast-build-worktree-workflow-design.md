# Fast Build and Worktree Workflow Design

## Goal

Keep Bevy edit/build/test feedback fast across git worktrees without multiplying cold builds or disk use. Reduce artifact growth without weakening release verification.

## Diagnosis

- `$HOME/.cargo/config.toml` sends every project and worktree to one shared target directory: `$HOME/.cache/cargo-target`.
- Current shared target uses about 140 GiB. Roughly 109 GiB is test executables; Bevy-heavy integration tests are often near 1 GiB each when statically linked with full debug information.
- Concurrent Cargo processes sharing one target directory can wait on build-directory locks.
- Changing `rustflags`, linker, features, profile, or toolchain creates new artifact variants. The current uncommitted BFD-to-mold change therefore causes a broad rebuild.
- Existing Bevy dev optimization settings already match Bevy 0.19 guidance.

## Design

### Cache strategy

Keep the existing shared Cargo target directory. A measured isolated-target trial produced a 33 GiB target in one Bevy worktree and forced cold dependency builds; repeating that per active worktree costs more time and disk than shared-target lock contention.

Use local `sccache` as secondary reuse across cleans and compatible configuration variants. Configure `SCCACHE_BASEDIRS` with the common worktree parent so absolute checkout paths normalize. Keep Cargo incremental compilation enabled.

```text
worktree A ─┐
worktree B ─┼── shared Cargo target ── sccache fallback
worktree C ─┘
```

Parallelize editing and small package checks. Serialize Bevy-heavy builds/tests and workspace-wide gates so concurrent jobs do not compete for the shared target, RAM, and linker. Shared-target cleanup remains explicit maintenance, never an automatic worktree-removal side effect.

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
- shared-target, serialized-heavy-build, and sccache policy;
- warning that linker/rustflag changes invalidate caches;
- `CARGO_BUILD_JOBS=1` fallback for full workspace gates.

## Migration

1. Add reduced-debug profile settings and debugger profile.
2. Update `AGENTS.md`.
3. Install/configure sccache locally while retaining the shared `build.target-dir`.
4. Serialize Bevy-heavy and workspace-wide Cargo commands across worktrees.
5. After explicit confirmation, selectively remove obsolete large test artifacts and abandoned isolated targets; avoid a full clean unless a cold rebuild is acceptable.
6. Benchmark mold versus committed BFD separately; preserve current uncommitted linker change until resolved.

## Non-goals

- No nightly Cranelift configuration initially.
- No new task runner or wrapper scripts.
- No permanent `dynamic_linking` Cargo feature.
- No automatic destructive cache cleanup.
- No workspace-wide test run after every edit.

## Success Criteria

- New worktrees reuse existing Bevy artifacts instead of performing 30+ GiB cold builds.
- Bevy integration-test binaries and link time shrink through reduced debug info and optional dynamic linking.
- Heavy Cargo commands are serialized; small package feedback remains scoped and fast.
- sccache provides secondary reuse after compatible cache misses or cleanup.
- Inner-loop commands target changed packages; full validation remains available before merge.
- Release artifacts remain statically linked and independently runnable.
