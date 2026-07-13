# Documentation and Repository Truth Repair — Design

Date: 2026-07-13
Status: Approved
Program cycle: 8
Explicit exclusion: CI/CD configuration and workflows

## Goal

Make the repository's player instructions, contributor rules, reference links,
decisions, support claims, and executable behavior agree. Documentation becomes
a maintained product surface with local verification, not a collection of
dated notes that readers must reconcile themselves.

## Canonical document map

The stable entry points are:

- `README.md`: concise player/contributor landing page;
- `PRODUCT.md`: audience, purpose, principles, and committed accessibility;
- `docs/roadmap.md`: current program status and future work;
- `docs/decisions/README.md`: ADR index and status;
- `docs/player-guide.md`: controls and player workflows;
- `docs/compatibility.md`: executable chart/media support matrix;
- `docs/data-and-persistence.md`: locations, schemas, migration, and backup;
- `docs/contributing.md`: architecture, reference-first workflow, and local gates;
- crate `AGENTS.md`: crate-local boundaries, references, and tests.

Dated `docs/notes/`, specs, and plans remain immutable historical evidence
except for broken paths that prevent navigation or a clearly marked correction
of a false repository-state statement. They are not the current roadmap or
manual.

## Roadmap repair

Create `docs/roadmap.md` and replace the missing
`docs/superpowers/specs/2026-07-11-game-improvement-roadmap-design.md` pointer.
The new roadmap carries Cycle 0–8 status, links to each approved design/plan,
and lists remaining work without duplicating implementation details. The
2026-07-13 program note remains the execution ledger.

`AGENTS.md` and README link only to the stable roadmap. A dated spec may link to
the roadmap, but the roadmap does not require readers to discover a newer dated
file.

## Reference-root repair

The live reference root is `references/DTXmaniaNX/`. Replace stale
`references/DTXmaniaNX-BocuD/` paths in source comments, crate handbooks,
plans/specs, tests, and contributor docs. The current audit finds 80 affected
files.

This is not a blind string replacement:

- validate that each resulting target exists;
- repair renamed/moved files individually;
- preserve line citations only when the cited range still contains the claimed
  behavior;
- correct dated notes that falsely claim references are absent by adding a
  dated correction, not rewriting the historical research conclusion;
- never modify anything inside `references/`.

Reference examples and all new ADRs use repository-relative paths with the
actual root.

## Reconstructed ADRs

Create individual decision records with Context, Decision, Evidence,
Consequences, Status, and Supersedes/Superseded By fields. Reconstruct only
decisions supported by code/comments/commits:

1. Drums-first product scope and instrument boundary.
2. Gameplay/audio clock authority; never judge on accumulated frame delta.
3. References are read-only vendored research inputs.
4. Reference-first mechanics workflow and citation policy.
5. Pure/Engine/Game/Binary crate layering.
6. Input profiles as the live source of truth, with legacy binding migration.
7. Mechanics port from NX while UX/UI is independently redesigned.
8. 300 ms OutQuint screen transitions.
9. Preview/crossfade ownership and per-chart preview fallback.
10. Modifier-qualified score persistence, including speed, practice, and
    No Fail exclusions.

Use existing ADR numbers where code already cites them. If the historical title
cannot be proven, keep the number and mark the reconstruction explicitly; do
not invent a false original document date.

### Transition contradiction

The binding product decision is the implemented 300 ms OutQuint fade in
`dtx-ui`/`game-shell`. `game-shell/AGENTS.md` currently describes NX's 1500 ms
linear snapshot as mandatory and contradicts the root handbook and runtime.
Rewrite it to distinguish:

- reference behavior: NX 1500 ms linear snapshot;
- product decision: DTXManiaRS 300 ms OutQuint overlay;
- reason: UX is redesigned while stage mechanics remain port-first.

NX behavior remains cited for comparison, never presented as the current
requirement.

## README and player documentation

README covers only the shortest successful path and links to detail. It must
include verified information for:

- supported operating systems and Rust 1.95+ toolchain;
- platform prerequisites, including FFmpeg/video and audio dependencies;
- install, development run, and release build commands;
- song directory resolution and archive import;
- supported chart, archive, audio, image, and movie formats;
- keyboard and MIDI setup, profiles, calibration, and system binds;
- Song Select search/sort/discovery controls;
- normal play, pause, practice, results analysis, and recommended practice;
- score qualification for play speed, practice, No Fail, and future modifiers;
- config, scores, library preferences, profiles, and layout locations;
- common recovery paths: no songs, unsupported media, parse/load warning,
  missing FFmpeg, MIDI unavailable, and corrupt settings;
