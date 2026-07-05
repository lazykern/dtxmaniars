# GITADORA Menu UI Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild Title, Song Select, Settings and Song Loading screens with a GITADORA-style visual language (plain black stage, yellow selection, big skill numbers — no decorative streaks or gradients, per user revision) animated with osu-grade motion, on top of a reusable menu kit in `dtx-ui`.

**Architecture:** New pure-math motion primitives (`SpringValue`, `RollingNumber`, `BeatPulse`, `EnterChoreo`) and stage widgets (`stage_background`, `stage_panel`, `density_graph`, `difficulty_grid`, `song_wheel`) live in `dtx-ui`. `game-menu` screens are rebuilt on the kit. Existing `AppState` machine, 300ms `ScreenFade`, preview audio, folder grouping and score.ini persistence stay untouched.

**Tech Stack:** Bevy 0.19 (`UiTransform`/`Val2`; `BoxShadow::new`; `BorderColor::all`), hand-rolled tweens (`ScalarTween`, `EaseFunction`), `AsyncComputeTaskPool` for chart-stat parsing.

**Spec:** `docs/superpowers/specs/2026-07-05-menu-ui-gitadora-redesign-design.md`

**Verified API facts (do not re-derive):**
- `UiTransform { translation: Val2, scale: Vec2, rotation: Rot2 }`, `Val2::px(x, y)`. Inserting `UiTransform` auto-inserts `UiGlobalTransform`.
- `BoxShadow::new(color, x_offset: Val, y_offset: Val, spread_radius: Val, blur_radius: Val)`.
- `BorderColor::all(color)`; `Node { border: UiRect, .. }`.
- Bevy 0.19 renamed events: `MessageWriter`/`MessageReader`/`add_message`. Text: `Text::new`, `TextFont { font: FontSource::SansSerif, font_size: FontSize::Px(..) }`, `TextColor`.
- No `despawn_recursive` in 0.19 — `game_shell::despawn_stage::<T>` despawns marked entities (children die with parents when spawned via `with_children`/`children![]`... **no**: `despawn_stage` only despawns the marked entity; Bevy 0.19 `despawn` despawns related children by default via relationships. `song_select.rs` has a manual recursive helper — keep using `despawn_stage::<T>` for single roots (children are despawned automatically through the `Children` relationship in 0.19; the manual helper in song_select predates that and can be dropped when the file is rebuilt).
- Drum channels (`dtx_core::channel::EChannel`): `LeftCymbal, HiHatClose, HiHatOpen, LeftPedal, LeftBassDrum, Snare, BassDrum, HighTom, LowTom, FloorTom, Cymbal, RideCymbal`; `chip.channel.is_drum()` exists.
- `dtx_scoring::score_ini::{score_ini_path, read_best}` → `Option<DrumScoreIni>` with `score, rank: String, max_combo, play_count, clear_count` and `accuracy() -> f32` (0..100).
- `SongInfo { path, title, artist, bpm: Option<f32>, dlevel: Option<u32>, bgm_path, preview_path, preview_is_loopable, preimage_path }`; `notes_total()` re-parses the DTX from disk (slow — do NOT call per frame; Task 10's async stats replace its use in song select).

**Conventions for every task:**
- Run tests with `cargo test -p <crate>` from repo root.
- Commit after each green step; message style follows repo history (`feat(ui): ...`, `feat(song-select): ...`). No AI co-author lines.
- Do not add comments that merely narrate code; keep comment density of surrounding files.
- All user-facing text for the config screen says "Settings".

---

### Task 1: GITADORA palette in theme.rs

**Files:**
- Modify: `crates/dtx-ui/src/theme.rs`

- [ ] **Step 1: Write failing tests** — append to the `tests` module in `crates/dtx-ui/src/theme.rs`:

```rust
    #[test]
    fn stage_palette_black_bg_yellow_select() {
        let t = Theme::default();
        assert!(t.stage_bg.to_srgba().red < 0.05);
        let y = t.select_yellow.to_srgba();
        assert!(y.red > 0.9 && y.green > 0.7 && y.blue < 0.1);
    }

    #[test]
    fn difficulty_colors_distinct() {
        let t = Theme::default();
        let all = [
            t.difficulty_color(0),
            t.difficulty_color(1),
            t.difficulty_color(2),
            t.difficulty_color(3),
        ];
        for i in 0..4 {
            for j in (i + 1)..4 {
                assert_ne!(all[i], all[j]);
            }
        }
        // out-of-range clamps to MASTER color
        assert_eq!(t.difficulty_color(9), t.difficulty_color(3));
    }

    #[test]
    fn lane_colors_cover_nine_display_lanes() {
        let t = Theme::default();
        assert_eq!(t.lane_colors().len(), 9);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p dtx-ui theme`
Expected: FAIL — `no field stage_bg`, `no method difficulty_color`.

- [ ] **Step 3: Implement** — add fields to `Theme` struct, values to `Default`, and methods to `impl Theme`:

```rust
// fields appended to pub struct Theme:
    pub stage_bg: Color,
    pub stage_panel_bg: Color,
    pub stage_panel_border: Color,
    pub select_yellow: Color,
    pub clear_green: Color,
    pub skill_bar_fill: Color,

// appended inside Default::default() Self { ... }:
            stage_bg: Color::srgb(0.02, 0.02, 0.02),                 // #050505
            stage_panel_bg: Color::srgba(0.051, 0.051, 0.051, 0.93), // #0d0d0dee
            stage_panel_border: Color::srgb(0.267, 0.267, 0.267),    // #444444
            select_yellow: Color::srgb(1.0, 0.8, 0.0),               // #ffcc00
            clear_green: Color::srgb(0.0, 0.8, 0.533),               // #00cc88
            skill_bar_fill: Color::srgb(1.0, 0.8, 0.0),

// methods appended to impl Theme:
    /// BASIC / ADVANCED / EXTREME / MASTER colors; >=3 clamps to MASTER.
    pub fn difficulty_color(&self, difficulty: u8) -> Color {
        match difficulty {
            0 => Color::srgb(0.0, 0.533, 1.0),  // #0088ff BASIC
            1 => Color::srgb(1.0, 0.8, 0.0),    // #ffcc00 ADVANCED
            2 => Color::srgb(1.0, 0.267, 0.267), // #ff4444 EXTREME
            _ => Color::srgb(0.8, 0.2, 1.0),    // #cc33ff MASTER+
        }
    }

    /// Density-graph display lanes (LC HH LP SD HT BD LT FT CY), GITADORA order
    /// (matches gameplay-drums lane_geometry::COLUMNS: HT before BD).
    pub fn lane_colors(&self) -> [Color; 9] {
        [
            Color::srgb(0.8, 0.2, 1.0),   // LC purple
            Color::srgb(0.0, 0.667, 1.0), // HH blue
            Color::srgb(1.0, 0.4, 0.8),   // LP pink
            Color::srgb(1.0, 0.8, 0.0),   // SD yellow
            Color::srgb(0.0, 0.8, 0.4),   // HT green
            Color::srgb(0.6, 0.6, 0.65),  // BD gray
            Color::srgb(1.0, 0.267, 0.267), // LT red
            Color::srgb(1.0, 0.533, 0.0), // FT orange
            Color::srgb(0.0, 0.867, 0.867), // CY cyan
        ]
    }
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p dtx-ui theme`
Expected: PASS (all theme tests, old + new).

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-ui/src/theme.rs
git commit -m "feat(ui): GITADORA stage palette in theme"
```

---

### Task 2: SpringValue motion primitive

**Files:**
- Create: `crates/dtx-ui/src/motion.rs`
- Modify: `crates/dtx-ui/src/lib.rs` (add `pub mod motion;`)

- [ ] **Step 1: Create `crates/dtx-ui/src/motion.rs` with failing tests**

```rust
//! Menu-kit motion primitives (spec 2026-07-05): spring scroll,
//! rolling numbers, beat pulse, staggered enter choreography.

use bevy::prelude::*;

/// Critically-damp-able spring toward `target`. Tick with real dt.
#[derive(Component, Debug, Clone)]
pub struct SpringValue {
    pub value: f32,
    pub target: f32,
    pub velocity: f32,
    /// Stiffness (1/s^2). Higher = snappier.
    pub stiffness: f32,
    /// Damping ratio. 1.0 = critical (no overshoot), <1 overshoots.
    pub damping_ratio: f32,
}

impl SpringValue {
    pub fn new(value: f32, stiffness: f32, damping_ratio: f32) -> Self {
        Self {
            value,
            target: value,
            velocity: 0.0,
            stiffness,
            damping_ratio,
        }
    }

    /// Song-wheel default: slight overshoot, settles in ~250ms.
    pub fn wheel(value: f32) -> Self {
        Self::new(value, 400.0, 0.82)
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Semi-implicit Euler integration; clamps dt to avoid explosion
    /// on hitches. Snaps when close and slow.
    pub fn tick(&mut self, dt_s: f32) {
        let dt = dt_s.clamp(0.0, 1.0 / 30.0);
        let omega = self.stiffness.sqrt();
        let accel = -self.stiffness * (self.value - self.target)
            - 2.0 * self.damping_ratio * omega * self.velocity;
        self.velocity += accel * dt;
        self.value += self.velocity * dt;
        if (self.value - self.target).abs() < 0.0005 && self.velocity.abs() < 0.01 {
            self.value = self.target;
            self.velocity = 0.0;
        }
    }

    pub fn settled(&self) -> bool {
        self.value == self.target && self.velocity == 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(spring: &mut SpringValue, seconds: f32) {
        let steps = (seconds / 0.016).ceil() as usize;
        for _ in 0..steps {
            spring.tick(0.016);
        }
    }

    #[test]
    fn spring_settles_on_target() {
        let mut s = SpringValue::wheel(0.0);
        s.set_target(5.0);
        run(&mut s, 2.0);
        assert!(s.settled(), "value={} vel={}", s.value, s.velocity);
        assert_eq!(s.value, 5.0);
    }

    #[test]
    fn spring_underdamped_overshoots() {
        let mut s = SpringValue::new(0.0, 400.0, 0.5);
        s.set_target(1.0);
        let mut max = 0.0f32;
        for _ in 0..200 {
            s.tick(0.016);
            max = max.max(s.value);
        }
        assert!(max > 1.0);
    }

    #[test]
    fn spring_moves_toward_target_immediately() {
        let mut s = SpringValue::wheel(0.0);
        s.set_target(1.0);
        s.tick(0.016);
        assert!(s.value > 0.0);
    }

    #[test]
    fn spring_clamps_huge_dt() {
        let mut s = SpringValue::wheel(0.0);
        s.set_target(1.0);
        s.tick(10.0); // hitch — must not explode
        assert!(s.value.is_finite() && s.value.abs() < 10.0);
    }
}
```

- [ ] **Step 2: Register module** — in `crates/dtx-ui/src/lib.rs` add `pub mod motion;` to the module list (after `pub mod easing;`).

- [ ] **Step 3: Run tests**

Run: `cargo test -p dtx-ui motion`
Expected: PASS (implementation is written with the tests in this file layout; if any assert fails, fix `tick` math, not the test).

- [ ] **Step 4: Commit**

```bash
git add crates/dtx-ui/src/motion.rs crates/dtx-ui/src/lib.rs
git commit -m "feat(ui): SpringValue motion primitive"
```

---

### Task 3: RollingNumber + BeatPulse primitives

**Files:**
- Modify: `crates/dtx-ui/src/motion.rs`

- [ ] **Step 1: Append failing tests** to `motion.rs` `tests` module:

```rust
    #[test]
    fn rolling_number_approaches_target() {
        let mut r = RollingNumber::new(0.0);
        r.set_target(100.0);
        for _ in 0..30 {
            r.tick(0.016);
        }
        assert!(r.shown > 50.0 && r.shown <= 100.0);
        for _ in 0..300 {
            r.tick(0.016);
        }
        assert!((r.shown - 100.0).abs() < 0.01);
    }

    #[test]
    fn rolling_number_snaps_when_close() {
        let mut r = RollingNumber::new(99.999);
        r.set_target(100.0);
        r.tick(0.016);
        assert_eq!(r.shown, 100.0);
    }

    #[test]
    fn beat_pulse_scale_peaks_on_beat() {
        let mut p = BeatPulse::new(60.0, 0.08);
        // At phase 0 (on-beat) scale is max.
        assert!((p.scale() - 1.08).abs() < 0.001);
        p.tick(0.5); // half a beat at 60bpm
        assert!(p.scale() < 1.04);
        p.tick(0.5); // full beat — wraps to peak
        assert!((p.scale() - 1.08).abs() < 0.01);
    }

    #[test]
    fn beat_pulse_bpm_change_keeps_phase_bounded() {
        let mut p = BeatPulse::new(157.0, 0.05);
        for _ in 0..1000 {
            p.tick(0.016);
        }
        assert!(p.phase >= 0.0 && p.phase < 1.0);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p dtx-ui motion`
Expected: FAIL — `RollingNumber` / `BeatPulse` not found.

- [ ] **Step 3: Implement in `motion.rs`** (above the tests module):

```rust
/// Exponential approach for numeric readouts (skill, BPM, notes).
#[derive(Component, Debug, Clone)]
pub struct RollingNumber {
    pub shown: f32,
    pub target: f32,
    /// Fraction of remaining distance closed per second (~10 = fast).
    pub rate: f32,
}

impl RollingNumber {
    pub fn new(value: f32) -> Self {
        Self {
            shown: value,
            target: value,
            rate: 10.0,
        }
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    pub fn tick(&mut self, dt_s: f32) {
        let diff = self.target - self.shown;
        if diff.abs() < 0.005 {
            self.shown = self.target;
            return;
        }
        self.shown += diff * (self.rate * dt_s).min(1.0);
    }
}

/// BPM-synced pulse in [1.0, 1.0+amplitude]; peak on the beat, decays
/// across the beat interval (cos falloff).
#[derive(Component, Debug, Clone)]
pub struct BeatPulse {
    pub bpm: f32,
    /// Beat phase in [0, 1).
    pub phase: f32,
    pub amplitude: f32,
}

impl BeatPulse {
    pub fn new(bpm: f32, amplitude: f32) -> Self {
        Self {
            bpm: bpm.max(1.0),
            phase: 0.0,
            amplitude,
        }
    }

    pub fn tick(&mut self, dt_s: f32) {
        let beats_per_s = self.bpm / 60.0;
        self.phase = (self.phase + dt_s * beats_per_s).rem_euclid(1.0);
    }

    /// 1.0+amplitude at phase 0, easing back to ~1.0 by phase 1.
    pub fn scale(&self) -> f32 {
        let falloff = (1.0 - self.phase).powi(2);
        1.0 + self.amplitude * falloff
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p dtx-ui motion`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-ui/src/motion.rs
git commit -m "feat(ui): RollingNumber and BeatPulse primitives"
```

---

### Task 4: EnterChoreo + motion systems registered in plugin

**Files:**
- Modify: `crates/dtx-ui/src/motion.rs`
- Modify: `crates/dtx-ui/src/lib.rs`

- [ ] **Step 1: Append failing tests** to `motion.rs`:

```rust
    #[test]
    fn enter_choreo_waits_for_delay_then_progresses() {
        let mut c = EnterChoreo::slide(Vec2::new(-40.0, 0.0), 60.0, 200.0);
        c.tick(30.0);
        assert_eq!(c.progress(), 0.0);
        c.tick(130.0); // 160ms total = 100ms into 200ms anim
        let p = c.progress();
        assert!(p > 0.0 && p < 1.0);
        c.tick(500.0);
        assert_eq!(c.progress(), 1.0);
        assert!(c.finished());
    }

    #[test]
    fn enter_choreo_offset_shrinks_to_zero() {
        let mut c = EnterChoreo::slide(Vec2::new(-40.0, 0.0), 0.0, 200.0);
        assert_eq!(c.current_offset(), Vec2::new(-40.0, 0.0));
        c.tick(1000.0);
        assert_eq!(c.current_offset(), Vec2::ZERO);
    }

    #[test]
    fn choreo_system_moves_ui_transform() {
        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin);
        app.add_systems(Update, enter_choreo_system);
        let e = app
            .world_mut()
            .spawn((
                EnterChoreo::slide(Vec2::new(-40.0, 0.0), 0.0, 200.0),
                UiTransform::default(),
            ))
            .id();
        app.update();
        app.update();
        let tf = app.world().get::<UiTransform>(e).unwrap();
        // after two ticks the node moved off its start offset
        assert_ne!(tf.translation, Val2::px(-40.0, 0.0));
    }
```

Add `use bevy::app::App;` etc. inside the test module if not already imported (test module already has `use super::*;`; add `use bevy::ui::Val2;` at top of file imports as needed).

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p dtx-ui motion`
Expected: FAIL — `EnterChoreo` not found.

- [ ] **Step 3: Implement in `motion.rs`:**

```rust
use crate::easing::EaseFunction;

/// Staggered screen-enter animation: node starts at `offset` px and
/// slides to rest after `delay_ms`, over `duration_ms` with OutQuint.
#[derive(Component, Debug, Clone)]
pub struct EnterChoreo {
    pub offset: Vec2,
    pub delay_ms: f32,
    pub duration_ms: f32,
    pub elapsed_ms: f32,
    pub easing: EaseFunction,
}

impl EnterChoreo {
    pub fn slide(offset: Vec2, delay_ms: f32, duration_ms: f32) -> Self {
        Self {
            offset,
            delay_ms,
            duration_ms: duration_ms.max(0.001),
            elapsed_ms: 0.0,
            easing: EaseFunction::OutQuint,
        }
    }

    pub fn tick(&mut self, delta_ms: f32) {
        self.elapsed_ms += delta_ms;
    }

    /// 0 before delay, eased 0..1 across duration, 1 after.
    pub fn progress(&self) -> f32 {
        let t = ((self.elapsed_ms - self.delay_ms) / self.duration_ms).clamp(0.0, 1.0);
        self.easing.ease(t)
    }

    pub fn finished(&self) -> bool {
        self.elapsed_ms >= self.delay_ms + self.duration_ms
    }

    pub fn current_offset(&self) -> Vec2 {
        self.offset * (1.0 - self.progress())
    }
}

/// Drives every `EnterChoreo` + `UiTransform`; removes the component
/// when finished so idle nodes cost nothing.
pub fn enter_choreo_system(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut EnterChoreo, &mut UiTransform)>,
) {
    let dt_ms = time.delta_secs() * 1000.0;
    for (entity, mut choreo, mut tf) in &mut q {
        choreo.tick(dt_ms);
        let off = choreo.current_offset();
        tf.translation = Val2::px(off.x, off.y);
        if choreo.finished() {
            tf.translation = Val2::ZERO;
            commands.entity(entity).remove::<EnterChoreo>();
        }
    }
}

/// Drives every standalone `BeatPulse` + `UiTransform` scale.
pub fn beat_pulse_system(time: Res<Time>, mut q: Query<(&mut BeatPulse, &mut UiTransform)>) {
    for (mut pulse, mut tf) in &mut q {
        pulse.tick(time.delta_secs());
        let s = pulse.scale();
        tf.scale = Vec2::splat(s);
    }
}
```

Add `use bevy::ui::Val2;` to the file's imports.

- [ ] **Step 4: Register systems** — in `crates/dtx-ui/src/lib.rs` `plugin`, extend the `Update` tuple:

```rust
        .add_systems(
            Update,
            (
                widget::album_art::album_art_tween_system,
                widget::album_art::apply_album_art_opacity,
                parallax::parallax_info_tween_system,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (motion::enter_choreo_system, motion::beat_pulse_system),
        );
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p dtx-ui`
Expected: PASS (all dtx-ui tests).

- [ ] **Step 6: Commit**

```bash
git add crates/dtx-ui/src/motion.rs crates/dtx-ui/src/lib.rs
git commit -m "feat(ui): EnterChoreo and motion systems in plugin"
```

---

### Task 5: stage_background widget (plain black stage + ambient art)

Per user revision: **no streaks, no gradients** — the stage is a black fill with an optional ambient album-art layer under a dark overlay.

**Files:**
- Create: `crates/dtx-ui/src/widget/stage_background.rs`
- Modify: `crates/dtx-ui/src/widget/mod.rs`
- Modify: `crates/dtx-ui/src/lib.rs` (register system)

- [ ] **Step 1: Create the widget file with tests**

```rust
//! Fullscreen stage: black bg + optional ambient album-art layer
//! (osu-style tint) under a dark overlay. Deliberately minimal —
//! layout carries the design (spec revision 2026-07-05).

use bevy::prelude::*;

use crate::theme::Theme;

/// Fullscreen album-art tint under the dark overlay. `max_alpha`
/// caps opacity; entity also carries `crate::widget::album_art::AlbumArt`
/// so selection swaps crossfade it.
#[derive(Component, Debug, Clone, Copy)]
pub struct AmbientArt {
    pub max_alpha: f32,
}

/// Spawn the stage as children of `parent`. Layer order (back to
/// front): black fill, ambient art, dark overlay.
pub fn spawn_stage_background(parent: &mut ChildSpawnerCommands, theme: &Theme) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(theme.stage_bg),
    ));
    parent.spawn((
        AmbientArt { max_alpha: 0.30 },
        crate::widget::album_art::AlbumArt::default(),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        ImageNode {
            color: Color::WHITE.with_alpha(0.0),
            ..default()
        },
    ));
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(theme.stage_bg.with_alpha(0.55)),
    ));
}

/// Copy `AlbumArt.opacity` (crossfade tween) into the ambient image
/// alpha, capped at `max_alpha`. A hidden art (`Handle::default`)
/// stays fully transparent = black stage.
pub fn ambient_art_apply_system(
    mut q: Query<(&AmbientArt, &crate::widget::album_art::AlbumArt, &mut ImageNode)>,
) {
    for (ambient, art, mut image) in &mut q {
        let target = if image.image == Handle::default() {
            0.0
        } else {
            art.opacity * ambient.max_alpha
        };
        if (image.color.alpha() - target).abs() > 0.001 {
            image.color = image.color.with_alpha(target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_spawn_creates_ambient_layer() {
        let mut app = bevy::app::App::new();
        let theme = Theme::default();
        let world = app.world_mut();
        let mut commands = world.commands();
        commands.spawn(Node::default()).with_children(|p| {
            spawn_stage_background(p, &theme);
        });
        drop(commands);
        world.flush();
        let ambient = world.query::<&AmbientArt>().iter(&world).count();
        assert_eq!(ambient, 1);
    }
}
```

Note for the implementer: `ChildSpawnerCommands` is the Bevy 0.19 child-builder type used by `with_children(|p| ...)`. If the compiler names it differently in this workspace (check existing widgets in `dtx-ui/src/widget/` or `crates/gameplay-drums/src/hud.rs` for the established parent-builder parameter type), match the existing pattern.

- [ ] **Step 2: Register** — in `crates/dtx-ui/src/widget/mod.rs` add `pub mod stage_background;`. In `lib.rs` plugin, add to the second `add_systems(Update, ...)` tuple:

```rust
            (
                motion::enter_choreo_system,
                motion::beat_pulse_system,
                widget::stage_background::ambient_art_apply_system,
            ),
```

- [ ] **Step 3: Build + test**

Run: `cargo test -p dtx-ui`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/dtx-ui/src/widget/stage_background.rs crates/dtx-ui/src/widget/mod.rs crates/dtx-ui/src/lib.rs
git commit -m "feat(ui): plain stage background with ambient art layer"
```

---

### Task 6: stage_panel + skill badge helpers

**Files:**
- Create: `crates/dtx-ui/src/widget/stage_panel.rs`
- Modify: `crates/dtx-ui/src/widget/mod.rs`

- [ ] **Step 1: Create `stage_panel.rs`**

```rust
//! GITADORA panel chrome: dark bordered boxes, yellow selected
//! variant with glow, and label+big-number badge rows.

use bevy::prelude::*;

use crate::theme::Theme;

/// Base panel: #0d0d0dee fill, 1px #444 border.
pub fn panel(theme: &Theme, node: Node) -> impl Bundle {
    (
        Node {
            border: UiRect::all(Val::Px(1.0)),
            ..node
        },
        BackgroundColor(theme.stage_panel_bg),
        BorderColor::all(theme.stage_panel_border),
    )
}

/// Selected panel: yellow 2px border + glow.
pub fn selected_panel(theme: &Theme, node: Node) -> impl Bundle {
    (
        Node {
            border: UiRect::all(Val::Px(2.0)),
            ..node
        },
        BackgroundColor(theme.stage_panel_bg),
        BorderColor::all(theme.select_yellow),
        BoxShadow::new(
            theme.select_yellow.with_alpha(0.45),
            Val::Px(0.0),
            Val::Px(0.0),
            Val::Px(2.0),
            Val::Px(14.0),
        ),
    )
}

/// Apply/remove selection chrome on an existing panel entity.
pub fn set_panel_selected(
    theme: &Theme,
    selected: bool,
    border: &mut BorderColor,
    shadow: &mut BoxShadow,
) {
    if selected {
        *border = BorderColor::all(theme.select_yellow);
        *shadow = BoxShadow::new(
            theme.select_yellow.with_alpha(0.45),
            Val::Px(0.0),
            Val::Px(0.0),
            Val::Px(2.0),
            Val::Px(14.0),
        );
    } else {
        *border = BorderColor::all(theme.stage_panel_border);
        *shadow = BoxShadow::new(Color::NONE, Val::Px(0.0), Val::Px(0.0), Val::Px(0.0), Val::Px(0.0));
    }
}

/// Marker + value channel for a big-number badge ("SKILL 145.14",
/// "BPM 157"). Text updated by screen systems via `RollingNumber`.
#[derive(Component, Debug, Clone)]
pub struct BadgeValueText {
    /// Format with 2 decimals when true (skill), integer otherwise.
    pub decimals: bool,
}

/// Spawn "LABEL   <big number>" row inside a panel.
pub fn spawn_badge_row(
    parent: &mut ChildSpawnerCommands,
    theme: &Theme,
    label: &str,
    initial: &str,
    decimals: bool,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(label.to_string()),
                Theme::font(12.0),
                TextColor(theme.clear_green),
            ));
            row.spawn((
                BadgeValueText { decimals },
                Text::new(initial.to_string()),
                Theme::font(26.0),
                TextColor(theme.text_primary),
            ));
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_value_marker_carries_format() {
        assert!(BadgeValueText { decimals: true }.decimals);
    }
}
```

- [ ] **Step 2: Register** — add `pub mod stage_panel;` to `widget/mod.rs`.

- [ ] **Step 3: Build**

Run: `cargo test -p dtx-ui`
Expected: PASS / compiles. `BorderColor::all` and `BoxShadow::new` are the 0.19 constructors.

- [ ] **Step 4: Commit**

```bash
git add crates/dtx-ui/src/widget/stage_panel.rs crates/dtx-ui/src/widget/mod.rs
git commit -m "feat(ui): stage panel chrome and badge rows"
```

---

### Task 7: density_graph widget

**Files:**
- Create: `crates/dtx-ui/src/widget/density_graph.rs`
- Modify: `crates/dtx-ui/src/widget/mod.rs`
- Modify: `crates/dtx-ui/src/lib.rs`

- [ ] **Step 1: Create with tests first** — file `density_graph.rs`:

```rust
//! GITADORA note-density graph: one vertical bar per display lane,
//! height ∝ note count, staggered re-grow on selection change.

use bevy::prelude::*;

use crate::easing::EaseFunction;
use crate::theme::Theme;
use crate::tween::ScalarTween;

pub const LANE_COUNT: usize = 9;
pub const BAR_MAX_H: f32 = 200.0;
pub const BAR_STAGGER_MS: f32 = 20.0;
pub const BAR_GROW_MS: f32 = 220.0;

/// Per-lane note counts in display order LC HH LP SD HT BD LT FT CY.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct DensityData {
    pub lanes: [u32; LANE_COUNT],
    pub total: u32,
}

/// Normalized bar heights: tallest lane = 1.0, empty chart = all 0.
pub fn bar_fractions(lanes: &[u32; LANE_COUNT]) -> [f32; LANE_COUNT] {
    let max = *lanes.iter().max().unwrap_or(&0);
    let mut out = [0.0; LANE_COUNT];
    if max == 0 {
        return out;
    }
    for (i, n) in lanes.iter().enumerate() {
        out[i] = *n as f32 / max as f32;
    }
    out
}

#[derive(Component, Debug)]
pub struct DensityBar {
    pub lane: usize,
    pub tween: ScalarTween,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct DensityTotalText;

/// Spawn graph panel content: bar rail + TOTAL NOTES footer.
pub fn spawn_density_graph(parent: &mut ChildSpawnerCommands, theme: &Theme) {
    parent.spawn((
        Text::new("END"),
        Theme::font(10.0),
        TextColor(theme.text_secondary),
    ));
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(BAR_MAX_H),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::End,
            column_gap: Val::Px(3.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            ..default()
        })
        .with_children(|rail| {
            let colors = theme.lane_colors();
            for lane in 0..LANE_COUNT {
                rail.spawn((
                    DensityBar {
                        lane,
                        tween: ScalarTween::new(0.0, 0.0, BAR_GROW_MS, EaseFunction::OutQuint),
                    },
                    Node {
                        flex_grow: 1.0,
                        height: Val::Px(0.0),
                        ..default()
                    },
                    BackgroundColor(colors[lane]),
                ));
            }
        });
    parent.spawn((
        Text::new("START"),
        Theme::font(10.0),
        TextColor(theme.text_secondary),
    ));
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
            ..default()
        })
        .with_children(|footer| {
            footer.spawn((
                Text::new("TOTAL NOTES"),
                Theme::font(10.0),
                TextColor(theme.text_secondary),
            ));
            footer.spawn((
                DensityTotalText,
                Text::new("0"),
                Theme::font(22.0),
                TextColor(theme.text_primary),
            ));
        });
}

/// On `DensityData` change: restart each bar's tween toward the new
/// fraction, staggered by lane; update total text. Every frame: tick
/// tweens and write heights.
pub fn density_graph_system(
    time: Res<Time>,
    data: Res<DensityData>,
    mut bars: Query<(&mut DensityBar, &mut Node)>,
    mut totals: Query<&mut Text, With<DensityTotalText>>,
) {
    if data.is_changed() {
        let fractions = bar_fractions(&data.lanes);
        for (mut bar, _) in &mut bars {
            let lane = bar.lane;
            let from = bar.tween.value();
            bar.tween.reset(
                from,
                fractions[lane],
                BAR_GROW_MS + lane as f32 * BAR_STAGGER_MS,
                EaseFunction::OutQuint,
            );
        }
        for mut text in &mut totals {
            *text = Text::new(data.total.to_string());
        }
    }
    let dt_ms = time.delta_secs() * 1000.0;
    for (mut bar, mut node) in &mut bars {
        bar.tween.tick(dt_ms);
        node.height = Val::Px(BAR_MAX_H * bar.tween.value());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractions_scale_to_tallest() {
        let mut lanes = [0u32; LANE_COUNT];
        lanes[3] = 200; // SD
        lanes[4] = 100; // BD
        let f = bar_fractions(&lanes);
        assert_eq!(f[3], 1.0);
        assert!((f[4] - 0.5).abs() < 0.001);
        assert_eq!(f[0], 0.0);
    }

    #[test]
    fn fractions_empty_chart_all_zero() {
        let f = bar_fractions(&[0; LANE_COUNT]);
        assert!(f.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn lane_count_matches_theme_lane_colors() {
        assert_eq!(Theme::default().lane_colors().len(), LANE_COUNT);
    }
}
```

- [ ] **Step 2: Register** — `pub mod density_graph;` in `widget/mod.rs`; in `lib.rs` plugin add `.init_resource::<widget::density_graph::DensityData>()` and add `widget::density_graph::density_graph_system` to the motion systems tuple.

- [ ] **Step 3: Test**

Run: `cargo test -p dtx-ui density`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/dtx-ui/src/widget/density_graph.rs crates/dtx-ui/src/widget/mod.rs crates/dtx-ui/src/lib.rs
git commit -m "feat(ui): GITADORA density graph widget"
```

---

### Task 8: difficulty_grid widget

**Files:**
- Create: `crates/dtx-ui/src/widget/difficulty_grid.rs`
- Modify: `crates/dtx-ui/src/widget/mod.rs`

- [ ] **Step 1: Create `difficulty_grid.rs`**

```rust
//! Difficulty grid: one slot per chart in the selected folder —
//! colored label bar, big level number, achievement + rank when
//! played, dimmed "no play" otherwise. Selected slot gets yellow
//! border + glow (applied by the song-select screen system).

use bevy::prelude::*;

use crate::theme::Theme;

pub const GRID_MAX_SLOTS: usize = 5; // BASIC..EDIT

/// Slot state pushed by the screen each selection change.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DifficultySlot {
    pub present: bool,
    pub label: String,
    /// Display level, e.g. 7.80 (dlevel / 10.0).
    pub level: Option<f32>,
    /// Achievement percent 0..100 when a score exists.
    pub achievement: Option<f32>,
    pub rank: Option<String>,
}

#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct DifficultyGridData {
    pub slots: [DifficultySlot; GRID_MAX_SLOTS],
    pub selected: usize,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct DifficultySlotPanel(pub usize);

#[derive(Component, Debug, Clone, Copy)]
pub struct DifficultySlotLabel(pub usize);

#[derive(Component, Debug, Clone, Copy)]
pub struct DifficultySlotLevel(pub usize);

#[derive(Component, Debug, Clone, Copy)]
pub struct DifficultySlotScore(pub usize);

/// Spawn the grid slots (all 5; absent slots render empty and dim).
pub fn spawn_difficulty_grid(parent: &mut ChildSpawnerCommands, theme: &Theme) {
    for i in (0..GRID_MAX_SLOTS).rev() {
        // MASTER on top like GITADORA (highest index first)
        parent
            .spawn((
                DifficultySlotPanel(i),
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(6.0)),
                    margin: UiRect::bottom(Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(theme.stage_panel_bg),
                BorderColor::all(theme.stage_panel_border),
                BoxShadow::new(Color::NONE, Val::Px(0.0), Val::Px(0.0), Val::Px(0.0), Val::Px(0.0)),
            ))
            .with_children(|slot| {
                slot.spawn((
                    DifficultySlotLabel(i),
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(theme.difficulty_color(i as u8)),
                    Text::new(""),
                    Theme::font(11.0),
                    TextColor(theme.text_primary),
                ));
                slot.spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        DifficultySlotScore(i),
                        Text::new(""),
                        Theme::font(11.0),
                        TextColor(theme.text_secondary),
                    ));
                    row.spawn((
                        DifficultySlotLevel(i),
                        Text::new("--"),
                        Theme::font(28.0),
                        TextColor(theme.text_primary),
                    ));
                });
            });
    }
}

/// Format helpers used by the update system (kept pure for tests).
pub fn level_text(level: Option<f32>) -> String {
    match level {
        Some(v) => format!("{v:.2}"),
        None => "--".into(),
    }
}

pub fn score_text(slot: &DifficultySlot) -> String {
    if !slot.present {
        return String::new();
    }
    match (slot.achievement, slot.rank.as_deref()) {
        (Some(a), Some(r)) => format!("{r}  {a:.2}%"),
        (Some(a), None) => format!("{a:.2}%"),
        _ => "— no play".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_text_formats_two_decimals() {
        assert_eq!(level_text(Some(7.8)), "7.80");
        assert_eq!(level_text(None), "--");
    }

    #[test]
    fn score_text_states() {
        let mut s = DifficultySlot::default();
        assert_eq!(score_text(&s), "");
        s.present = true;
        assert_eq!(score_text(&s), "— no play");
        s.achievement = Some(93.04);
        s.rank = Some("S".into());
        assert_eq!(score_text(&s), "S  93.04%");
    }
}
```

- [ ] **Step 2: Register** — `pub mod difficulty_grid;` in `widget/mod.rs`; in `lib.rs` add `.init_resource::<widget::difficulty_grid::DifficultyGridData>()`. (The update system that reads `DifficultyGridData` and writes texts/borders lives in `song_select.rs` — Task 11 — because it needs `ThemeResource` + slot semantics owned by the screen.)

- [ ] **Step 3: Test**

Run: `cargo test -p dtx-ui difficulty`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/dtx-ui/src/widget/difficulty_grid.rs crates/dtx-ui/src/widget/mod.rs crates/dtx-ui/src/lib.rs
git commit -m "feat(ui): difficulty grid widget"
```

---

### Task 9: song_wheel widget (row geometry + components)

**Files:**
- Create: `crates/dtx-ui/src/widget/song_wheel.rs`
- Modify: `crates/dtx-ui/src/widget/mod.rs`

- [ ] **Step 1: Create with geometry tests first**

```rust
//! GITADORA song wheel: big rows arcing toward the selection.
//! Pure geometry here; the song-select screen owns spawning/content.

use bevy::prelude::*;

/// Wheel container marker.
#[derive(Component, Debug, Clone, Copy)]
pub struct SongWheel;

/// One wheel row; `index` is the folder index in the visible list.
#[derive(Component, Debug, Clone, Copy)]
pub struct WheelRow {
    pub index: usize,
}

/// Spring state for the wheel scroll (one per wheel).
#[derive(Resource, Debug, Clone)]
pub struct WheelSpring(pub crate::motion::SpringValue);

impl Default for WheelSpring {
    fn default() -> Self {
        Self(crate::motion::SpringValue::wheel(0.0))
    }
}

pub const ROW_H: f32 = 78.0;
pub const ROW_H_SELECTED: f32 = 122.0;
pub const ROW_GAP: f32 = 6.0;
pub const MAX_INDENT: f32 = 110.0;
/// Rows drawn above/below center.
pub const VISIBLE_HALF: i32 = 5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowGeom {
    /// Offset in px from wheel vertical center to row center.
    pub center_y: f32,
    pub height: f32,
    /// Left indent (arc): 0 at selection, grows with distance.
    pub indent: f32,
    /// 1.0 at selection fading toward edges.
    pub alpha: f32,
}

/// Geometry for a row at signed `offset` slots from the (fractional)
/// selection. offset = row_index - spring_value.
pub fn row_geometry(offset: f32) -> RowGeom {
    let d = offset.abs();
    // Selected-row expansion blends in within half a slot.
    let sel = (1.0 - d.min(1.0)).clamp(0.0, 1.0);
    let height = ROW_H + (ROW_H_SELECTED - ROW_H) * sel;
    // Row centers: selected row is bigger, neighbors push outward.
    let base = offset * (ROW_H + ROW_GAP);
    let expand = (ROW_H_SELECTED - ROW_H) * 0.5 * sel_shift(offset);
    let center_y = base + expand;
    // Arc indent: quadratic ease by distance, capped.
    let indent = MAX_INDENT * ((d / VISIBLE_HALF as f32).min(1.0)).powf(1.4);
    let alpha = (1.0 - (d / (VISIBLE_HALF as f32 + 0.5)).powi(2)).clamp(0.15, 1.0);
    RowGeom {
        center_y,
        height,
        indent,
        alpha,
    }
}

/// Signed push away from center for neighbor rows (-1..1).
fn sel_shift(offset: f32) -> f32 {
    offset.clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_row_is_biggest_and_flush_left() {
        let g = row_geometry(0.0);
        assert_eq!(g.height, ROW_H_SELECTED);
        assert_eq!(g.indent, 0.0);
        assert_eq!(g.center_y, 0.0);
        assert_eq!(g.alpha, 1.0);
    }

    #[test]
    fn distant_rows_shrink_indent_and_fade() {
        let g1 = row_geometry(1.0);
        let g4 = row_geometry(4.0);
        assert_eq!(g1.height, ROW_H);
        assert!(g4.indent > g1.indent);
        assert!(g4.alpha < g1.alpha);
    }

    #[test]
    fn geometry_symmetric() {
        let up = row_geometry(-2.0);
        let down = row_geometry(2.0);
        assert_eq!(up.height, down.height);
        assert_eq!(up.indent, down.indent);
        assert!((up.center_y + down.center_y).abs() < 0.001);
    }

    #[test]
    fn fractional_offset_interpolates_height() {
        let g = row_geometry(0.5);
        assert!(g.height > ROW_H && g.height < ROW_H_SELECTED);
    }
}
```

- [ ] **Step 2: Register** — `pub mod song_wheel;` in `widget/mod.rs`; in `lib.rs` add `.init_resource::<widget::song_wheel::WheelSpring>()`.

- [ ] **Step 3: Test**

Run: `cargo test -p dtx-ui song_wheel`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/dtx-ui/src/widget/song_wheel.rs crates/dtx-ui/src/widget/mod.rs crates/dtx-ui/src/lib.rs
git commit -m "feat(ui): song wheel geometry"
```

---

### Task 10: chart stats + skill points in game-menu

**Files:**
- Create: `crates/game-menu/src/chart_stats.rs`
- Modify: `crates/game-menu/src/lib.rs` (add `pub mod chart_stats;` and register plugin — check `lib.rs` for how `song_select::plugin` etc. are added and mirror it)

- [ ] **Step 1: Create `chart_stats.rs` with pure-fn tests first**

```rust
//! Selected-chart statistics for song select: per-lane density,
//! total notes (async parse, off the main thread) and the GITADORA
//! display skill formula.

use bevy::prelude::*;
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};
use dtx_core::channel::EChannel;
use dtx_ui::widget::density_graph::{DensityData, LANE_COUNT};
use game_shell::AppState;
use std::path::PathBuf;

use crate::song_select::{Selection, SongSelectSelection};

/// Display skill: (dlevel/10) × achievement% / 100 × 20.
/// 100% on a 9.80 chart = 19.6 skill points. Display-only.
pub fn skill_points(dlevel: Option<u32>, achievement_pct: f32) -> f32 {
    let level = dlevel.unwrap_or(0) as f32 / 10.0;
    level * (achievement_pct / 100.0) * 20.0
}

/// Map a drum channel to its density-graph display lane.
pub fn display_lane(ch: EChannel) -> Option<usize> {
    Some(match ch {
        EChannel::LeftCymbal => 0,
        EChannel::HiHatClose | EChannel::HiHatOpen => 1,
        EChannel::LeftPedal | EChannel::LeftBassDrum => 2,
        EChannel::Snare => 3,
        EChannel::HighTom => 4,
        EChannel::BassDrum => 5,
        EChannel::LowTom => 6,
        EChannel::FloorTom => 7,
        EChannel::Cymbal | EChannel::RideCymbal => 8,
        _ => return None,
    })
}

/// Compute per-lane counts from a parsed chart.
pub fn lane_counts(chart: &dtx_core::Chart) -> ([u32; LANE_COUNT], u32) {
    let mut lanes = [0u32; LANE_COUNT];
    let mut total = 0u32;
    for chip in &chart.chips {
        if let Some(lane) = display_lane(chip.channel) {
            lanes[lane] += 1;
            total += 1;
        }
    }
    (lanes, total)
}

/// In-flight stats parse for the currently selected chart path.
#[derive(Resource, Default)]
pub struct ChartStatsTask {
    pub task: Option<Task<Option<(PathBuf, [u32; LANE_COUNT], u32)>>>,
    pub for_path: Option<PathBuf>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<ChartStatsTask>().add_systems(
        Update,
        (start_stats_task, poll_stats_task).run_if(in_state(AppState::SongSelect)),
    );
}

/// Kick a background parse when the selected chart path changes.
fn start_stats_task(
    selection: Res<Selection>,
    selection_state: Res<SongSelectSelection>,
    db: Res<dtx_library::SongDb>,
    mut task: ResMut<ChartStatsTask>,
    mut data: ResMut<DensityData>,
) {
    if !selection.is_changed() && task.for_path.is_some() {
        return;
    }
    let path = selection
        .chart_index(&selection_state)
        .and_then(|i| db.songs.get(i))
        .map(|s| s.path.clone());
    if path == task.for_path {
        return;
    }
    task.for_path = path.clone();
    let Some(path) = path else {
        *data = DensityData::default();
        task.task = None;
        return;
    };
    let pool = AsyncComputeTaskPool::get();
    task.task = Some(pool.spawn(async move {
        let bytes = std::fs::read(&path).ok()?;
        let chart = dtx_core::parse(bytes.as_slice()).ok()?;
        let (lanes, total) = lane_counts(&chart);
        Some((path, lanes, total))
    }));
}

/// Publish finished stats (discard if the selection moved on).
fn poll_stats_task(mut task: ResMut<ChartStatsTask>, mut data: ResMut<DensityData>) {
    let Some(active) = task.task.as_mut() else {
        return;
    };
    let Some(result) = block_on(future::poll_once(active)) else {
        return;
    };
    task.task = None;
    if let Some((path, lanes, total)) = result {
        if task.for_path.as_ref() == Some(&path) {
            *data = DensityData { lanes, total };
        }
    } else {
        *data = DensityData::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_formula_matches_gitadora_shape() {
        assert!((skill_points(Some(98), 100.0) - 196.0 / 10.0).abs() < 0.01); // 9.8 * 1.0 * 20 = 196... 
        // explicit: level 9.8, 100% → 196.0? No: 9.8 * 1.0 * 20 = 196.0 / — keep raw:
        assert!((skill_points(Some(98), 100.0) - 196.0).abs() < 0.01);
        assert!((skill_points(Some(78), 93.04) - (7.8 * 0.9304 * 20.0)).abs() < 0.01);
        assert_eq!(skill_points(None, 100.0), 0.0);
        assert_eq!(skill_points(Some(50), 0.0), 0.0);
    }

    #[test]
    fn display_lane_groups_hh_and_cy() {
        assert_eq!(display_lane(EChannel::HiHatClose), display_lane(EChannel::HiHatOpen));
        assert_eq!(display_lane(EChannel::Cymbal), display_lane(EChannel::RideCymbal));
        assert_eq!(display_lane(EChannel::BGM), None);
        assert_eq!(display_lane(EChannel::Snare), Some(3));
    }
}
```

**Note:** the first assertion in `skill_formula_matches_gitadora_shape` contains a self-correcting comment — clean it to a single assert: `assert!((skill_points(Some(98), 100.0) - 196.0).abs() < 0.01);`. If `EChannel` variants differ in name (check `crates/dtx-core/src/channel.rs:78-131`), use the exact variant names from that file.

- [ ] **Step 2: Wire module** — in `crates/game-menu/src/lib.rs`, add `pub mod chart_stats;` and register `chart_stats::plugin` alongside the existing screen plugins (open the file to find the plugin aggregation — mirror how `song_select::plugin` is registered).

- [ ] **Step 3: Test**

Run: `cargo test -p game-menu chart_stats`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/game-menu/src/chart_stats.rs crates/game-menu/src/lib.rs
git commit -m "feat(song-select): async chart stats and skill formula"
```

---

### Task 11: song_select.rs rebuild — layout + wheel + left cluster

This is the centerpiece and the largest task. Keep ALL existing logic resources/tests (`SongFolderView`, `SongSelectSelection`, `Selection`, `CommandHistory`, navigation, preview systems, `recompute` machinery, and every existing test) — only the spawn/render layer changes.

**Files:**
- Modify: `crates/game-menu/src/song_select.rs`

**Delete from the file:**
- `spawn_song_select_overlay`, `show_song_select_overlay`, `hide_song_select_overlay`, `SongSelectOverlay`, `StatusPanelComp`, `StatusPaneKind`, `update_status_panes`, `kind_str`, `SortMenuElement`, `SortMenuContainerComp`, `SongSearchMenuComp`, `update_density_graph` (dead), `update_search_filter`, `DensityGraphComp`, `mode_label`, and the layout-constant tests that assert BocuD pixel positions for the removed chrome (`status_panel_positions_match_reference`, `density_graph_geometry_matches_reference`, `sort_menu_constants_match_reference`, `song_search_constants_match_reference`) plus the now-unused constants they test. Keep `COMMAND_HISTORY_BUF` + CommandHistory tests.
- The manual `despawn_recursive` helper — replace `despawn_song_select` with `game_shell::despawn_stage::<SongSelectEntity>` (children despawn via relationships in 0.19).

- [ ] **Step 1: Replace `spawn_song_select` with the GITADORA layout.** New/changed components and spawn:

```rust
use dtx_ui::motion::{EnterChoreo, RollingNumber};
use dtx_ui::widget::density_graph::spawn_density_graph;
use dtx_ui::widget::difficulty_grid::{
    spawn_difficulty_grid, DifficultyGridData, DifficultySlot, DifficultySlotLabel,
    DifficultySlotLevel, DifficultySlotPanel, DifficultySlotScore, GRID_MAX_SLOTS,
    level_text, score_text,
};
use dtx_ui::widget::song_wheel::{row_geometry, SongWheel, WheelRow, WheelSpring, VISIBLE_HALF};
use dtx_ui::widget::stage_background::spawn_stage_background;
use dtx_ui::widget::stage_panel::{panel, selected_panel, set_panel_selected, spawn_badge_row, BadgeValueText};

/// Wheel row text (title/artist), tagged for per-frame updates.
#[derive(Component)]
struct WheelRowTitle;
#[derive(Component)]
struct WheelRowMeta;
/// Left-cluster dynamic texts.
#[derive(Component)]
struct SkillValueText;
#[derive(Component)]
struct BpmValueText;
#[derive(Component)]
struct SearchText;
#[derive(Component)]
struct SortChipText;
/// Big art panel in the left column.
#[derive(Component)]
struct BigAlbumArt;

fn spawn_song_select(
    mut commands: Commands,
    selection_state: Res<SongSelectSelection>,
    theme: Res<ThemeResource>,
) {
    let t = theme.0;
    commands
        .spawn((
            SongSelectEntity,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
        ))
        .with_children(|root| {
            spawn_stage_background(root, &t);

            // ---- top bar
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(0.0),
                    left: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Px(52.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(Val::Px(20.0)),
                    ..default()
                },
                UiTransform::default(),
                EnterChoreo::slide(Vec2::new(0.0, -52.0), 0.0, 200.0),
            ))
            .with_children(|bar| {
                bar.spawn((Text::new("DTXMANIARS"), Theme::font(22.0), TextColor(t.text_primary)));
                bar.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|chips| {
                    chips.spawn((
                        SearchText,
                        Text::new("type to search…"),
                        Theme::font(13.0),
                        TextColor(t.text_secondary),
                    ));
                    chips
                        .spawn((
                            Node {
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(t.select_yellow),
                        ))
                        .with_children(|c| {
                            c.spawn((
                                SortChipText,
                                Text::new("SORT: DEFAULT"),
                                Theme::font(12.0),
                                TextColor(Color::BLACK),
                            ));
                        });
                });
            });

            // ---- left column: art + skill/bpm
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(20.0),
                    top: Val::Px(64.0),
                    width: Val::Px(300.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    ..default()
                },
                UiTransform::default(),
                EnterChoreo::slide(Vec2::new(-340.0, 0.0), 30.0, 220.0),
            ))
            .with_children(|left| {
                left.spawn((
                    BigAlbumArt,
                    AlbumArt::default(),
                    AlbumArtEntity,
                    panel(
                        &t,
                        Node {
                            width: Val::Px(300.0),
                            height: Val::Px(300.0),
                            ..default()
                        },
                    ),
                    ImageNode {
                        color: Color::WHITE.with_alpha(0.0),
                        ..default()
                    },
                ));
                left.spawn(panel(
                    &t,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                ))
                .with_children(|p| {
                    spawn_badge_row(p, &t, "SKILL BY SONG", "0.00", true);
                });
                left.spawn(panel(
                    &t,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                ))
                .with_children(|p| {
                    spawn_badge_row(p, &t, "BPM", "---", false);
                });
            });

            // ---- center column: density graph + difficulty grid
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(336.0),
                    top: Val::Px(64.0),
                    width: Val::Px(280.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(10.0),
                    ..default()
                },
                UiTransform::default(),
                EnterChoreo::slide(Vec2::new(-340.0, 0.0), 60.0, 220.0),
            ))
            .with_children(|center| {
                center
                    .spawn(panel(
                        &t,
                        Node {
                            width: Val::Px(120.0),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            padding: UiRect::all(Val::Px(8.0)),
                            ..default()
                        },
                    ))
                    .with_children(|p| spawn_density_graph(p, &t));
                center
                    .spawn(Node {
                        flex_grow: 1.0,
                        flex_direction: FlexDirection::Column,
                        ..default()
                    })
                    .with_children(|p| spawn_difficulty_grid(p, &t));
            });

            // ---- right: song wheel container (rows spawned separately)
            root.spawn((
                SongWheel,
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(0.0),
                    top: Val::Px(52.0),
                    width: Val::Px(620.0),
                    height: Val::Px(632.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
            ))
            .with_children(|wheel| {
                spawn_wheel_rows(wheel, &selection_state, &t);
            });

            // ---- bottom hint bar
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(0.0),
                    left: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Px(34.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(18.0),
                    flex_direction: FlexDirection::Row,
                    ..default()
                },
                UiTransform::default(),
                EnterChoreo::slide(Vec2::new(0.0, 34.0), 0.0, 200.0),
            ))
            .with_children(|bar| {
                for (label, hot) in [
                    ("↑↓ SELECT", false),
                    ("←→ DIFFICULTY", false),
                    ("ENTER PLAY", true),
                    ("TAB SORT", false),
                    ("F5 RESCAN", false),
                    ("F1 SETTINGS", false),
                    ("ESC BACK", false),
                ] {
                    bar.spawn((
                        Text::new(label),
                        Theme::font(12.0),
                        TextColor(if hot { t.select_yellow } else { t.text_secondary }),
                    ));
                }
            });
        });
}

/// Spawn one absolute-positioned row per visible folder. Positions are
/// written every frame by `wheel_layout_system`.
fn spawn_wheel_rows(
    wheel: &mut ChildSpawnerCommands,
    selection_state: &SongSelectSelection,
    t: &Theme,
) {
    if selection_state.visible.is_empty() {
        wheel.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(60.0),
                top: Val::Px(280.0),
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(t.stage_panel_bg),
            Text::new(format!(
                "no songs found — put song folders in {}\npress F5 to rescan",
                dtx_library::default_song_dir().display()
            )),
            Theme::font(16.0),
            TextColor(t.text_secondary),
        ));
        return;
    }
    for (i, folder) in selection_state.visible.iter().enumerate() {
        wheel
            .spawn((
                WheelRow { index: i },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(600.0),
                    height: Val::Px(dtx_ui::widget::song_wheel::ROW_H),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(12.0),
                    padding: UiRect::horizontal(Val::Px(14.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(t.stage_panel_bg),
                BorderColor::all(t.stage_panel_border),
                BoxShadow::new(Color::NONE, Val::Px(0.0), Val::Px(0.0), Val::Px(0.0), Val::Px(0.0)),
                Visibility::Hidden,
            ))
            .with_children(|row| {
                row.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    ..default()
                })
                .with_children(|col| {
                    col.spawn((
                        WheelRowTitle,
                        Text::new(folder.title.clone()),
                        Theme::font(19.0),
                        TextColor(t.text_primary),
                    ));
                    col.spawn((
                        WheelRowMeta,
                        Text::new(folder.artist.clone()),
                        Theme::font(12.0),
                        TextColor(t.text_secondary),
                    ));
                });
            });
    }
}
```

- [ ] **Step 2: Add the per-frame wheel layout system**

```rust
/// Drive the wheel spring toward the selected index and lay out rows.
fn wheel_layout_system(
    time: Res<Time>,
    selection: Res<Selection>,
    theme: Res<ThemeResource>,
    mut spring: ResMut<WheelSpring>,
    mut rows: Query<(
        &WheelRow,
        &mut Node,
        &mut Visibility,
        &mut BorderColor,
        &mut BoxShadow,
        &mut BackgroundColor,
    )>,
) {
    let t = theme.0;
    spring.0.set_target(selection.folder as f32);
    spring.0.tick(time.delta_secs());
    let center = spring.0.value;
    const WHEEL_H: f32 = 632.0;
    for (row, mut node, mut vis, mut border, mut shadow, mut bg) in &mut rows {
        let offset = row.index as f32 - center;
        if offset.abs() > (VISIBLE_HALF as f32 + 1.0) {
            *vis = Visibility::Hidden;
            continue;
        }
        *vis = Visibility::Visible;
        let g = row_geometry(offset);
        node.top = Val::Px(WHEEL_H / 2.0 + g.center_y - g.height / 2.0);
        node.left = Val::Px(g.indent);
        node.height = Val::Px(g.height);
        let selected = offset.abs() < 0.5;
        set_panel_selected(&t, selected, &mut border, &mut shadow);
        bg.0 = t.stage_panel_bg.with_alpha(0.93 * g.alpha);
    }
}
```

- [ ] **Step 3: Rebuild `render_selected_song` into cluster updates** — replace `format_song_detail` usage with systems feeding the widgets:

```rust
/// Push selection → difficulty grid, skill/bpm badges, sort chip.
fn update_left_cluster(
    selection: Res<Selection>,
    selection_state: Res<SongSelectSelection>,
    db: Res<SongDb>,
    mut grid: ResMut<DifficultyGridData>,
    mut badge_texts: Query<(&BadgeValueText, &mut Text)>,
    mut sort_chip: Query<&mut Text, (With<SortChipText>, Without<BadgeValueText>)>,
) {
    if !selection.is_changed() && !selection_state.is_changed() {
        return;
    }
    // difficulty grid
    let mut data = DifficultyGridData::default();
    data.selected = selection.difficulty as usize;
    if let Some(folder) = selection_state.visible.get(selection.folder) {
        for (slot_i, chart_idx) in folder.chart_indices.iter().enumerate().take(GRID_MAX_SLOTS) {
            let Some(song) = db.songs.get(*chart_idx) else { continue };
            let ini = dtx_scoring::score_ini::score_ini_path(&song.path);
            let best = dtx_scoring::score_ini::read_best(&ini);
            data.slots[slot_i] = DifficultySlot {
                present: true,
                label: format!("DRUM · {}", SongFolderView::difficulty_label(slot_i as u8)),
                level: song.dlevel.map(|v| v as f32 / 10.0),
                achievement: best.as_ref().map(|b| b.accuracy()),
                rank: best.as_ref().map(|b| b.rank.clone()),
            };
        }
    }
    *grid = data;

    // skill + bpm badges
    let (skill, bpm) = selection
        .chart_index(&selection_state)
        .and_then(|i| db.songs.get(i))
        .map(|song| {
            let ini = dtx_scoring::score_ini::score_ini_path(&song.path);
            let acc = dtx_scoring::score_ini::read_best(&ini)
                .map(|b| b.accuracy())
                .unwrap_or(0.0);
            (
                crate::chart_stats::skill_points(song.dlevel, acc),
                song.bpm.unwrap_or(0.0),
            )
        })
        .unwrap_or((0.0, 0.0));
    for (badge, mut text) in &mut badge_texts {
        *text = Text::new(if badge.decimals {
            format!("{skill:.2}")
        } else if bpm > 0.0 {
            format!("{}", bpm.round() as i32)
        } else {
            "---".into()
        });
    }
    for mut text in &mut sort_chip {
        *text = Text::new(format!(
            "SORT: {}",
            match selection_state.sort_mode {
                SortMode::Default => "DEFAULT",
                SortMode::ByTitle => "TITLE",
                SortMode::ByArtist => "ARTIST",
            }
        ));
    }
}

/// Write grid slot data into the widget's text/border entities.
fn render_difficulty_grid(
    grid: Res<DifficultyGridData>,
    theme: Res<ThemeResource>,
    mut panels: Query<(&DifficultySlotPanel, &mut BorderColor, &mut BoxShadow, &mut BackgroundColor)>,
    mut labels: Query<(&DifficultySlotLabel, &mut Text), (Without<DifficultySlotLevel>, Without<DifficultySlotScore>)>,
    mut levels: Query<(&DifficultySlotLevel, &mut Text), (Without<DifficultySlotLabel>, Without<DifficultySlotScore>)>,
    mut scores: Query<(&DifficultySlotScore, &mut Text), (Without<DifficultySlotLabel>, Without<DifficultySlotLevel>)>,
) {
    if !grid.is_changed() {
        return;
    }
    let t = theme.0;
    for (panel, mut border, mut shadow, mut bg) in &mut panels {
        let slot = &grid.slots[panel.0];
        let selected = slot.present && panel.0 == grid.selected;
        set_panel_selected(&t, selected, &mut border, &mut shadow);
        bg.0 = if slot.present {
            t.stage_panel_bg
        } else {
            t.stage_panel_bg.with_alpha(0.35)
        };
    }
    for (label, mut text) in &mut labels {
        *text = Text::new(grid.slots[label.0].label.clone());
    }
    for (level, mut text) in &mut levels {
        *text = Text::new(level_text(grid.slots[level.0].level));
    }
    for (score, mut text) in &mut scores {
        *text = Text::new(score_text(&grid.slots[score.0]));
    }
}
```

Keep the existing `update_album_art_image` (it now targets the `BigAlbumArt` panel via `AlbumArtEntity`, unchanged) — and extend it to also swap the ambient background image: add a second query `mut ambient: Query<&mut ImageNode, (With<dtx_ui::widget::stage_background::AmbientArt>, Without<AlbumArtEntity>)>` and inside the per-song branch set `a.image = asset_server.load(...)` / `a.image = Handle::default()` correspondingly (missing art → `Handle::default()` → ambient system holds it at alpha 0 → black, per spec).

- [ ] **Step 4: Wheel row respawn on list change** — when `SongSelectSelection.visible` changes (sort/search/rescan), despawn row entities and respawn:

```rust
fn respawn_wheel_on_change(
    mut commands: Commands,
    selection_state: Res<SongSelectSelection>,
    theme: Res<ThemeResource>,
    wheel: Query<Entity, With<SongWheel>>,
    rows: Query<Entity, With<WheelRow>>,
) {
    if !selection_state.is_changed() {
        return;
    }
    let Ok(wheel_entity) = wheel.single() else { return };
    for row in &rows {
        commands.entity(row).despawn();
    }
    let t = theme.0;
    commands.entity(wheel_entity).with_children(|w| {
        spawn_wheel_rows(w, &selection_state, &t);
    });
}
```

- [ ] **Step 5: Update the plugin systems** — replace the old Update set:

```rust
        .add_systems(
            Update,
            (
                maybe_recompute_visible,
                song_select_navigation,
                search_input,          // Task 12
                respawn_wheel_on_change,
                wheel_layout_system,
                update_left_cluster,
                render_difficulty_grid,
                bgm_preview_on_change,
                update_album_art_image,
            )
                .run_if(in_state(AppState::SongSelect)),
        );
```

(Until Task 12 lands, omit `search_input` from the tuple.) Remove the `Startup` overlay system registration and OnEnter/OnExit references to removed systems. Reset the wheel spring on enter: add to the OnEnter chain a small system `fn reset_wheel_spring(selection: Res<Selection>, mut spring: ResMut<WheelSpring>) { spring.0 = dtx_ui::motion::SpringValue::wheel(selection.folder as f32); }`.

- [ ] **Step 6: Fix imports/tests, run**

Run: `cargo test -p game-menu`
Expected: PASS — all preserved logic tests green; removed-chrome tests deleted. Then `cargo build -p dtxmaniars 2>/dev/null || cargo build --workspace` to confirm the binary builds.

- [ ] **Step 7: Commit**

```bash
git add crates/game-menu/src/song_select.rs
git commit -m "feat(song-select): GITADORA stage layout with song wheel"
```

---

### Task 12: type-to-search input

**Files:**
- Modify: `crates/game-menu/src/song_select.rs`

- [ ] **Step 1: Add test for query editing helper** (pure function first):

```rust
    #[test]
    fn apply_search_edit_appends_and_deletes() {
        let mut q = String::new();
        apply_search_char(&mut q, 'a');
        apply_search_char(&mut q, 'B');
        assert_eq!(q, "aB");
        apply_search_backspace(&mut q);
        assert_eq!(q, "a");
        apply_search_backspace(&mut q);
        apply_search_backspace(&mut q);
        assert_eq!(q, "");
    }

    #[test]
    fn apply_search_char_caps_length() {
        let mut q = "x".repeat(64);
        apply_search_char(&mut q, 'y');
        assert_eq!(q.len(), 64);
    }
```

- [ ] **Step 2: Implement helpers + system:**

```rust
pub fn apply_search_char(query: &mut String, c: char) {
    if query.len() >= 64 || c.is_control() {
        return;
    }
    query.push(c);
}

pub fn apply_search_backspace(query: &mut String) {
    query.pop();
}

/// Live type-to-search: printable keys append, Backspace deletes,
/// filter recomputes immediately. Nav/hotkeys still work (arrows,
/// Enter, Tab, F-keys, Esc are not printable characters).
fn search_input(
    mut chars: MessageReader<bevy::input::keyboard::KeyboardInput>,
    mut selection_state: ResMut<SongSelectSelection>,
    mut search_text: Query<&mut Text, With<SearchText>>,
) {
    use bevy::input::keyboard::Key;
    let mut changed = false;
    for ev in chars.read() {
        if !ev.state.is_pressed() {
            continue;
        }
        match &ev.logical_key {
            Key::Character(s) => {
                for c in s.chars() {
                    apply_search_char(&mut selection_state.search_query, c);
                }
                changed = true;
            }
            Key::Space => {
                apply_search_char(&mut selection_state.search_query, ' ');
                changed = true;
            }
            Key::Backspace => {
                apply_search_backspace(&mut selection_state.search_query);
                changed = true;
            }
            _ => {}
        }
    }
    if changed {
        selection_state.dirty = true;
        let q = selection_state.search_query.clone();
        for mut text in &mut search_text {
            *text = Text::new(if q.is_empty() {
                "type to search…".to_string()
            } else {
                format!("search: {q}")
            });
        }
    }
}
```

Add `search_input` to the Update tuple (before `respawn_wheel_on_change`). Also clear the query on screen enter (append to the OnEnter chain): `fn reset_search(mut s: ResMut<SongSelectSelection>) { s.search_query.clear(); s.dirty = true; }`.

- [ ] **Step 3: Test + run**

Run: `cargo test -p game-menu`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/game-menu/src/song_select.rs
git commit -m "feat(song-select): live type-to-search"
```

---

### Task 13: Title screen rebuild

**Files:**
- Modify: `crates/game-menu/src/title.rs`

- [ ] **Step 1: Rewrite `spawn_title`:**

```rust
use dtx_ui::motion::{BeatPulse, EnterChoreo};
use dtx_ui::widget::stage_background::spawn_stage_background;

fn spawn_title(mut commands: Commands, theme: Res<ThemeResource>) {
    let t = theme.0;
    commands
        .spawn((
            TitleEntity,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(48.0),
                ..default()
            },
        ))
        .with_children(|root| {
            spawn_stage_background(root, &t);
            root.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(48.0), Val::Px(18.0)),
                    border: UiRect::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(t.stage_panel_bg),
                BorderColor::all(t.text_primary),
                BoxShadow::new(
                    Color::srgba(0.0, 0.667, 1.0, 0.25),
                    Val::Px(0.0),
                    Val::Px(0.0),
                    Val::Px(4.0),
                    Val::Px(30.0),
                ),
                UiTransform::default(),
                EnterChoreo::slide(Vec2::new(0.0, -120.0), 0.0, 450.0),
            ))
            .with_children(|logo| {
                logo.spawn((
                    Text::new("DTXMANIARS"),
                    Theme::font(56.0),
                    TextColor(t.text_primary),
                ));
            });
            root.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(32.0), Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(t.select_yellow),
                BoxShadow::new(
                    t.select_yellow.with_alpha(0.4),
                    Val::Px(0.0),
                    Val::Px(0.0),
                    Val::Px(2.0),
                    Val::Px(18.0),
                ),
                UiTransform::default(),
                BeatPulse::new(60.0, 0.06),
                EnterChoreo::slide(Vec2::new(0.0, 60.0), 150.0, 300.0),
            ))
            .with_children(|chip| {
                chip.spawn((
                    Text::new("PRESS ENTER"),
                    Theme::font(20.0),
                    TextColor(Color::BLACK),
                ));
            });
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(12.0),
                    left: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::horizontal(Val::Px(20.0)),
                    ..default()
                },
            ))
            .with_children(|bar| {
                bar.spawn((
                    Text::new(format!("v{}", env!("CARGO_PKG_VERSION"))),
                    Theme::font(12.0),
                    TextColor(t.text_secondary),
                ));
                bar.spawn((
                    Text::new("ESC QUIT"),
                    Theme::font(12.0),
                    TextColor(t.text_secondary),
                ));
            });
        });
}
```

Note the `EnterChoreo` on the logo panel — a `BeatPulse` and `EnterChoreo` both write `UiTransform` on the PRESS ENTER chip; `enter_choreo_system` writes translation, `beat_pulse_system` writes scale — they compose because they touch different fields. No tagline text anywhere (spec).

- [ ] **Step 2: Build + existing test**

Run: `cargo test -p game-menu title && cargo build --workspace`
Expected: PASS / builds.

- [ ] **Step 3: Commit**

```bash
git add crates/game-menu/src/title.rs
git commit -m "feat(title): GITADORA stage title screen"
```

---

### Task 14: Settings screen rebuild

**Files:**
- Modify: `crates/game-menu/src/config.rs`

Keep: `ConfigTab`, `ConfigItem` tables, `ConfigDraft` load/save, all adjust/label helpers and their tests. Replace the render layer + add Tab-key section switching (currently missing). Delete `spawn_config_layout`, `show_config_chrome`, `hide_config_chrome`, `ConfigLeftMenu`, `ConfigDescriptionPanel` (Startup chrome replaced by per-enter spawn), and the pixel-position constants + their 4 tests (`config_left_menu_position_matches_reference`, `config_cursor_size_matches_reference`, `config_description_position_matches_reference`, `config_item_bar_matches_reference`).

- [ ] **Step 1: Add description text to `ConfigItem`** — add field `pub desc: &'static str` and populate every item (one line each, e.g. VSync: `"Lock framerate to display refresh. Reduces tearing; adds up to one frame of latency."`; Scroll Speed: `"Note scroll speed multiplier during gameplay."` — write a sensible one-liner per item, all 34). Update the `ConfigItem` construction sites in all four tables.

