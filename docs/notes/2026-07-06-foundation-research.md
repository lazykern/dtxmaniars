# Foundation Research — Seek Engine, Rate, Chart Hash, Replay Format

Date: 2026-07-06
Status: research notes feeding the foundation design specs. Sources: repo
survey (file:line verified), kira 0.12.1 source, SongCore/Etterna/osu!/
BeatLeader/YARG/Tachi/DTXManiaNX primary sources.

Foundation = the four things everything else stacks on:

1. Seek engine op (practice mode substrate)
2. Playback rate (audio-side, currently greenfield)
3. Chart hash identity (score DB successor)
4. Replay record format + section identity

---

## 1. Seek — engine facts

### Audio layer (good news)

- Project uses `bevy_kira_audio 0.26.0` → kira 0.12.1, **static sounds only**
  (fully decoded to memory, ogg/vorbis via Symphonia). Static seek is
  **sample-accurate, effective within one audio callback (~5–20 ms)**. The
  streaming-sound stale-buffer problem (~370 ms) does not apply to us.
- Seek primitive already exists:
  `play_bgm_from_seconds` / `play_bgm_handle_with_mix_from_seconds`
  (`crates/dtx-audio/src/lib.rs:282`, `:357`) — kira
  `.play(...).looped().start_from(sec)`. Each stops the old BGM first.
- Working template: `recover_primary_bgm`
  (`crates/gameplay-drums/src/bgm_scheduler.rs:258`) already restarts the
  stream at `(current_ms - start_ms)/1000` — the only implemented
  seek-into-BGM today.
- Position read: `position_ms` (`dtx-audio/src/lib.rs:565`). kira updates it
  once per audio callback (stairstep, ~3–23 ms cadence). Caveats:
  - After seek, `position()` still reports pre-seek value for up to one
    callback while playing.
  - **While paused, `position()` never updates after a seek** — do not trust
    it until frames actually play again.
  - Seek is fire-and-forget (no ack); only confirmation is `position()`
    moving.
- kira clocks cannot jump to arbitrary time (start/pause/stop-to-zero only),
  and scheduled sounds can't be rescheduled. Irrelevant today (we schedule
  nothing via kira clocks — chips fire from game systems), but rules out
  "pre-schedule keysounds in kira" designs later.

### Game-state layer (the real work)

Authoritative clock = `GameplayClock`
(`crates/gameplay-drums/src/resources.rs:399`), free-running + drift-corrects
toward BGM position (gain 10.0, max 20 ms/tick). `sync(ms)` snaps all fields —
natural jump primitive, BUT next tick the drift-corrector pulls toward
whatever BGM reports. Seek must therefore restart BGM at the matching offset
and keep `GameStartMs` (`resources.rs:94`) consistent, or add a dedicated
`GameplayClock::seek(ms)` (likely needed — `start_in_mode` zeroes too much).

Every scheduler dedupes via **grow-only `HashSet<usize>` keyed by chip
index**:

| Set | File | Hazard |
|---|---|---|
| `PlayedBgmChips` | `bgm_scheduler.rs:21` | back-seek: BGM layers never replay |
| `PlayedSeChips` | `se_scheduler.rs:18` | fwd-seek: burst of stale SE fires |
| `JudgedChips` | `judge.rs:15` | both; also gates note spawner |
| `TimingLineCrossed` | `resources.rs:135` | visual only |

- **Forward seek without seeding = every un-played chip with
  `target_ms <= now` fires on one tick** (bgm_scheduler, se_scheduler,
  autoplay all scan-and-fire). Must pre-insert all indices before target.
- **Backward seek without pruning = silence** (sets never shrink). Must
  remove indices ≥ target.
- Notes are ephemeral UI entities, spawned on approach / despawned on pass
  (`scroll.rs:79`, `:220`). Seek = despawn live `Note` entities; spawner
  refills from new `now`. Careful: `despawn_missed_notes_system` inserts
  into `JudgedChips` — order the seek op so it can't race.
