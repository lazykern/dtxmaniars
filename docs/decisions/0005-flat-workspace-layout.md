# 0005: Flat `crates/` workspace layout

Status: accepted
Date: 2026-06-23

## Context

A project of 10–30 crates benefits from either flat (`crates/<name>/`) or
nested (`crates/engine/<name>/`) layout. matklad's analysis (2021) and rust-analyzer's
structure favor flat until ~50 crates or ~1M LOC.

## Decision

- Root `Cargo.toml` is a virtual manifest only — no `src/`.
- All crates live at `crates/<name>/` or `app/<name>/` or `tools/<name>/`.
- Folder name == crate name.
- All internal crates `version = "0.0.0"`, `publish = false`.
- **Layer convention is logical** (Pure / Engine / Game in `docs/ARCHITECTURE.md`),
  not physical — no nested `crates/engine/` folder.

## Consequences

- `ls crates/` shows full project at a glance.
- Adding/splitting crates is trivial.
- No empty/catch-all folders appearing as project grows.
- Renames are simple (folder = crate name).

## Alternatives considered

- **Nested layout:** prettier grouping, but hierarchy rot, harder AI navigation.

## Reference files

- https://matklad.github.io/2021/08/22/large-rust-workspaces.html
- https://github.com/rust-analyzer/rust-analyzer (32 crates, flat)