- [ ] **Step 2: New components + rail-based spawn:**

```rust
use dtx_ui::motion::EnterChoreo;
use dtx_ui::widget::stage_background::spawn_stage_background;
use dtx_ui::widget::stage_panel::{panel, set_panel_selected};

#[derive(Component)]
struct RailTabLabel(usize);
#[derive(Component)]
struct SettingsDescText;

fn spawn_config(
    mut commands: Commands,
    theme: Res<ThemeResource>,
    draft: Res<ConfigDraft>,
    active: Res<ActiveConfigTab>,
    mut selection: ResMut<ConfigSelection>,
    mut tab_idx: ResMut<ActiveTabIndex>,
) {
    let t = theme.0;
    let tab = active.0.unwrap_or(ConfigTab::System);
    let items = tab.items();
    tab_idx.0 = ConfigTab::all().iter().position(|x| *x == tab).unwrap_or(0);
    selection.0 = 0;

    commands
        .spawn((
            ConfigEntity,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
        ))
        .with_children(|root| {
            spawn_stage_background(root, &t);

            // left rail
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(220.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(20.0)),
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                BackgroundColor(t.stage_panel_bg),
                UiTransform::default(),
                EnterChoreo::slide(Vec2::new(-220.0, 0.0), 0.0, 220.0),
            ))
            .with_children(|rail| {
                rail.spawn((
                    Text::new("SETTINGS"),
                    Theme::font(24.0),
                    TextColor(t.text_primary),
                ));
                for (i, tab_i) in ConfigTab::all().iter().enumerate() {
                    let is_active = *tab_i == tab;
                    rail.spawn((
                        RailTabLabel(i),
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                            margin: UiRect::top(Val::Px(if i == 0 { 24.0 } else { 0.0 })),
                            ..default()
                        },
                        BackgroundColor(if is_active { t.select_yellow } else { Color::NONE }),
                        Text::new(tab_i.label().to_uppercase()),
                        Theme::font(15.0),
                        TextColor(if is_active { Color::BLACK } else { t.text_secondary }),
                    ));
                }
            });

            // rows
            root.spawn(Node {
                position_type: PositionType::Absolute,
                left: Val::Px(250.0),
                top: Val::Px(50.0),
                width: Val::Px(680.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|list| {
                if matches!(tab, ConfigTab::Exit) {
                    list.spawn((
                        Text::new("Save settings and return to Title. (ENTER)"),
                        Theme::font(18.0),
                        TextColor(t.text_primary),
                    ));
                } else {
                    for (i, item) in items.iter().enumerate() {
                        list.spawn((
                            ConfigItemEntity(i),
                            Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                padding: UiRect::axes(Val::Px(16.0), Val::Px(9.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(t.stage_panel_bg),
                            BorderColor::all(t.stage_panel_border),
                            BoxShadow::new(Color::NONE, Val::Px(0.0), Val::Px(0.0), Val::Px(0.0), Val::Px(0.0)),
                            UiTransform::default(),
                            EnterChoreo::slide(Vec2::new(240.0, 0.0), i as f32 * 20.0, 200.0),
                        ))
                        .with_children(|row| {
                            row.spawn((
                                Text::new(item.label),
                                Theme::font(16.0),
                                TextColor(t.text_primary),
                            ));
                            row.spawn((
                                ConfigValueText(i),
                                Text::new(format!("◂ {} ▸", (item.value)(&draft.0))),
                                Theme::font(16.0),
                                TextColor(t.clear_green),
                            ));
                        });
                    }
                }
            });

            // description panel
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(250.0),
                    bottom: Val::Px(60.0),
                    width: Val::Px(680.0),
                    padding: UiRect::all(Val::Px(12.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(t.stage_panel_bg),
                BorderColor::all(t.stage_panel_border),
                SettingsDescText,
                Text::new(items.first().map(|i| i.desc).unwrap_or("")),
                Theme::font(14.0),
                TextColor(t.text_secondary),
            ));

            // hint bar
            root.spawn(Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(34.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                column_gap: Val::Px(18.0),
                flex_direction: FlexDirection::Row,
                ..default()
            })
            .with_children(|bar| {
                for (label, hot) in [
                    ("↑↓ ROW", false),
                    ("←→ ADJUST", false),
                    ("TAB SECTION", false),
                    ("ESC SAVE & BACK", true),
                ] {
                    bar.spawn((
                        Text::new(label),
                        Theme::font(12.0),
                        TextColor(if hot { t.select_yellow } else { t.text_secondary }),
                    ));
                }
            });
        });
}
```

