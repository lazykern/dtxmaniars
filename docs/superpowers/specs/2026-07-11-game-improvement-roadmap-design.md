# DTXManiaRS Player-Impact Improvement Roadmap

Date: 2026-07-11
Status: Approved
Priority: Player impact, trust-first
Scope: Drums and electronic drums

## Goal

Make DTXManiaRS trustworthy on first setup, fast for daily play, and useful for skill improvement. Preserve port-first DTX mechanics. Improve surrounding UX without adding live-service scope.

## Current Baseline

Existing code already provides:

- Drum gameplay, scoring, gauge, practice seek/loop/ramp, and results.
- Type-to-search song selection.
- Keyboard and MIDI menu navigation with connected-device state.
- Canonical chart identity and versioned local score storage.
- Local play-history design ready for implementation.
- 1,377 passing workspace tests in local validation.

Old research notes that describe practice, search, or MIDI navigation as absent are stale. Plans must inspect current source before filing work.

## Product Flow

```text
Kit or keyboard
      |
      v
Readiness + calibration
      |
      v
Song select ---- history, filters, remembered chart
      |
      v
Drums performance ---- BGA image playback
      |
      v
Results ---- retry
      |
      +---- practice weakest section
                    |
                    v
             existing loop/ramp
```

## Roadmap

### Phase 1: Restore Player Trust

#### 1. Guided calibration and diagnostics

Expand the existing input-offset tap test into one guided flow for input, audio, and visual timing. Show active MIDI device, sample spread, estimated offset, refresh rate, and frame-time spikes. Keep manual adjustment. Do not auto-apply a low-confidence estimate.

Evidence:

- osu! ships an Offset Wizard for device-level timing.
- YARG separates audio and video offsets and documents 20–40 ms as perceptible.
- DTXManiaRS already has input and BGM offsets plus an input tap-test foundation.

#### 2. Correct normal play-speed behavior

`ScrollSettings::play_speed` currently compresses chart time without rescaling audio. Resolve the contract before exposing it as a normal setting: port NX time-stretch behavior or restrict the control so players cannot create audio/chart desync.

Repo evidence: `crates/gameplay-drums/src/resources.rs:229-232`.

#### 3. Real BGA images

Replace colored BGA placeholders with chart image assets while preserving BocuD event and layer semantics. Keep video decode as separate later work because it adds platform and dependency risk.

Repo evidence: `crates/dtx-bga/src/lib.rs:3-6`, `crates/dtx-bga/src/lib.rs:85-90`.

#### 4. Honest settings surface

Wire each exposed gameplay toggle before showing it. Hide any setting still lacking a runtime consumer. Current in-progress settings work takes precedence over audit findings.

#### 5. Guitar non-goal

Do not spend roadmap capacity completing guitar mode. Hide it or label it experimental if players can reach its incomplete mechanics.

### Phase 2: Shorten Return to Play

#### 1. Play history

Implement the approved play-history panel from `docs/superpowers/specs/2026-07-11-play-history-panel-design.md`. Reuse the current score store and chart identity. Add no schema or dependency unless the approved plan requires it.

#### 2. Library filters and collections

Keep current type-to-search. Add favorites, played/unplayed, clear state, difficulty range, recent additions, near-my-level, and random-within-filter. Preserve remembered song and difficulty.

Instrument scan duration, chart count, and parse failures before adding cache or async machinery. Current chart-stat lookup reparses files on demand at `crates/dtx-library/src/lib.rs:105-123`. Add caching only when representative libraries show a visible delay.

#### 3. Atomic score persistence

Write score JSON to a sibling temporary file, flush it, then rename it over the destination. Preserve the last valid file on any failure. Current direct overwrite lives at `crates/dtx-scoring/src/store.rs:190-205`.

### Phase 3: Close the Learning Loop

#### 1. Actionable results

Add:

- signed early/late histogram;
- timing spread;
- per-lane misses and bias;
- weak-section timeline;
- personal-best delta.

Keep score and rank visible, but make diagnosis understandable without knowing rhythm-game jargon.

#### 2. One-action practice handoff

Results offers `Practice weakest section`. It creates a practice intent with section, lead-in, tempo, and reason, then enters the existing practice loop. Practice attempts never write normal scores.

#### 3. Pitch-preserving practice tempo

Keep current practice UX and domain model. Replace pitch-shifting playback-rate behavior when a suitable audio backend proves viable. Do not redesign practice again.