- contributor quickstart and links to architecture/reference rules.

`docs/player-guide.md` replaces the dated current-behavior manual as the
maintained player reference. `docs/compatibility.md` uses the exact Supported /
Degraded with Warning / Rejected with Recovery language from Cycle 7 and is
checked against fixture-backed outcomes. `docs/data-and-persistence.md`
documents version fields, atomicity where present, backup implications, and
safe deletion/recovery without promising migrations that do not exist.

Every command in these docs is executed from a clean checkout or verified
against Cargo metadata. Platform packages are listed only when confirmed by
the build dependencies; unsupported platforms are stated honestly.

## Crate handbook refresh

Review every crate `AGENTS.md`, prioritizing crates changed by Cycles 0–7.
Update:

- layer and dependency boundaries;
- current API/types rather than milestone-era shapes;
- implemented/deferred support;
- exact package and integration-test commands;
- correct reference paths;
- ownership of cross-crate behavior;
- local rules that genuinely differ from the root handbook.

Remove stale milestone claims and contradictions. Do not duplicate the entire
root handbook in each crate; a crate file should answer only what is local.

## Local documentation checker

Add a small pure Rust tool at `tools/docs-check`, automatically included by the
workspace's `tools/*` member pattern. Its default command checks:

- relative Markdown links resolve;
- repository-local code/reference paths exist;
- the obsolete reference-root token does not appear as a live path in source or
  current documentation; an allowlisted historical explanation and the
  checker's negative fixture may quote it as non-resolving text;
- canonical docs exist and link to one another;
- reference paths do not escape `references/`;
- no documentation claims a missing canonical file.

The tool reports file, line, failing target, and a concise reason, returns
nonzero on failure, and never modifies files. URL availability is not checked,
so the command remains deterministic and network-free. No CI/CD files are
created or changed.

## Test-quality audit

Search the workspace for placeholder, tautological, and surface-only tests,
including:

- assertions that accept both boolean outcomes;
- a value compared to itself or to the constant that constructed it;
- tests that only prove an enum/constant exists;
- placeholder names/count tests with no behavior;
- ignored bodies with no linked issue/reason.

For each finding:

- replace it with a public behavior assertion when behavior exists;
- merge it into an existing behavior test when that is clearer;
- remove it when it proves nothing and no behavior is implemented;
- retain compile-surface tests only when they protect a documented public API,
  and rename them to state that contract.

This is not a coverage-percentage exercise and does not authorize unrelated
refactoring. Touched tests must demonstrate a failure mode or observable
contract.

## Consistency workflow

1. Generate an inventory of canonical docs, stale roots, broken links, ADR
   citations, compatibility claims, commands, and suspect tests.
2. Repair stable docs and ADRs first.
3. Update crate handbooks/source citations against those decisions.
4. Replace/remove suspect tests.
5. Run `docs-check`, every documented command that is safe locally, formatting,
   workspace check, Clippy, and relevant package tests.
6. Update the roadmap/program status only after all acceptance criteria pass.

## Error handling and historical integrity

- A reference citation whose intended target cannot be identified is reported
  and left explicit; it is not redirected to a plausible unrelated file.
- A lost ADR with insufficient evidence remains `Unreconstructed` in the index.
- Historical notes receive correction blocks with dates and links; their
  original observations are not silently rewritten.
- Documentation commands that require optional hardware or GUI interaction are
  labelled manual checks rather than claimed as automated verification.
- No network access is required for the local documentation gate.

## Verification

- `cargo run -p docs-check` passes from the repository root.
- Every relative link/reference path in canonical docs and handbooks resolves.
- No stale reference root remains in a live path; only this design's historical
  explanation and the explicit docs-check negative fixture may quote its token.
- Cargo install/run/build/test commands in README and contributing docs are
  executed or mechanically validated against Cargo metadata.
- Compatibility statements match Cycle 7 fixture outcomes.
- Reconstructed ADR citations match runtime/source behavior.
- Suspect-test inventory is empty or every retained item has a documented API
  contract.
- Local formatting, workspace check, warnings-as-errors Clippy, and relevant
  tests pass; CI/CD remains untouched.

## Acceptance criteria

- A new player can install, add songs, configure input, play/practice, and find
  their data without reading source or dated notes.
- A contributor can find the current roadmap, decisions, crate rules,
  references, and release gates from README/AGENTS.
- Current docs contain no known broken local links or false reference paths.
- Transition, compatibility, and score-modifier policy each have one binding
  written source that matches executable behavior.
- The improvement program is marked complete only after these conditions and
  the cross-cycle acceptance criteria are verified.