- [ ] **Step 3: Tab switching + selection render + description:**

```rust
/// Tab key cycles sections; respawns the screen content for the new
/// tab (rows re-enter with the stagger choreography).
fn config_tab_navigation(
    keys: Res<ButtonInput<KeyCode>>,
    mut active: ResMut<ActiveConfigTab>,
    mut requests: MessageWriter<TransitionRequest>,
    mut commands: Commands,
    roots: Query<Entity, With<ConfigEntity>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        request_transition(&mut requests, AppState::Title);
        return;
    }
    if keys.just_pressed(KeyCode::Tab) {
        let all = ConfigTab::all();
        let cur = active.0.unwrap_or(ConfigTab::System);
        let idx = all.iter().position(|t| *t == cur).unwrap_or(0);
        active.0 = Some(all[(idx + 1) % all.len()]);
        for e in &roots {
            commands.entity(e).despawn();
        }
    }
}

/// Respawn content when the active tab changed (after despawn above).
fn respawn_on_tab_change(
    active: Res<ActiveConfigTab>,
    roots: Query<(), With<ConfigEntity>>,
    commands: Commands,
    theme: Res<ThemeResource>,
    draft: Res<ConfigDraft>,
    selection: ResMut<ConfigSelection>,
    tab_idx: ResMut<ActiveTabIndex>,
) {
    if active.is_changed() && roots.is_empty() {
        spawn_config(commands, theme, draft, active.into(), selection, tab_idx);
    }
}
```

