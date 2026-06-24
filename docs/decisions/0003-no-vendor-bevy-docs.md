# 0003: Do not vendor Bevy documentation

Status: accepted
Date: 2026-06-23

## Context

Bevy 0.19 docs are large (~100MB+). Vendoring them adds repo bloat, goes stale
within weeks, and conflicts with the project's "don't reinvent" rule (a local
copy is worse than upstream).

## Decision

Bevy docs are fetched on demand via **`ctx7`**:

```sh
npx ctx7@latest docs /websites/rs_bevy "<exact question>"
```

Plus local `cargo doc --open` for offline browsing when needed.

## Consequences

- Repo stays small.
- Always-current API surface.
- Quota limit on ctx7 free tier; use `cargo doc` for offline fallback.
- Requires network for fresh API questions; this is acceptable.

## Alternatives considered

- **Git submodule of bevy repo:** full source, but ~500MB and slow.
- **mdBook download:** ~50MB, monthly restale cycle.
- **Train-from-context:** impossible, Bevy API changes every release.

## Reference files

- (none — applies to all Bevy work)