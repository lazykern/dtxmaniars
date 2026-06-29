# Bevy patterns (project-specific)

~50 lines. Bevy docs via `ctx7`. This file = opinionated project rules only.

## Versions

- Bevy **0.19** (Rust 1.95+)
- `bevy` with `default-features = false` in plugins; enable features explicitly
- `bevy_kira_audio` (M1+) — never raw `kira` directly
- `bevy_framepace` (M2+) — smooth input latency
- `bevy_tweening` (djeedai) — M5+ UI animations, easing lenses (`TweeningPlugin`)
  - Pinned to git rev `5e3d0c9` (PR #170, merged 2026-06-28; bevy 0.19). No crates.io 0.16 yet — swap rev → version when 0.16 ships.
  - Note: ADR-0007 still names `bevy_tween` (multirious); that crate is Bevy 0.18 only and **not used**. Standard: `bevy_tweening` (djeedai). See `docs/BEVY_UX_UI.md` §6 for crate matrix.

## Plugin organization

```rust
// crates/gameplay-drums/src/scroll.rs
use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, scroll_notes.in_set(DrumsSet::Scroll));
}

fn scroll_notes(time: Res<Time>, mut q: Query<&mut Transform, With<Note>>) { /* ... */ }
```

One plugin fn per file. Empty plugins: omit. Sub-plugins aggregated in parent `mod.rs`.

## Screens as States

```rust
#[derive(States, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub enum Screen { #[default] Boot, Title, SongSelect, Loading, Playing, Result, Exit }

// spawn with:
commands.spawn((Name::new("Title"), StateScoped(Screen::Title), /* ... */));
```

Use `StateScoped` (bevy 0.14+) for cleanup. No manual despawn.

## Fluidity constants (DTXManiaNX baseline — ADR-0010)

```rust
pub const SCREEN_FADE_MS: u32 = 1500;  // StageManager.cs:29 FadeDurationMs = 1500f
pub const LOAD_HOLD_MS:   u32 = 0;     // DTXManiaNX has no min load hold
pub const INPUT_LATENCY_MS: u32 = 16;  // bevy_framepace target
```

**These are the DTXManiaNX baseline values, NOT osu-lazer aspirational ones.**
"osu-lazer-grade fluidity" is the M6+ destination. See ADR-0010.

## Events (decoupling)

```rust
#[derive(Event)] pub struct LaneHit { pub lane: u8, pub time_ms: i64 }
#[derive(Event)] pub struct JudgmentHit { pub kind: JudgmentKind, pub delta_ms: i64 }

// Writers: gameplay-drums
// Readers: dtx-scoring, dtx-audio, dtx-ui
```

EventWriters before EventReaders in the frame. Use SystemSet ordering, not `.before()` chains.

## Assets

```rust
#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct SkinAssets {
    #[dependency] pub note: Handle<Image>,
    #[dependency] pub hit_lane: Handle<Image>,
    #[dependency] pub judgement: Handle<Image>,
}
```

Preload in `OnEnter(Loading)`. Block screen exit until handles ready.

## Animation

Use **`bevy_tweening`** (djeedai). Pinned to git rev `5e3d0c9` in
`[workspace.dependencies]`, pulled by `dtx-ui`; swap to crates.io 0.16 when
published. Add `TweeningPlugin` in `dtx-ui/src/lib.rs` when first tween lands.

```rust
// bevy_tweening — M5+
use bevy_tweening::{Tween, EaseFunction, lens::TextColorLens};
Tween::new(
    TextColorLens { start: Color::WHITE, end: Color::NONE },
    Duration::from_millis(300),
    EaseFunction::QuadraticOut,
)
```

Easing: `EaseFunction` enum or custom via `EaseMethod::CustomFunction`.

**Hand-rolled tween:** `dtx-ui::tween::ScalarTween` for v1 fade/gauge/lane-flush
(see `BEVY_UX_UI.md` §6.2). Will be replaced by `bevy_tweening` lenses as they
land.

**JSON keyframe loader = NOT in v1.** Hardcode animations in Rust. See ADR-0007.
BocuD `AnimationClip` JSON port deferred to M5+ skin system.

## Audio clock

**Never** judge on `Time::delta()`. Use `dtx-timing::AudioClock` (resource holding elapsed ms from BGM playback). See ADR-0002.