Note for the implementer: `spawn_config` takes `Res<ActiveConfigTab>` — calling it from another system needs the same params; if the `.into()` conversion fights you, duplicate the two-line body (despawn happens in `config_tab_navigation`, and `respawn_on_tab_change` can simply be `spawn_config` gated with `.run_if(|active: Res<ActiveConfigTab>, roots: Query<(), With<ConfigEntity>>| active.is_changed() && roots.is_empty())` added to the Update schedule — pick whichever compiles cleanly).

Extend `render_config_selection` to also (a) wrap values in `◂ ▸` arrows for the selected row only, (b) yellow border+glow the selected row via `set_panel_selected`, and (c) write `items[selection.0].desc` into `SettingsDescText`:

```rust
fn render_config_selection(
    theme: Res<ThemeResource>,
    selection: Res<ConfigSelection>,
    draft: Res<ConfigDraft>,
    active: Res<ActiveConfigTab>,
    mut rows: Query<(&ConfigItemEntity, &mut BorderColor, &mut BoxShadow, &mut BackgroundColor)>,
    mut values: Query<(&ConfigValueText, &mut Text), Without<SettingsDescText>>,
    mut desc: Query<&mut Text, With<SettingsDescText>>,
) {
    let t = theme.0;
    let items = match active.0 {
        Some(tab) => tab.items(),
        None => &[],
    };
    for (row, mut border, mut shadow, mut bg) in &mut rows {
        let selected = row.0 == selection.0;
        set_panel_selected(&t, selected, &mut border, &mut shadow);
        bg.0 = t.stage_panel_bg;
    }
    for (value, mut text) in &mut values {
        let display = items.get(value.0).map(|i| (i.value)(&draft.0)).unwrap_or_default();
        *text = Text::new(if value.0 == selection.0 {
            format!("◂ {display} ▸")
        } else {
            display
        });
    }
    if let Some(item) = items.get(selection.0) {
        for mut text in &mut desc {
            *text = Text::new(item.desc);
        }
    }
}
```

