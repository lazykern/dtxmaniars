# 0008: Reference-first workflow

Status: accepted
Date: 2026-06-23

## Context

DTXManiaNX (730MB) and osu-lazer (89MB) are the **only** authoritative sources of
DTX semantics and fluidity UX. AI agents have a tendency to code from training
memory or guess API shapes, producing plausible-but-wrong implementations:

- Wrong DTX command parsing (e.g. parsing `#BPM` as int when it's float)
- Re-implementing osu-lazer's `PlayerLoader` 1800ms hold from scratch instead
  of citing the actual constant
- Inventing guitar wailing rules that conflict with `CChip.WailingBonus`

## Decision

**Before writing implementation code for any crate, agents MUST:**

1. Read the **per-crate `AGENTS.md`** to find the relevant reference files.
2. Read those reference files (or a targeted excerpt via `ctx_execute_file` if >50KB).
3. Cite the file:line in the PR/commit for any non-trivial behavior ported.
4. If unsure, prefer to **port the original behavior** over a clean-room rewrite.

**Tools to use (in order):**

| Need | Tool |
|---|---|
| Quick excerpt (line ranges, structure of a known file) | `ctx_execute_file path=...` |
| Whole-file index for later recall-by-topic | `ctx_index path=...` (small files <50KB) |
| Cross-file search across indexed refs | `ctx_search queries=[...]` |
| Bevy API questions | `npx ctx7@latest docs /websites/rs_bevy "..."` |

**Already-indexed reference files** (re-index on per-session basis):

- `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/{EChannel,CChip,CChartData}.cs`
- `references/DTXmaniaNX-BocuD/FDK/Sound/CSoundTimer.cs`
- `references/DTXmaniaNX-BocuD/DTXMania/Core/StageManager.cs`
- `references/DTXmaniaNX-BocuD/DTXMania/Stage/04.SongSelectionNew/*.cs`
- `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/*.cs`
- `references/osu-lazer/osu.Game.Rulesets.Mania/UI/*.cs`
- `references/osu-lazer/osu.Game/Screens/{Select/SongSelect,Play/PlayerLoader}.cs`

CDTX.cs (272KB) is **not** indexed wholesale — use `ctx_execute_file` with line
ranges when implementing specific parser features.

## Consequences

- Slower per-task (read first, code second) but fewer wrong-direction commits.
- Reference files become **primary**, training memory becomes **secondary**.
- Every behavior in the game has a citable origin.
- New sessions must (re)index refs once; this is cheap (~30s) and pays back.

## Alternatives considered

- **Code first, ref later:** faster iteration, much higher rework rate.
- **No refs at all:** would diverge from DTXManiaNX semantics within a week.