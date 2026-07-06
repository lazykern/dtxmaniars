# Next-Gen DTXMania — Feature Idea Catalog (Research-Backed)

Date: 2026-07-06
Status: idea catalog, not a spec. Sources: DTXMania/NX, GITADORA, osu!(lazer),
Clone Hero/YARG, Etterna/StepMania, Quaver, Beat Saber ecosystem, IIDX,
Melodics/Drumr/Herta/Anytune/Rocksmith, Tachi, auto-charting research.

Current state (from repo survey): song select + gameplay + scoring + autoplay +
results + skill + config editor all working (ROADMAP M0–M13.5 done). No
practice/seek anywhere. Lane config partial (visibility/mirror/reverse only).
Options: Speed / Risky / Auto / Mirror. Config = TOML via `dtx-config`.

---

## 1. Practice & Training

### 1.1 Practice mode core (seek / loop / rate) — user idea #1
The single biggest gap. Nothing comparable exists in DTX world beyond NX's
crude looping.

Design sketch:
- **Seek surface = density graph** (Etterna pattern, best UX found): the
  DensityGraph widget from song select rendered as an in-game scrub bar.
  Click = warp. Right-click = set anchor, `Backspace` = return to anchor.
  Two right-clicks = A/B loop region.
- **A/B loop** snapped to bar lines (DTX has no section markers; bars are the
  natural unit). Optional loop rep counter.
- **Rate control** 0.5x–1.5x in 5% steps (Clone Hero) or 0.05x (Etterna).
  Must stay audible when slowed — Taiko Rhythm Festival mutes slowed music
  and players hate it. Pitch-preserving stretch: kira `playback_rate` shifts
  pitch; DTXMania precedent is `PlaySpeed` + `TimeStretch=ON`. Needs DSP
  (e.g. signalsmith-stretch) or accept pitch shift at v1 with a toggle
  (lazer: "Adjust pitch" toggle on rate mods).
- **No fail, no score submission** in practice (Beat Saber/CH standard).
- Engine note: `dtx-timing` clock is one-way audio-authoritative; seek =
  reposition kira playhead + rebuild scroll/judge/BGM-scheduler state.
  Touches `dtx-timing`, `bgm_scheduler.rs`, `scroll.rs`, `orchestrator.rs`.
  Do this before more systems stack on the clock.

```
            ┌─ density graph (scrub bar) ─┐
   click ──►│ ▂▃▅▇▅▃▂▁▂▅▇▇▅▂ [A====B]     │◄── right-click x2 = loop
            └──────────────┬──────────────┘
                           ▼
                 seek(target_ms)
                           │
        ┌──────────────────┼──────────────────┐
        ▼                  ▼                  ▼
  kira playhead      rebuild scroll     re-arm BGM/keysound
  reposition         + judge cursor     scheduler from target
```

### 1.2 Wait mode (Melodics) — differentiator
Playback pauses at each note until the correct pad is hit. No PC drum game
has this. Trivial once seek/pause plumbing exists: freeze clock at chip time,
resume on matching `LaneHit`.

### 1.3 Accuracy-gated speed ramp (Rocksmith + Anytune hybrid)
Loop section at 70% rate; each pass with ≥90% accuracy auto-bumps +5% until
100%. Rocksmith "accelerate" is the proven design; Anytune ramps blindly —
gating on judge % beats both. Cheap layer on top of 1.1.

### 1.4 Auto-checkpoints / retry-from-section (ADOFAI pattern)
Quick-retry (IIDX INFINITAS hold-to-restart) + "retry from last A point".
Removes biggest practice friction on long songs.

### 1.5 SRS practice queue (Sostenuto/Phiano pattern) — long-term
Treat (chart, bar-range, rate) tuples as spaced-repetition items. Accuracy at
last attempt sets review interval; mastery decays over time; "due today"
queue on title screen. No rhythm game does this.

### 1.6 Drum-mute play-along — nearly free
.dtx keysounds are separate from BGM already. Expose chart-drum volume
slider: 100% / 50% / 0% ("you are the drummer" mode). Moises' killer feature,
free with our architecture. Optional far-future: demucs stem-split import for
arbitrary songs.

---

## 2. Play Options & Modifiers

### 2.1 Lane arrangement system — user idea #2
Arcade-authentic: GITADORA XG itself ships 3 lane-view configs (A/B/C differ
in pedal-lane placement). Key insight: **DTX channels already distinguish
everything worth splitting** — open HH (ch 18) vs closed (11), LP (1B/1C),
LC (1A), Ride (19). DTXMania merges them at *display* time only.

Design sketch: mapping layer `EChannel → DisplayLane`:

```
  chart channels          lane map (config)         display lanes
  ch11 HH-close ──┐
  ch18 HH-open ───┼──► preset "classic": merge ──►  [HH][SD][BD][HT][LT][CY]...
  ch19 Ride ──────┤
  ch1A LC ────────┤    preset "GM split": 1:1 ──►  [LC][HHc][HHo][SD][BD]
  ch1B/1C LP ─────┘                                 [LP][HT][LT][FT][CY][RD]
```