Update the plugin: drop `spawn_config_layout` from Startup, drop show/hide chrome from OnEnter/OnExit, add the respawn-on-tab-change system to Update. On the Exit tab, ENTER key saves + transitions (add to `config_row_navigation`: `if matches!(tab, ConfigTab::Exit) && keys.just_pressed(KeyCode::Enter) { ... request_transition(...Title) }` — move the tab lookup above the early-return).

- [ ] **Step 4: Test**

Run: `cargo test -p game-menu config`
Expected: PASS (kept tests; deleted position tests gone). A new test to add:

```rust
    #[test]
    fn every_item_has_description() {
        for tab in ConfigTab::all() {
            for item in tab.items() {
                assert!(!item.desc.is_empty(), "{} missing desc", item.label);
            }
        }
    }
```

- [ ] **Step 5: Commit**

```bash
git add crates/game-menu/src/config.rs
git commit -m "feat(settings): GITADORA settings screen with section rail"
```

---

### Task 15: Song Loading rebuild

**Files:**
- Modify: `crates/game-menu/src/song_loading.rs`

Keep the entire load state machine (`LoadPhase`, `ChartParseTask`, `RequiredAudio`, cancel, nowloading, ghost, jacket/level data extraction). Replace only `spawn_loading`, `spawn_jacket`, `spawn_level_ui`, `update_status_text` visuals.

