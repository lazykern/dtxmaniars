# Roadmap

Drums-first MVP. DTXmaniaNX semantics + osu-lazer fluidity. Bevy 0.19.

| Milestone | Deliverable | Exit criteria | Status |
|---|---|---|---|
| **M0** | Workspace + dtx-core parser + dtx-cli + docs | `cargo test -p dtx-core` green; `dtx-cli validate` parses fixture | **Done** |
| **M1** | dtx-timing + dtx-audio (kira) headless clock | Headless test: play BGM, log authoritative `current_ms` over 5s | **Done** (5s playback test deferred to M2 — needs audio device or NullBackend) |
| **M2** | gameplay-drums vertical slice | One chart playable via keyboard; P/G/Miss + score shown | **Done** |
| **M3** | game-shell: AppState, loading, DTXManiaNX fades | Title → load → play → back with **1500ms snapshot fades** (StageManager.cs:29 — DTXManiaNX baseline, NOT osu) | **Done** (black-overlay approximation; true framebuffer snapshot → M3.1 ADR-0011) |
| **M4** | dtx-assets + game-menu (real SongSelect + Config) | End-to-end: Title → SongSelect → SongLoading (parses DTX) → Performance (with ActiveChart) → Title | **Done** (visual simplification per ADR-0012; real SongDb + BGM preview → M5) |
| **M5** | dtx-library + BGM preview + game-results | Browse folder of charts (SongDb); BGM preview plays on row select; results screen shows score + per-judgment counts + rank | **Done** (CScoreIni save → M6; visual polish on Result → M5.1) |
| **M6** | Guitar + CScoreIni + BGA + MIDI | `gameplay-guitar` crate, `dtx-input` MIDI, `dtx-scoring` JSON scores, `dtx-bga` placeholder | **Done** |
| **M7** | BGA event detection | `dtx-bga` crate + placeholder overlays; real image/video decode → M7.1+ | **Done** (overlay-only; real image load → M7.1) |
| **M8** | Audit + roadmap refresh | docs/notes/PORT_GAP.md maps all 267 ref files; ROADMAP updated | **Done** |
| **M9** | Full Performance HUD (Drums) | All 10 CActPerfDrums* sub-acts + CStagePerfDrumsScreen ported; render in Performance stage | **Done** |
| **M10** | Full SongSelect UX | DensityGraph + SortMenu + SongSearchMenu + StatusPanel + StatusPane + CommandHistory verbatim per reference | **Done** |
| **M11** | Full Result UX | Rank icon (S/A/B/C/D/E) + song bar + ghost bar + score + max combo + 5 judgment counts | **Done** |
| **M12** | Full Config | 5 tabs (system/game/play/keyassign/skin) | **Done for M13** (skin browser dropped — see ADR-0014/roadmap notes) |
| **M13** | Integration + full-flow verify | All 9 states boot clean, BocuD timing windows, audio preload/voice pools, focused tests green | **In progress** (final workspace verify pending) |
| **M13.5** | Playable + osu-smooth drums | Split clock + sub-frame interpolation + framepace; NX scroll/scoring/gauge/combo; pause + StageClear/StageFailed; real loading (bg parse + wait-on-handles); Config edits persist; tiered WAV preload; BocuD `.score.ini` best-score read in song select | **Done** |
| M14+ | CubeTest, Updater, online | TBD | Future |

## M13.5 — Playable smooth drums (this goal)

Two sources kept distinct (see `docs/decisions/`): **mechanics** ported verbatim
from DTXManiaNX-BocuD, **smoothness/UX** patterns from the dtxpt POC.

- **Timing/smoothness (dtxpt engine plumbing):** split clock
  (`audio`/`visual`/`prev` + drift correction) in `GameplayClock`; `RenderClock`
  sub-frame interpolation drives scroll; PreUpdate wall-clock input capture;
  `bevy_framepace` in the desktop binary.
- **Mechanics (BocuD-verbatim):** NX scroll `(scrollIdx+1)·0.17875` px/ms;
  XG scoring (base + combo ramp + FC/EXC bonuses); Poor resets combo; gauge
  start 2/3, fail −0.1, XG deltas + damage-level miss scaling; nearest-unhit
  chip judgment with lane groups; input offset applied to the judgement clock.
- **Playable UX:** Esc pause overlay (resume/retry/quit, freezes BGM+clock);
  StageClear / StageFailed banners between Performance and Result; real song
  loading (background parse + wait-on-asset-handles, no fake timer); Config
  screen edits + persists scroll speed / input offset / master volume / damage
  level, re-applied on each Performance entry; tiered WAV preload (immediate
  note WAVs waited on, deferred BGM/SE decode in background).