- Presets: `DTXMania classic`, `GITADORA XG A/B/C`, `Full split (GM)`,
  plus user custom (reorder / merge / split / per-lane width & color).
- Judgment stays per-channel; only display + input binding change.
- Input side must follow: `dtx-input` MIDI map targets channels, not lanes.
- Extends existing `dtx-config::LaneDisplay`.
- Editor UX can reuse HUD-editor interaction model (see §5) — drag lanes.

### 2.2 Floating hi-speed / "green number" (IIDX)
Set constant on-screen note travel time in ms; game auto-adjusts multiplier
per BPM (and mid-song BPM changes). Best scroll QoL invented; huge for DTX
libraries with wild BPM variance. IIDX formula reference:
`ms ≈ 174000 / (bpm × hispeed)`. Offer both modes: classic multiplier +
floating.

### 2.3 Per-lane auto with skill modifiers (GITADORA)
Already have global autoplay. GITADORA precedent: auto LC+LP+FT → skill ×0,
auto both pedals → ×0.25, Mirror B → ×0.50. Per-lane auto = practice tool +
accessibility, skill multiplier keeps leaderboards honest.

### 2.4 Mods system with presets (lazer model)
- Continuous rate mods with pitch toggle (not fixed DT/HT).
- Sudden/Hidden (note lift/cover — currently missing, only `dark_mode`).
- Judge adjust (Etterna J1–J9 / Quaver): stricter windows selectable,
  leaderboard standardizes on canonical judge, score records judge used.
- Named mod presets per user (lazer). Modifier → score multiplier so scores
  stay comparable (Beat Saber model).
- Fun tier later: Wind Up/Down (rate ramp), Adaptive Speed.

---

## 3. Input / e-drum Depth (differentiator — nobody does this well)