- [ ] **Step 1: Replace UI spawn** — hero card reads selection info available before parse via `SongSelectSelection`/`SongDb`:

```rust
use dtx_ui::motion::EnterChoreo;
use dtx_ui::widget::stage_background::spawn_stage_background;
use dtx_ui::widget::stage_panel::panel;
use crate::song_select::{Selection, SongSelectSelection};

#[derive(Component)]
struct LoadingBarFill;

fn spawn_loading(
    mut commands: Commands,
    theme: Res<ThemeResource>,
    selection: Res<Selection>,
    selection_state: Res<SongSelectSelection>,
    db: Res<dtx_library::SongDb>,
    asset_server: Res<AssetServer>,
) {
    let t = theme.0;
    let song = selection
        .chart_index(&selection_state)
        .and_then(|i| db.songs.get(i))
        .cloned();
    let (title, artist, bpm, dlevel, difficulty, art) = match &song {
        Some(s) => (
            s.title.clone(),
            s.artist.clone(),
            s.bpm,
            s.dlevel,
            crate::song_select::SongFolderView::difficulty_label(selection.difficulty).to_string(),
            s.preimage_path.clone(),
        ),
        None => ("Unknown".into(), String::new(), None, None, String::new(), None),
    };

    commands
        .spawn((
            LoadingEntity,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|root| {
            spawn_stage_background(root, &t);
            root.spawn((
                panel(
                    &t,
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(24.0),
                        padding: UiRect::all(Val::Px(24.0)),
                        ..default()
                    },
                ),
                UiTransform::default(),
                EnterChoreo::slide(Vec2::new(0.0, 40.0), 0.0, 250.0),
            ))
            .with_children(|card| {
                let mut img = ImageNode {
                    color: Color::WHITE.with_alpha(if art.is_some() { 1.0 } else { 0.0 }),
                    ..default()
                };
                if let Some(p) = &art {
                    img.image = asset_server.load(p.to_string_lossy().to_string());
                }
                card.spawn((
                    Node {
                        width: Val::Px(160.0),
                        height: Val::Px(160.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::BLACK),
                    BorderColor::all(t.select_yellow),
                    img,
                ));
                card.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|col| {
                    col.spawn((
                        Text::new("NOW LOADING"),
                        Theme::font(12.0),
                        TextColor(t.text_secondary),
                    ));
                    col.spawn((
                        Text::new(title),
                        Theme::font(34.0),
                        TextColor(t.text_primary),
                    ));
                    col.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(10.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|meta| {
                        meta.spawn((
                            Text::new(format!(
                                "{artist} · BPM {}",
                                bpm.map(|v| (v.round() as i32).to_string())
                                    .unwrap_or_else(|| "?".into())
                            )),
                            Theme::font(15.0),
                            TextColor(t.text_secondary),
                        ));
                        meta.spawn((
                            Node {
                                padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(t.difficulty_color(2)),
                        ))
                        .with_children(|chip| {
                            chip.spawn((
                                Text::new(format!(
                                    "{difficulty} {}",
                                    dlevel
                                        .map(|v| format!("{:.2}", v as f32 / 10.0))
                                        .unwrap_or_else(|| "--".into())
                                )),
                                Theme::font(12.0),
                                TextColor(t.text_primary),
                            ));
                        });
                    });
                    col.spawn((
                        Node {
                            width: Val::Px(420.0),
                            height: Val::Px(8.0),
                            border: UiRect::all(Val::Px(1.0)),
                            margin: UiRect::top(Val::Px(14.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.13, 0.13, 0.13)),
                        BorderColor::all(t.stage_panel_border),
                    ))
                    .with_children(|track| {
                        track.spawn((
                            LoadingBarFill,
                            Node {
                                width: Val::Percent(0.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(t.select_yellow),
                            BoxShadow::new(
                                t.select_yellow.with_alpha(0.5),
                                Val::Px(0.0),
                                Val::Px(0.0),
                                Val::Px(1.0),
                                Val::Px(8.0),
                            ),
                        ));
                    });
                    col.spawn((
                        Text::new(""),
                        Theme::font(12.0),
                        TextColor(t.text_secondary),
                        LoadingStatusText,
                    ));
                });
            });
        });
}
```