### Phase 4: Accessibility and Retention

#### 1. Shared control accessibility

Update shared slider, stepper, and toggle constructors rather than patching each screen. Add keyboard focus, visible focus state, semantic name/value, and larger targets. Current slider and toggle heights are 14 px and 16 px at `crates/dtx-ui/src/widget/controls.rs:74-190`.

Add high-contrast presentation, color-plus-shape cues, reduced flashes/background dimming, scalable HUD text, and No Fail where mechanics allow it.

#### 2. Local retention features

After history and results analytics prove useful, add local weekly goals, PB ghosts, and shareable result cards. Start without accounts or backend services.

## Parallel Engineering Lane

Player work does not excuse a broken release gate.

1. Fix `cargo fmt --all -- --check` on intended changes.
2. Resolve current `clippy -D warnings` failures in `dtx-audio` with the smallest justified change.
3. Run workspace tests in low-memory CI package groups with `-j 2`; CI currently omits tests despite its job name (`.github/workflows/ci.yml:13-46`).
4. Restore or replace missing roadmap, architecture, Bevy-pattern, and decision-document links in `AGENTS.md`.
5. Remove Pure-to-Engine dependency violations before adding more coupling:
   - `dtx-core` depends on `dtx-timing` in `crates/dtx-core/Cargo.toml:13-16`.
   - `dtx-config` depends on `dtx-input` in `crates/dtx-config/Cargo.toml:13-18`.
6. Define one supported release target and produce a versioned release artifact before adding a platform matrix.

## Success Checks

- A new player reaches a correctly calibrated first drum performance without external docs.
- Calibration preserves manual control and rejects low-confidence auto-apply.
- Song selection exposes routine actions without hidden key chords.
- A completed score survives simulated interruption during replacement.
- Library scan reports duration and failures; caching follows measured need.
- Results identify timing bias and weakest lane or section.
- One action starts practice at the recommended section with lead-in.
- Gameplay sustains the selected refresh target on named reference hardware; frame-time overruns are visible.
- Keyboard and MIDI can complete every routine flow.
- CI enforces package-group tests alongside format and clippy.

## Failure Handling

- Skip malformed charts, show failure count, and log paths and reasons.
- Show an honest fallback for unsupported BGA video.
- Preserve prior score/config files when replacement fails.
- Preserve calibration samples across device disconnect and ask the player to retry.
- Keep practice, assists, and modified-speed plays out of normal score records.
- Keep unavailable features hidden rather than exposing inert controls.

## Verification

- Pure unit tests for calibration estimation, confidence, atomic persistence, filters, and weak-section analysis.
- Headless Bevy flow tests for results to practice and keyboard/MIDI parity.
- Fixture charts for BGA layers, missing assets, malformed assets, and chart parse failures.
- Manual matrix with keyboard, at least two MIDI drum modules, and 60/120/144 Hz displays.
- Representative small and large song libraries for startup and browse timing.
- Existing workspace suite remains green.

## Non-Goals

- Guitar completion.
- Live multiplayer or matchmaking.
- Hosted song marketplace or subscription.
- AI chart generation.
- New database/cache before scan measurements justify it.
- Full video decode in the BGA-image change.
- Another practice-mode redesign.

## Research Basis

Primary or project-maintained sources:

- osu! Offset Wizard: <https://osu.ppy.sh/wiki/en/Client/Options/Offset_Wizard>
- osu! Unstable Rate: <https://osu.ppy.sh/wiki/en/Gameplay/Unstable_rate>
- osu! performance troubleshooting: <https://osu.ppy.sh/wiki/en/Performance_troubleshooting>
- YARG calibration: <https://wiki.yarg.in/wiki/Help:Calibration>
- YARG music library: <https://wiki.yarg.in/wiki/Music_Library>
- Taiko 2025 Free Training and Song Search update: <https://dondafulfestival-20th.taiko-ch.net/qa/update.php>
- Melodics Practice Mode: <https://support.melodics.com/en/articles/6777027-practice-mode>
- Microsoft Xbox Accessibility Guideline 103: <https://learn.microsoft.com/en-us/xbox/accessibility/xbox-accessibility-guidelines/103>

These sources prove feature adoption and product convergence, not causal retention. Validate priority assumptions with 8–12 target players split across experienced DTX users, electronic drummers, and keyboard players.