- **Persistence:** BocuD-compatible `<chart>.score.ini` written on Result and
  read into the song-select detail panel (best score + rank + max combo).

## Current focus

**ADR-0014 UX redesign** — osu-inspired fluidity. Mechanics stay BocuD-ported; all visuals redesigned.

Completed: 300ms OutQuint transitions, dark theme, osu-style HUD widgets (rolling score, bounce combo, tweened gauge, judgment popup, lane flush), 3-column song select, themed screens (Startup/Title/SongSelect/Config/SongLoading/Result/End), BocuD 34/67/84/117 timing windows, chart sound-bank preload, real drum voice reuse, BRP debug, `.mcp.json`, `system_font_discovery` for CJK text.

Remaining: data-driven density bars (M10.1), real BGA decode (M7.1).

## Port coverage summary

From `docs/notes/PORT_GAP.md`:

| Status | Files | Lines |
|--------|------:|------:|
| ported  |  ~3 |  ~3k |
| minimal | ~19 | ~9k |
| missing | ~245 | ~71k |

Total: 267 ref files, ~83k LOC. Current Rust port: ~12k LOC across 14 crates (~14% ratio, up from 6.3% after M8–M12).

## M8–M12 completion summary (this goal)

All five milestones shipped:
- **M8**: docs/notes/PORT_GAP.md (520 lines) tags all 267 game ref files; ROADMAP updated.
- **M9**: 10 CActPerfDrums* sub-acts + CStagePerfDrumsScreen orchestrator ported. 11 new tests in gameplay-drums::hud.
- **M10**: 7 SongSelectNew components ported (StatusPanel/StatusPane/DensityGraph/SortMenu/SongSearchMenu/CommandHistory/CActSelectPresound). 13 new tests in game-menu::song_select_full.
- **M11**: 5 Result components ported (ResultRankIcon/InfoPanel/ParameterPanel/SongBar/GhostBar). 8 new tests in game-results::result_full.
- **M12**: 12 ConfigTab enum + 11 ConfigItem loaders + AvailableSkins scanner. 12 new tests in game-menu::config_full.
- Total tests: 181 (up from 137 before M8, +44 across M9–M12).

## M9–M13 scope (this goal)

Gameplay-complete port per active goal. Each milestone ends with `cargo test --workspace` green + binary boot verification.

**M9** — Full Performance HUD (Drums). Sub-acts listed above. EGameMode-gated to Drums (per active goal's out-of-scope: M6.1 polish).

**M10** — Full SongSelect UX. `Stage/04.SongSelectionNew/` (11 files, 2779 lines). DensityGraph + SortMenu + SongSearchMenu + StatusPanel + StatusPane + CommandHistory + CActSelectPresound. Supsersedes M4.1.

**M11** — Full Result UX. `Stage/07.Result/` (5 files, 2104 lines). CStageResult + CActResultParameterPanel + ResultInfoPanel + ResultParameterPanel + ResultRankIcon.

**M12** — Full Config. `Stage/03.Config/` (14 files, 4996 lines). 5 config tabs (system/audio/graphics/gameplay/menu/drums/guitar/bass/skin/keyassign). Skin browser removed from scope — no `CStageChangeSkin` port.

**M13** — Integration + verify. Wire End stage minimal (CStageEnd.cs, 87 lines), end-to-end boot test, all tests green, clippy clean.

## Out of scope (deliberate deferrals)

- CubeTest stage — M14+
- Updater — M14+
- Online / Discord rich presence — M14+
- M3.1 framebuffer snapshot fade (ADR-0011) — M3.1
- M5.1 Result visual panels (rank icon / song bar / ghost) — **superseded by M11**
- M6.1 polish (gate Drums input on EGameMode, guitar chord/open/hold judgment, EGameMode rehome) — out of scope
- DTXCreator tool (separate editor) — out of scope
- libbjxa library (Ogg/Vorbis encoder) — out of scope, use kira
- FDK Common (CActivity / CTimer / CPad / COpenGL) — minimal Bevy equivalents, not 1:1 port
- FDK Sound (CSoundManager / CEtude / etc.) — replaced by bevy_kira_audio + dtx-audio
- M9.1: PerfChipFireD chip-strike particles (1081 LOC) — placeholder, deferred
- M10.1: DensityGraph bar heights from real chip count, search text input
- M11.1: real rank icon sprites, sub-rank SS detection
- M12.1: per-item dtx-config backing, KeyAssign key-press capture