Delete `spawn_jacket` and `spawn_level_ui` and their call sites in `poll_chart_parse` (the hero card already shows art + level; remove `JacketImage`, `LevelText`, `DifficultyText` components). Keep `play_nowloading` call.

- [ ] **Step 2: Smooth progress bar** — extend `update_status_text`:

```rust
fn update_status_text(
    time: Res<Time>,
    phase: Res<LoadPhase>,
    progress: Res<LoadingProgress>,
    required: Res<RequiredAudio>,
    mut status_query: Query<&mut Text, With<LoadingStatusText>>,
    mut bar: Query<&mut Node, With<LoadingBarFill>>,
) {
    let target_pct = match *phase {
        LoadPhase::Parsing => 8.0,
        LoadPhase::LoadingAudio => 10.0 + progress.0 * 88.0,
        LoadPhase::Ready => 100.0,
        _ => 0.0,
    };
    for mut node in &mut bar {
        let current = match node.width {
            Val::Percent(p) => p,
            _ => 0.0,
        };
        let next = current + (target_pct - current) * (8.0 * time.delta_secs()).min(1.0);
        node.width = Val::Percent(next.clamp(0.0, 100.0));
    }
    let total = required.0.len();
    let status = match *phase {
        LoadPhase::Idle => String::new(),
        LoadPhase::Parsing => "parsing chart…".to_string(),
        LoadPhase::LoadingAudio => format!(
            "loading audio chips… {}/{}",
            ((progress.0 * total as f32).round() as usize).min(total),
            total
        ),
        LoadPhase::Ready => "ready".to_string(),
        LoadPhase::Failed => "failed — returning to song select".to_string(),
    };
    for mut text in &mut status_query {
        *text = Text::new(status.clone());
    }
}
```

