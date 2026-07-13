# Contributing

Purpose: maintained contributor workflow for architecture, reference evidence,
and local quality gates.

Audience: code and documentation contributors.

Status: Maintained. Package-specific ownership and focused commands live in
each crate's `AGENTS.md`.

Neighboring guides: [roadmap](roadmap.md),
[decision records](decisions/README.md),
[compatibility](compatibility.md), and
[data and persistence](data-and-persistence.md).

## Start with the contract

Before editing, read the root [AGENTS handbook](../AGENTS.md), this guide, the
affected crate's `AGENTS.md`, and relevant accepted ADRs. For a multi-step
change, read the approved design/plan linked from the [roadmap](roadmap.md).

Mechanics are reference-first under [ADR-0004](decisions/0004-reference-first-mechanics-workflow.md):
inspect and cite the corresponding implementation under
`references/DTXmaniaNX/`. UX is deliberately redesigned under
[ADR-0010](decisions/0010-port-mechanics-redesign-ux.md) and current product
decisions. The entire `references/` tree is read-only; never edit, format, copy
generated output into, or commit changes from it.

## Preserve dependency direction

| Layer | Packages | May depend on |
|---|---|---|
| Pure | `dtx-core`, `dtx-scoring`, `dtx-config`, `dtx-layout`, `dtx-persistence`, `xtask`, `docs-check` | Pure only |
| Engine | `dtx-timing`, `dtx-audio`, `dtx-input`, `dtx-assets`, `dtx-library`, `dtx-bga`, `gameplay-guitar` | Pure and Engine as needed |
| Game | `dtx-ui`, `gameplay-drums`, `game-shell`, `game-menu`, `game-results`, `dev-tools` | Pure and Engine; Game edges must remain intentional |
| Binary/tool | `app/dtxmaniars-desktop`, `tools/dtx-cli` | Composition only |

Do not make a Pure crate depend on Bevy. Cross-screen transitions are requests;
the shell/director owns the actual next state. Package boundaries and any
stricter local rules are documented in crate handbooks.

## Development loop

The workspace requires Rust 1.95+. Use a shared `CARGO_TARGET_DIR` across
worktrees when appropriate; Bevy rebuilds are large. Start with the smallest
command that exercises the changed contract:

```sh
cargo check -p <package>
cargo test -p <package> --lib
cargo test -p <package> --test <integration-test>
```

The chart validator package is `dtx-cli`, while its binary is named `dtx`:

```sh
cargo run -p dtx-cli -- validate crates/dtx-core/tests/fixtures/minimal.dtx
```

For the desktop game during development:

```sh
cargo run -p dtxmaniars-desktop --features bevy/dynamic_linking
```

Dynamic linking is a development optimization only. Release builds and install
instructions must omit it.

Write tests around observable behavior: parsing outcomes, state reduction,
persistence round trips, events, or rendered data. Avoid tautologies,
placeholder assertions, and tests that only repeat a constant without
protecting a public compile surface. When fixing a bug, first reproduce it with
a failing focused test.

## Documentation truth

Run the local checker after changing Markdown, canonical maps, reference
citations, or support claims:

```sh
cargo test -p docs-check
cargo run -p docs-check
```

It checks local Markdown targets, canonical documents, reference paths,
obsolete reference-root tokens, and known false “missing documentation” claims.
Historical notes may remain historical, but append a dated correction when a
maintained conclusion changed. Support statements must use the states defined
by [Compatibility](compatibility.md): Supported, Degraded with Warning, or
Rejected with Recovery.

## Before merge

Run changed-package tests, then the local release gates from repository root:

```sh
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --lib
cargo test -p docs-check
cargo run -p docs-check
git diff --check
```

Commands that require an actual MIDI device, audible output, or visual GUI
inspection are manual checks. Record them as manual rather than presenting a
successful compile as hardware evidence. A release binary can be built with:

```sh
cargo build --release -p dtxmaniars-desktop
```

Keep changes focused, preserve unrelated work, and make one commit per logical
change. Do not add CI/CD changes to the current improvement program.
