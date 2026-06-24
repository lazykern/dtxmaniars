# docs/

Three-tier documentation so scratch, stable, and irreversible don't mix.

| Folder | Purpose | Edit frequency |
|---|---|---|
| `ROADMAP.md` | Phase checklist, current milestone | Weekly |
| `ARCHITECTURE.md` | Stable crate map, layer rules, data flow | When structure changes |
| `BEVY_PATTERNS.md` | Project-specific Bevy patterns (~50 lines) | Rarely |
| `decisions/NNNN-*.md` | ADRs (Architecture Decision Records) | Append-only |
| `notes/` | Scratchpad, research, session logs | Often (gitignored if `local/`) |
| `phase-plans/` | Per-milestone deliverable checklists | Per phase |

## ADR workflow

1. Copy `decisions/template.md` to `decisions/NNNN-<kebab-title>.md`.
2. Fill in Status, Context, Decision, Consequences, Reference files.
3. Commit. Do not amend old ADRs — supersede with a new one.

## Notes workflow

- `docs/notes/<topic>.md` — shareable across agents (research dumps, findings).
- `docs/notes/local/<agent>/` — per-agent scratch. Gitignored.