### 3.1 MIDI mapping preset library
Ship presets for TD-07/TD-17/TD-27, Alesis Nitro/Strike, Millenium, etc.
(Clone Hero gist + Beatlii's 100+ presets prove demand). Per-note velocity
thresholds for crosstalk filtering (CH pattern).

### 3.2 Velocity → ghost/accent judging (YARG "Elite Drums" direction)
.dtx chips carry volume; e-drum MIDI carries velocity 1–127. Grade dynamics:
ghost-note and accent consistency. Beatlii/Herta prove demand; no PC rhythm
game grades dynamics today.

### 3.3 Hi-hat openness via CC#4
Convention: 127≈closed → 0≈open; thresholds MUST be user-tunable (TD-17 caps
CC4 at 90; entry Alesis sends distinct notes instead). Pairs directly with
§2.1 open/close HH lane split. Pedal chick/splash = separate notes on better
kits.

### 3.4 Cymbal choke (polyphonic aftertouch) + CC#16 positional sensing
Choke: standard on CH. Positional (Roland flagship, CC16 center→edge):
**no game or VST consumes it** — open niche (rimshot/center judging, tone
feedback).

### 3.5 Three-way calibration wizard + post-play offset suggestion
Rhythm Quest devlog 10/14 theory: audio, input, visual latency are three
independent quantities — keep three knobs (we have `input_offset_ms`,
`bgm_adjust_ms`; add visual). Wizard: tap-to-audio-only (eyes closed) +
tap-to-visual-only (muted), derive all three. Post-play: "average hit error
−12 ms — apply?" one-click (YARG pattern, lowest-friction found). Per-song
offset override suggested from that song's hit-error history (lazer).

---

## 4. Feedback & Analytics

### 4.1 Hit error meter + Unstable Rate (osu)
Live hit-error bar in HUD; UR = stddev(errors) × 10. Judge already computes
deltas — cheap.

### 4.2 Results v2: timing histogram + per-section breakdown
- Hit-error distribution with hit-window boundaries overlaid + fitted curve
  (osu refinements from their issue tracker).
- FAST/SLOW counts (Project Sekai) for self-diagnosing offset.
- Per-section grades (Beatlii pattern) with one-key "practice this section"
  jump into §1.1 with A/B preset. This closes the play→analyze→drill loop.

### 4.3 Per-lane / per-limb analytics (Herta pattern — standout find)
Per-pad: signed error histograms, early/late bias, velocity distribution.
Map lanes→limbs; report weak-hand vs strong-hand consistency, limb-pair sync
(HH+BD, HH+SD). Weak-spot heatmap over chart timeline → feeds SRS queue
(§1.5). All derivable from existing judge data + velocity.

### 4.4 Sessions / goals / rivals (Tachi model)
Auto-group plays into sessions ("+12 PBs, +0.3 skill"); declarative goals
("clear 10% of 7.x folder"); rivals. Cheapest path: emit Tachi BATCH-MANUAL
JSON — Kamaitachi already models GITADORA-family drums.

---

## 5. HUD / Layout Editor — user idea #3
Copy lazer's model, scoped to arrangement only (their launch scope too):
- Hotkey overlays editor on **live running scene** (WYSIWYG, no separate app).
- Component list sidebar (score, combo, gauge, hit-error meter, judgement
  counts, song progress, skill panel...) — `dtx-ui` widgets are already
  components; externalize position/anchor to config.
- **Anchor + origin system**: two-dot model, snap to corners/edges/midpoints.
- **Two layers**: screen-anchored HUD vs **playfield-anchored** (element
  follows lane stack — critical once §2.1 makes lane count variable).
- Layout = JSON (or TOML alongside config), per scene (gameplay / song
  select / results), import/export as archive later.
- Build order: after §2.1 — playfield geometry must stabilize first.

---

## 6. Progression & Scoring

### 6.1 GITADORA-authentic skill (upgrade existing `skill.rs`)
- Achievement (current formula): `Perfect%×0.85 + Great%×0.35 + Combo%×0.05
  + PhraseComboRate×0.10`.
- Song skill = `rate × level × 20 × modifier` (auto/mirror penalties §2.3).
- **Total = best 25 HOT + best 25 OTHER** songs.
- Rank: SS ≥95, S ≥80, A ≥73, B ≥63. Lamps on wheel: clear / FC (yellow) /
  EXCELLENT all-perfect (rainbow). Level folders in 0.1 steps; lamp matrix
  per folder (Tachi grid) is the motivation engine.

### 6.2 Retention layer
Stars/mastery per chart (Yousician), streaks, daily challenge (lazer 2024:
one curated chart/day, streak tiers, percentile stats). 3 favorite folders
(GITADORA red/blue/green) — song select already has favorites groundwork.

---

## 7. Ecosystem (longer horizon, but decide formats early)

### 7.1 Chart hash identity + open score/replay format — decide EARLY
Hash chart note-data as canonical ID (BeatSaver model — makes leaderboards,
replays, downloads all line up). Replay = timestamped pad hits + judgments
(bytes/note; BSOR-style open spec). Unlocks: score verification, web replay
viewers, "watch #1 player hit this fill", ghost battles vs replay data
(results screen already has ghost bars).

### 7.2 Replay system
Record every play automatically (inputs are tiny). lazer replay UX: pause,
seek, ±5 s arrows, frame-step. Reuses practice-mode seek plumbing (§1.1).

### 7.3 In-game chart editor + auto-chart import (the ecosystem bet)
DTXCreator = Windows-only WinForms legacy; ecosystem bottleneck. Quaver
proves in-client editor with waveform, live difficulty recalc, playtest-from-
cursor, publish loop. Killer DTX-specific feature: demucs drum-stem waveform
behind the lanes while charting.
Auto-chart: drums are the *best* case in audio→chart research (STRUM: onset
F1 0.838; AutoChartDTX already exists for DTX; Omnizart pip CLI). Honest
framing: "import song → draft opens in editor → hand-fix" — Beat Sage proved
the demand; label generated charts, exclude from ranked.

### 7.4 Streamer/spectator
Local WebSocket/JSON telemetry endpoint (now-playing, combo, acc) + stock
OBS browser-source overlays — as game authors we skip the memory-reading
hacks (StreamCompanion/tosu exist only because osu! exposes nothing).
Discord Rich Presence via `discord-rich-presence` crate (lazer's impl is a
small readable reference).

### 7.5 Multiplayer (last)
Start with **async playlists** (lazer "timeshift": room + chart list + open
window, async scores on room leaderboard — near-zero infra). Realtime
lobbies (Quaver: 16p FFA/Teams) much later. Community IR precedent:
chanmori.net DTX IR.

---

## Suggested build order

```
Foundation:  seek in dtx-timing ──► 1.1 practice core ──► 1.2/1.3/1.4 trainers
                                          │
Parallel QoL (independent, any time): 2.2 floating hi-speed, 3.5 calibration,
                                      4.1 hit-error meter, 4.2 results v2
                                          │
Identity:    2.1 lane mapping layer ──► 5. HUD editor (needs stable playfield)
                                          │
Depth:       3.x e-drum stack (velocity, CC4, choke) — pairs with 2.1 split lanes
                                          │
Progression: 6.1 skill/lamps ──► 4.3 analytics ──► 4.4 Tachi ──► 1.5 SRS
                                          │
Ecosystem:   7.1 formats (decide early!) ──► 7.2 replays ──► 7.3 editor+AI ──►
             7.4 streaming ──► 7.5 multiplayer
```

Cross-cutting early decisions (cheap now, expensive later):
1. Chart hash ID + replay format (§7.1) — affects score.ini successor.
2. Pitch-preserving time-stretch in audio engine (§1.1/§2.4).
3. Channel→lane indirection (§2.1) — everything display-side should consume
   DisplayLane, never raw channel.
4. Three separate offset knobs (§3.5).