- `chart.chips` is a flat unsorted `Vec<Chip>`; chip time recomputed via
  `chip_time_ms_with_bpm_and_bar_changes`. Seeding skip-sets = O(n) scan.
  Fine (charts are small), but a sorted `(target_ms, idx)` timeline built
  once on enter would make seek + A/B + scrub density all binary-searchable.
- A/B loop must suppress `detect_end_of_stage` (`orchestrator.rs:386`) —
  loop-back before `chart_end_ms` triggers StageClear.
- Practice must skip `save_result_then_despawn`
  (`crates/game-results/src/lib.rs:235`) — natural gate exists.
- Combo/gauge/score/skill state: freeze or reset on seek = design choice for
  the spec (practice doesn't submit scores either way).

### Sliced-BGM charts

Prior art is the (unimplemented) live-preview plan
`docs/superpowers/plans/2026-07-06-live-autoplay-preview.md` Task 4:

- Exactly one BGM chip → seek into it (`seek_seconds = target − bgm_chip_time`).
- Multiple BGM chips (sliced BGM) → **don't seek inside slices**; snap start
  to nearest BGM chip boundary.
- No BGM → clock free-runs from target.

Practice seek and live preview are the **same engine op**. Build one shared
"start/position playback at arbitrary chart time" primitive; both consume it.

### Seek op sequence (draft)

```
seek(target_ms, bar_snap):
  1. resolve target (snap to bar line / BGM-chip boundary per chart shape)
  2. stop BGM + layer instances + polyphony one-shots
  3. rebuild skip-sets: PlayedBgm/PlayedSe/Judged = { idx | chip_ms < target }
     (non-playable chips per autoplay rules included)
  4. despawn live Note entities (+ timing-line crossed set)
  5. restart BGM: play_bgm_.._from_seconds(target - primary_bgm_chip_ms)
  6. GameplayClock::seek(target)  — snap, don't fight drift correction
  7. resume; ignore stale position() for first callback
```

### Scrub bar

`DensityGraph` widget is a per-lane histogram — **not time-indexed**, wrong
substrate. Right substrate = `AccuracyHistory` model
(`resources.rs:559`): fixed 128-bucket array over song length. New widget:
time-bucketed note density, horizontal, playhead + A/B markers.

---

## 2. Playback rate — engine facts

- Fully greenfield audio-side. `play_speed` today only rescales chart-time
  math + scroll (`resources.rs:230` comment: audio rate NOT rescaled).
- kira supports `set_playback_rate(rate, tween)` on instances (bevy_kira_audio
  exposes it). **Resampling only — pitch shifts with rate. No built-in
  time-stretch.** Confirms plan: v1 = rate with pitch shift + "adjust pitch"
  toggle framing; pitch-preserving DSP (signalsmith-stretch) later, internal
  to dtx-audio.
- Design question for spec: keysound one-shots — play at same rate as BGM for
  pitch consistency? (Probably yes; both shift together sounds musically
  coherent; mismatched is jarring.)
- Static sounds support negative rate (reverse) — not needed, noted for fun.

---

## 3. Chart hash identity — decision input

Current state: `ScoreStore` keys by SHA-256 of raw chart file
(`game-results/src/lib.rs:261`), plus BocuD-compatible `.score.ini` beside
chart (DTXManiaNX uses raw-file MD5 — metadata edit orphans scores).

Survey of prior art:

| Scheme | Hashes | Survives metadata edit | Weakness |
|---|---|---|---|
| osu! MD5, BeatSaver SHA1, DTXManiaNX MD5 | raw file bytes | no | any byte change orphans scores |
| Etterna chartkey | parsed notes + int-truncated BPM | yes | omits row gaps + exact timing → real-world collisions |
| Tachi | none — internal chartID + N attached hashes | n/a | needs seeded DB |

**Recommendation:** dual identity, Tachi-style multi-hash:

1. **Canonical hash (primary key):** SHA-256 over parsed, normalized note
   data — per chip `(channel, measure, rational position)` sorted
   canonically, plus full timing map at exact values (BPM as decimal string /
   fixed-point — NOT truncated int, Etterna's mistake) and bar-length
   changes. Exclude: title/artist/comments, WAV/BGM/AVI paths, volume/pan,
   file encoding. Survives Shift-JIS↔UTF-8 re-encode, metadata edits,
   set.def reshuffles. **Version-prefix the key** (e.g. `dtx1:<hex>`) so the
   canonicalization can evolve.
2. **Raw-file hash (secondary):** keep current SHA-256 (or MD5 for
   DTXManiaNX interop) stored alongside — cheap dedupe + score.ini compat.

Identity lives per chart file, not per set.def entry.

---

## 4. Replay record + section identity — decision input

Prior art:

- **osu! .osr**: header + LZMA text frames (delta-ms, pos, buttons). Input
  stream → full re-sim. No extensibility (RNG seed smuggled as fake frame).
- **BSOR (BeatLeader)**: binary, magic + version byte, byte-tagged blocks;
  stores **both raw motion frames AND per-note judgement events** — viewers
  don't re-implement scoring, analytics don't parse frames. Acknowledged
  best-in-class open spec.
- **Etterna**: judgement-only V2 proved insufficient (can't see actual
  presses); later added gzipped input stream with rate/offset/rng/engine
  version in header. Keeps both tiers.
- **YARG** (closest model — input-driven deterministic engine): `YAREPLAY`
  magic, format version int + **separate engine version int**,
  length-prefixed integrity-checked blocks, chart checksum binding, stats
  stored redundantly so results screens don't re-simulate. Read results
  distinguish MetadataOnly/InvalidVersion/DataMismatch/Corrupted.

Sizes: event-based drum input ≈ 8–16 bytes/event before compression; a full
song ≈ tens of KB. Record everything, always.

**Recommendation (YARG structure + BSOR dual-layer):**

```
magic "DTXR" | u16 format_ver | u16 engine_ver (judge/scoring rules)
header (length-prefixed): canonical chart hash, raw hash, rate, offsets
       (input/bgm/visual), modifiers, timestamp, final stats (redundant)
section INPUT  (tagged, zstd): pad/velocity/CC4 events, delta-time encoded
section JUDGE  (tagged, zstd): per-chip judgement + signed error ms
optional tagged sections later (skip-unknown parsing)
```

Dual layer = replays watchable (input) + analytics/rescoring (judge) without
re-simulation. Engine version prevents mis-simulating old replays after
judge changes. Record velocity from day one — per-limb analytics (§4.3 of
idea catalog) reads it retroactively.

**Section identity** (training loop key): `(canonical chart hash, bar_start,
bar_end)`. Bar boundaries from `TimingLineList`; ms via
`chip_time_ms_with_bpm_and_bar_changes(measure, 0.0, ...)`.

**Tachi export note:** BATCH-MANUAL has no generic chart-hash matchType —
keep a local `(chart hash → songTitle/inGameID)` mapping exportable; export
is `game: "gitadora", playtype: "Dora"`, score + lamp + judgements +
timeAchieved (unix ms).

---

## Phase proposal

```
Phase 0  Formats spec (small, decide-and-freeze):
         canonical chart hash, replay record, section identity,
         three offset knobs. Spec + tiny impl (hash fn in dtx-scoring,
         replay writer skeleton). Everything downstream keys on these.
              │
Phase 1  Seek engine spec + impl:
         shared "position playback at chart time T" op
         (GameplayClock::seek, skip-set rebuild, BGM restart-at-offset,
         note despawn/refill, sliced-BGM snap rules).
         NOTE: live-autoplay-preview plan Task 4 = same machinery.
         Sequence them together — one primitive, two consumers.
              │
Phase 2  Practice mode UI spec + impl:
         scrub bar (time-bucketed density widget), A/B loop (bar-snapped),
         rate control (pitch-shift v1 + toggle), pause-menu integration,
         no score submission. Replay recording lands here too (same events).
              │
Phase 3  Trainers on top: wait mode, accuracy-gated ramp, checkpoints.
```

Per-pillar flow: research (done) → brainstorm/clarify → spec in
`docs/superpowers/specs/` → plan → implement. Formats (Phase 0) designed now
even though some consumers come much later — cheap to decide, expensive to
retrofit.
