# 0004: `references/` is read-only and not vendored

Status: accepted
Date: 2026-06-23

## Context

`references/` holds 730MB (DTXmaniaNX) + 89MB (osu-lazer). Submodules add clone
complexity, fetch-time, and pinning churn. Plain copy-into-repo bloats git.

## Decision

- `references/` is **gitignored**.
- Each developer clones reference repos locally as needed.
- In-session, agents use `ctx7` for Bevy docs and `ctx_search` over indexed
  reference content (indexed via `ctx_index` once per session).
- Reference files cited in issues/PRs are linked, not copied.

## Consequences

- Clone of `dtxmaniars` itself stays fast and small.
- Reference sync is per-developer, not enforced.
- Lost if developer forgets to clone; mitigate with `make setup` or `xtask setup`.

## Alternatives considered

- **Git submodules:** clean version pinning, painful UX, large clone.
- **Copy into repo:** +800MB git history, no upgrade path.
- **Sparse checkout:** requires git knowledge from every contributor.

## Reference files

- (applies to all reference repos)