- [ ] **Step 3: Test + build**

Run: `cargo test -p game-menu && cargo build --workspace`
Expected: PASS / builds.

- [ ] **Step 4: Commit**

```bash
git add crates/game-menu/src/song_loading.rs
git commit -m "feat(loading): hero card loading screen with real progress"
```

---

### Task 16: BPM-synced pulses + final verification

**Files:**
- Modify: `crates/game-menu/src/song_select.rs` (selected-row pulse sync)
- Verify: whole workspace

- [ ] **Step 1: Sync BeatPulse to selected song BPM** — in `song_select.rs`, add a `BeatPulse::new(120.0, 0.03)` + `UiTransform::default()` to the selected wheel row is wrong (rows swap selection) — instead pulse the **glow**: in `wheel_layout_system`, add a `Local<f32>` phase accumulator and modulate the selected row's shadow alpha:

```rust
// add params: mut phase: Local<f32>, and read the selected song bpm:
//   selection_state: Res<SongSelectSelection>,
// at top of system:
let bpm = selection_state
    .song
    .as_ref()
    .and_then(|s| s.bpm)
    .unwrap_or(120.0)
    .max(1.0);
*phase = (*phase + time.delta_secs() * bpm / 60.0).rem_euclid(1.0);
let glow = 0.30 + 0.25 * (1.0 - *phase).powi(2);
// in the selected branch, replace set_panel_selected(...) shadow with:
if selected {
    *border = BorderColor::all(t.select_yellow);
    *shadow = BoxShadow::new(
        t.select_yellow.with_alpha(glow),
        Val::Px(0.0), Val::Px(0.0), Val::Px(2.0), Val::Px(14.0),
    );
} else {
    set_panel_selected(&t, false, &mut border, &mut shadow);
}
```

(`SongSelectSelection.song` is set by existing selection systems; confirm it is still populated — if `render_selected_song` was the writer and got deleted, move the `sel.song = ...` assignment into `update_left_cluster`.)

- [ ] **Step 2: Full workspace test + lint**

Run: `cargo test --workspace && cargo clippy --workspace -- -D warnings`
Expected: all green. Fix any warnings introduced by the redesign (unused imports from deleted code are the usual suspects).

- [ ] **Step 3: Visual verification (bevy-brp)** — launch the game via the bevy-brp MCP (`brp_launch`, then `brp_extras_screenshot`) and screenshot each screen: Title → ENTER → Song Select (scroll a few rows, switch difficulty) → F1 → Settings (Tab through sections) → back → ENTER a song → Loading. Check against the approved mockups (`.superpowers/brainstorm/1333996-1783187662/content/song-select-layout-v2.html`, `other-screens.html`):
  - plain black stage on every screen (no streaks/gradient decorations)
  - wheel rows ~78px, selected ~122px with yellow border + pulsing glow, arc indent
  - density bars re-grow staggered on selection change; TOTAL NOTES updates
  - skill/BPM numbers update; sort chip reflects Tab
  - typing filters the list live; Backspace deletes
  - Settings: left rail, yellow active section, `◂ value ▸` on selected row, description panel updates, rows cascade on Tab
  - Loading: hero card, bar fills smoothly, no jump backward
  - missing album art → black art box, black ambient (no gradient)

- [ ] **Step 4: Commit any fixes, then final commit**

```bash
git add -A crates/
git commit -m "feat(menu): GITADORA menu UI polish pass"
```

---

## Self-review notes (already applied)

- Spec coverage: palette (T1), motion table (T2–T4, T11 wheel spring, T13 pulse, T14 stagger, T15 bar lerp, T16 beat glow), stage background + ambient art (T5), panels (T6), density graph (T7 + T10 data), difficulty grid (T8 + T11 render), wheel (T9 + T11), search (T12), Settings rename + rail + descriptions (T14), loading hero card + real progress (T15), error states (empty library T11 Step 1, missing art → black T11 Step 3 + T15, no scores → "no play" T8), screenshots (T16).
- Deviation from spec, accepted: `ExitChoreo` dropped — the existing 300ms `ScreenFade` already covers screen exit; enter choreography rides on fade-in. Recorded here so the spec's motion table row "Screen exit: reverse, 150ms" is knowingly simplified.
- User revision applied (2026-07-05): decorative streaks and gradient lines removed everywhere — plain black stage + ambient art only. `BackgroundGradient`/`Rot2` APIs no longer used.
- `notes_total()` (disk re-parse) is no longer called by song select; density/total come from the async `chart_stats` task.
- Type consistency checked: `WheelSpring(SpringValue)`, `DensityData{lanes,total}`, `DifficultyGridData{slots,selected}`, `BadgeValueText{decimals}`, `EnterChoreo::slide`, `set_panel_selected(theme, bool, &mut BorderColor, &mut BoxShadow)` used identically in T6/T11/T14.
- Known compile-risk spots called out inline: `ChildSpawnerCommands` name (T5 note), `spawn_config` re-invocation (T14 note), `EChannel` variant names (T10 note). The implementer resolves these against the workspace, not by redesigning.
