# Lane Arrangement (Layout Pillar, Plan 1 of 3) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace compile-time drum lane geometry (`COLUMNS`/`column_of`) with a runtime, file-persisted `LaneArrangement` (display order, widths, channel→lane mapping, presets) loaded from `layout.toml`.

**Architecture:** New pure-data crate `dtx-layout` (serde types, presets, `layout.toml` I/O with version field — mirrors `dtx-config` patterns, no bevy). `gameplay-drums` gains a `Lanes` resource wrapping `LaneArrangement`; `PlayfieldLayout` computes per-column geometry from it; note spawn/scroll, key caps, and HUD strip anchors read the resource instead of consts. Judgment/hit-groups are untouched (already implemented in `drum_groups.rs`).

**Tech Stack:** Rust, Bevy 0.19, serde + toml. Parent spec: `docs/superpowers/specs/2026-07-07-layout-editor-design.md`.

**Plan sequence:** This is plan 1 of 3 for the layout-editor pillar. Plan 2 = widget registry + layout persistence for HUD widgets. Plan 3 = the editor overlay. This plan must leave the game visually identical by default (classic preset == today's hardcoded geometry).

**Project gotchas (read first):**
- NEVER run `cargo fmt --all` or `cargo fmt -p <crate>` — the local rustfmt version reformats ~26 unrelated files. Only run `rustfmt` on wholly-new files you created. After any format, check `git status` and `git checkout --` stray files.
- Green tests don't prove the real `gameplay_drums::plugin` FixedUpdate schedule builds (test harnesses hand-wire `Update` apps). This plan only touches `Update`-schedule systems, so no new ordering-guard edges are needed — but do not add any `.before/.after` FixedUpdate edges without extending `crates/gameplay-drums/tests/fixed_update_schedule_ordering.rs`.
- Workspace test command: `cargo test -p <crate>` per crate; full: `cargo test --workspace`.

---

## File Structure

```
crates/dtx-layout/                    NEW crate — pure data, NO bevy
  Cargo.toml
  src/lib.rs                          errors, re-exports, default_path, load/save
  src/lanes.rs                        DisplayLane, LaneArrangement, channel names,
                                      per-channel default widths, resolve helpers
  src/presets.rs                      classic(), nx_type_b(), nx_type_d(), LanePreset
  src/file.rs                         LayoutFile (version), LanesSection (serde),
                                      resolution file→LaneArrangement, migrations

crates/gameplay-drums/
  src/lanes.rs                        NEW — `Lanes` Resource wrapper + color/hollow
  src/lane_geometry.rs                DELETED (logic moves to dtx-layout + lanes.rs)
  src/layout.rs                       PlayfieldLayout computes cols from Lanes
  src/scroll.rs                       column lookups via Lanes
  src/keyboard_viz.rs                 caps iterate Lanes; respawn on change
  src/hud.rs                          strip ref anchors via PlayfieldLayout methods
  src/lib.rs                          register lanes module + startup load
  tests/lane_arrangement.rs           NEW — integration: parity, split, merge
```

Consumers of the old consts (verified by grep): `hud.rs`, `scroll.rs`, `layout.rs`, `keyboard_viz.rs` only. `judge.rs`/`drum_groups.rs`/`lane_map.rs` use the 12-lane *input* model and are untouched.

---

### Task 1: dtx-layout crate scaffold + core lane types

**Files:**
- Create: `crates/dtx-layout/Cargo.toml`
- Create: `crates/dtx-layout/src/lib.rs`
- Create: `crates/dtx-layout/src/lanes.rs`

- [ ] **Step 1: Create the crate manifest**

`crates/dtx-layout/Cargo.toml` (mirrors `dtx-config`'s manifest style):

```toml
[package]
name = "dtx-layout"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true
description = "User layout persistence (lanes + HUD widget placement). Pure data, no bevy."

[lints.rust]
unsafe_code = "forbid"

[dependencies]
serde = { version = "1", features = ["derive"] }
toml = "0.8"
thiserror = { workspace = true }
dtx-core = { path = "../dtx-core" }
```

- [ ] **Step 2: Write failing tests for channel short names + core types**

`crates/dtx-layout/src/lanes.rs` — write the test module first (the file won't compile yet; that's the failing state):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::EChannel;

    #[test]
    fn short_name_round_trip_for_all_drum_channels() {
        for ch in DRUM_CHANNELS {
            let name = channel_short_name(ch).expect("drum channel has a name");
            assert_eq!(channel_from_short(name), Some(ch), "round trip {name}");
        }
    }

    #[test]
    fn non_drum_channels_have_no_short_name() {
        assert_eq!(channel_short_name(EChannel::BGM), None);
        assert_eq!(channel_short_name(EChannel::BarLine), None);
    }

    #[test]
    fn default_width_defined_for_every_drum_channel() {
        for ch in DRUM_CHANNELS {
            assert!(default_lane_width(ch) >= MIN_LANE_WIDTH);
            assert!(default_lane_width(ch) <= MAX_LANE_WIDTH);
        }
    }

    #[test]
    fn arrangement_lane_index_lookup() {
        let arr = crate::presets::classic();
        let hh = arr.lane_index_of(EChannel::HiHatClose).unwrap();
        let hho = arr.lane_index_of(EChannel::HiHatOpen).unwrap();
        assert_eq!(hh, hho, "classic merges HHO into HH lane");
        assert_eq!(arr.lanes[hh].id, "HH");
    }

    #[test]
    fn strip_width_is_sum_of_lane_widths() {
        let arr = crate::presets::classic();
        let sum: f32 = arr.lanes.iter().map(|l| l.width).sum();
        assert!((arr.strip_ref_width() - sum).abs() < f32::EPSILON);
    }
}
```

- [ ] **Step 3: Implement the core types in `lanes.rs`**

```rust
//! Display-lane model: order, widths, channel→lane mapping.
//!
//! DISPLAY axis only. Judgment-side pad grouping lives in
//! `dtx_config::DrumsConfig` + `gameplay-drums/src/drum_groups.rs` (NX port)
//! and is deliberately untouched by this crate.

use std::collections::HashMap;

use dtx_core::EChannel;

pub const MIN_LANE_WIDTH: f32 = 24.0;
pub const MAX_LANE_WIDTH: f32 = 160.0;

/// The 12 drum channels, canonical order (matches `lane_map::LANE_ORDER` labels).
pub const DRUM_CHANNELS: [EChannel; 12] = [
    EChannel::LeftCymbal,
    EChannel::HiHatClose,
    EChannel::HiHatOpen,
    EChannel::LeftPedal,
    EChannel::LeftBassDrum,
    EChannel::Snare,
    EChannel::HighTom,
    EChannel::BassDrum,
    EChannel::LowTom,
    EChannel::FloorTom,
    EChannel::Cymbal,
    EChannel::RideCymbal,
];

/// Canonical short name for a drum channel (used as lane ids + TOML keys).
pub fn channel_short_name(ch: EChannel) -> Option<&'static str> {
    Some(match ch {
        EChannel::LeftCymbal => "LC",
        EChannel::HiHatClose => "HH",
        EChannel::HiHatOpen => "HHO",
        EChannel::LeftPedal => "LP",
        EChannel::LeftBassDrum => "LBD",
        EChannel::Snare => "SD",
        EChannel::HighTom => "HT",
        EChannel::BassDrum => "BD",
        EChannel::LowTom => "LT",
        EChannel::FloorTom => "FT",
        EChannel::Cymbal => "CY",
        EChannel::RideCymbal => "RD",
        _ => return None,
    })
}

pub fn channel_from_short(name: &str) -> Option<EChannel> {
    DRUM_CHANNELS
        .into_iter()
        .find(|&ch| channel_short_name(ch) == Some(name))
}

/// Default ref-px width when a channel gets its own lane (ported from the
/// old `lane_geometry::COLUMNS` widths; split-out channels inherit their
/// merged sibling's width).
pub fn default_lane_width(ch: EChannel) -> f32 {
    match ch {
        EChannel::LeftCymbal => 72.0,
        EChannel::HiHatClose | EChannel::HiHatOpen => 49.0,
        EChannel::LeftPedal | EChannel::LeftBassDrum => 51.0,
        EChannel::Snare => 57.0,
        EChannel::HighTom => 49.0,
        EChannel::BassDrum => 69.0,
        EChannel::LowTom => 49.0,
        EChannel::FloorTom => 54.0,
        EChannel::Cymbal => 70.0,
        EChannel::RideCymbal => 38.0,
        _ => 49.0,
    }
}

/// One on-screen lane column.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayLane {
    /// Stable id — always a channel short name ("HH", "BD", …). v1 rule:
    /// lane ids are limited to channel short names so the primary channel
    /// is always derivable.
    pub id: String,
    pub label: String,
    /// Ref-px width, clamped to [MIN_LANE_WIDTH, MAX_LANE_WIDTH].
    pub width: f32,
    /// Base chip color (sRGB). `None` = derive from the primary channel's
    /// classic color at consumption time.
    pub color: Option<(f32, f32, f32)>,
    /// The channel this lane primarily represents (chips of other channels
    /// mapped here render as hollow "secondary" chips).
    pub primary: EChannel,
}

/// Runtime lane arrangement (display axis).
#[derive(Debug, Clone, PartialEq)]
pub struct LaneArrangement {
    pub preset: crate::presets::LanePreset,
    /// Display order left→right. Variable count (10 classic, 11 with HHO split…).
    pub lanes: Vec<DisplayLane>,
    /// Every drum channel maps to a lane id present in `lanes`.
    pub map: HashMap<EChannel, String>,
}

impl LaneArrangement {
    /// Index into `lanes` for a channel. None for non-drum channels.
    pub fn lane_index_of(&self, ch: EChannel) -> Option<usize> {
        let id = self.map.get(&ch)?;
        self.lanes.iter().position(|l| &l.id == id)
    }

    pub fn strip_ref_width(&self) -> f32 {
        self.lanes.iter().map(|l| l.width).sum()
    }

    /// Ref-px left offset of lane `i` measured from the strip's left edge.
    pub fn lane_ref_offset(&self, i: usize) -> f32 {
        self.lanes[..i].iter().map(|l| l.width).sum()
    }

    /// True when `ch` is a secondary chip on its lane (renders hollow).
    pub fn is_secondary(&self, ch: EChannel) -> bool {
        let Some(i) = self.lane_index_of(ch) else {
            return false;
        };
        self.lanes[i].primary != ch
    }
}
```

- [ ] **Step 4: Create `src/lib.rs` (minimal, grows in Task 4)**

```rust
//! dtx-layout — persisted user layout (lanes now; HUD widgets in plan 2).
//!
//! Pure data crate: serde types, presets, `layout.toml` I/O. No bevy.
//! Sibling of `dtx-config` (same XDG dir, separate file).

pub mod lanes;
pub mod presets;

pub use lanes::{
    channel_from_short, channel_short_name, default_lane_width, DisplayLane, LaneArrangement,
    DRUM_CHANNELS, MAX_LANE_WIDTH, MIN_LANE_WIDTH,
};
pub use presets::LanePreset;
```

Note: `presets` module doesn't exist yet — create a stub `src/presets.rs` with just the enum so Task 1 compiles standalone:

```rust
//! Built-in lane presets. Tables filled in by the presets task.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LanePreset {
    /// Current default: NX Type-A geometry, 10 columns.
    Classic,
    /// NX Type-B: pedals share one lane, SD left of pedals.
    NxTypeB,
    /// NX Type-D: symmetric pedals-center arrangement.
    NxTypeD,
    Custom,
}

impl Default for LanePreset {
    fn default() -> Self {
        Self::Classic
    }
}

/// Classic preset — implemented in the presets task; stub so lanes tests
/// referencing it fail (not error) until then is NOT possible in Rust, so
/// implement it here directly (it is the ground-truth port of the old
/// `lane_geometry::COLUMNS`).
pub fn classic() -> crate::lanes::LaneArrangement {
    use crate::lanes::{DisplayLane, LaneArrangement};
    use dtx_core::EChannel;
    use std::collections::HashMap;

    let spec: [(&str, f32, (f32, f32, f32), EChannel); 10] = [
        ("LC", 72.0, (0.945, 0.247, 0.725), EChannel::LeftCymbal),
        ("HH", 49.0, (0.000, 0.541, 1.000), EChannel::HiHatClose),
        ("LP", 51.0, (1.000, 0.353, 0.627), EChannel::LeftPedal),
        ("SD", 57.0, (0.941, 0.824, 0.000), EChannel::Snare),
        ("HT", 49.0, (0.157, 0.765, 0.157), EChannel::HighTom),
        ("BD", 69.0, (0.588, 0.353, 0.941), EChannel::BassDrum),
        ("LT", 49.0, (0.882, 0.176, 0.176), EChannel::LowTom),
        ("FT", 54.0, (1.000, 0.659, 0.000), EChannel::FloorTom),
        ("CY", 70.0, (1.000, 0.471, 0.000), EChannel::Cymbal),
        ("RD", 38.0, (0.000, 0.541, 1.000), EChannel::RideCymbal),
    ];

    let lanes = spec
        .iter()
        .map(|(id, w, c, primary)| DisplayLane {
            id: (*id).to_string(),
            label: (*id).to_string(),
            width: *w,
            color: Some(*c),
            primary: *primary,
        })
        .collect();

    let mut map = HashMap::new();
    for (id, _, _, primary) in &spec {
        map.insert(*primary, (*id).to_string());
    }
    map.insert(EChannel::HiHatOpen, "HH".to_string());
    map.insert(EChannel::LeftBassDrum, "BD".to_string());

    LaneArrangement {
        preset: LanePreset::Classic,
        lanes,
        map,
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p dtx-layout`
Expected: PASS (all 5 tests in `lanes::tests`).

- [ ] **Step 6: Format the new files (new crate only) and commit**

```bash
rustfmt crates/dtx-layout/src/lib.rs crates/dtx-layout/src/lanes.rs crates/dtx-layout/src/presets.rs
git status   # verify ONLY dtx-layout files changed
git add crates/dtx-layout
git commit -m "feat(dtx-layout): new crate with display-lane model + classic preset"
```

---

### Task 2: NX Type-B / Type-D presets + preset completeness tests

**Files:**
- Modify: `crates/dtx-layout/src/presets.rs`

- [ ] **Step 1: Write failing tests (append to `presets.rs`)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lanes::DRUM_CHANNELS;

    fn assert_complete(arr: &crate::lanes::LaneArrangement) {
        for ch in DRUM_CHANNELS {
            let idx = arr.lane_index_of(ch);
            assert!(idx.is_some(), "{ch:?} must map to an existing lane");
        }
        for lane in &arr.lanes {
            assert!(
                crate::lanes::channel_from_short(&lane.id).is_some(),
                "lane id {} must be a channel short name",
                lane.id
            );
        }
    }

    #[test]
    fn all_presets_are_complete() {
        for arr in [classic(), nx_type_b(), nx_type_d()] {
            assert_complete(&arr);
        }
    }

    #[test]
    fn classic_matches_legacy_columns() {
        let arr = classic();
        let labels: Vec<&str> = arr.lanes.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(
            labels,
            ["LC", "HH", "LP", "SD", "HT", "BD", "LT", "FT", "CY", "RD"]
        );
        assert!((arr.strip_ref_width() - 558.0).abs() < 0.01);
    }

    #[test]
    fn type_b_merges_pedals_into_one_lane() {
        use dtx_core::EChannel;
        let arr = nx_type_b();
        let labels: Vec<&str> = arr.lanes.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(
            labels,
            ["LC", "HH", "SD", "LP", "BD", "HT", "LT", "FT", "CY", "RD"]
        );
        assert_eq!(
            arr.lane_index_of(EChannel::LeftBassDrum),
            arr.lane_index_of(EChannel::LeftPedal),
            "Type-B shares one pedal lane"
        );
        // BD stays its own lane (NX x-table: LP/LBD@476, BD@533).
        assert_ne!(
            arr.lane_index_of(EChannel::BassDrum),
            arr.lane_index_of(EChannel::LeftPedal)
        );
    }

    #[test]
    fn type_d_is_pedals_center_symmetric_order() {
        let arr = nx_type_d();
        let labels: Vec<&str> = arr.lanes.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(
            labels,
            ["LC", "HH", "SD", "HT", "LP", "BD", "LT", "FT", "CY", "RD"]
        );
    }

    #[test]
    fn preset_serde_names_are_kebab() {
        assert_eq!(
            toml::to_string(&std::collections::BTreeMap::from([("p", LanePreset::NxTypeB)]))
                .unwrap()
                .trim(),
            r#"p = "nx-type-b""#
        );
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p dtx-layout presets`
Expected: FAIL — `nx_type_b`/`nx_type_d` not found.

- [ ] **Step 3: Implement the two presets (append to `presets.rs`)**

Derived from NX `CStagePerfDrumsScreen.cs:456-463` x-tables, expressed as
order + map (widths reuse the classic per-lane widths — see
`docs/superpowers/specs/2026-07-07-layout-editor-design.md` §Lane arrangement):

```rust
use dtx_core::EChannel;

fn arrangement_from(
    preset: LanePreset,
    order: &[&str],
    extra_map: &[(EChannel, &str)],
) -> crate::lanes::LaneArrangement {
    use crate::lanes::{channel_from_short, default_lane_width, DisplayLane};
    use std::collections::HashMap;

    let classic = classic();
    let lanes: Vec<DisplayLane> = order
        .iter()
        .map(|id| {
            classic
                .lanes
                .iter()
                .find(|l| l.id == *id)
                .cloned()
                .unwrap_or_else(|| {
                    let primary = channel_from_short(id)
                        .expect("preset lane ids are channel short names");
                    DisplayLane {
                        id: (*id).to_string(),
                        label: (*id).to_string(),
                        width: default_lane_width(primary),
                        color: None,
                        primary,
                    }
                })
        })
        .collect();

    let mut map: HashMap<EChannel, String> = classic
        .map
        .iter()
        .map(|(ch, id)| (*ch, id.clone()))
        .collect();
    for (ch, id) in extra_map {
        map.insert(*ch, (*id).to_string());
    }
    // Repair: any channel whose lane isn't in `order` falls back to the
    // first lane (presets below never hit this; the resolver in file.rs
    // reuses this helper for user customs where it can happen).
    for ch in crate::lanes::DRUM_CHANNELS {
        let id = map.get(&ch).cloned().unwrap_or_default();
        if !lanes.iter().any(|l| l.id == id) {
            map.insert(ch, lanes[0].id.clone());
        }
    }

    crate::lanes::LaneArrangement { preset, lanes, map }
}

/// NX Type-B ("summarize 2 pedals"): LBD joins LP's lane, SD moves left of
/// the pedals. Reference x-table `{370,419,533,596,645,748,694,373,815,298,476,476}`.
pub fn nx_type_b() -> crate::lanes::LaneArrangement {
    arrangement_from(
        LanePreset::NxTypeB,
        &["LC", "HH", "SD", "LP", "BD", "HT", "LT", "FT", "CY", "RD"],
        &[(EChannel::LeftBassDrum, "LP")],
    )
}

/// NX Type-D (left-right symmetric, pedals center). Reference x-table
/// `{370,419,582,476,645,748,694,373,815,298,525,527}`.
pub fn nx_type_d() -> crate::lanes::LaneArrangement {
    arrangement_from(
        LanePreset::NxTypeD,
        &["LC", "HH", "SD", "HT", "LP", "BD", "LT", "FT", "CY", "RD"],
        &[(EChannel::LeftBassDrum, "LP")],
    )
}

/// Table lookup used by file resolution + (later) the editor preset dropdown.
pub fn arrangement_for(preset: LanePreset) -> crate::lanes::LaneArrangement {
    match preset {
        LanePreset::Classic => classic(),
        LanePreset::NxTypeB => nx_type_b(),
        LanePreset::NxTypeD => nx_type_d(),
        LanePreset::Custom => classic(), // custom is built by the file resolver
    }
}
```

Also add `arrangement_for` + the two new fns to `lib.rs` re-exports:

```rust
pub use presets::{arrangement_for, classic, nx_type_b, nx_type_d, LanePreset};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p dtx-layout`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-layout
git commit -m "feat(dtx-layout): NX Type-B/Type-D presets + completeness tests"
```

---

### Task 3: `[lanes]` file section + custom resolution

**Files:**
- Create: `crates/dtx-layout/src/file.rs`
- Modify: `crates/dtx-layout/src/lib.rs`

- [ ] **Step 1: Write failing tests (in `file.rs`)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::EChannel;

    #[test]
    fn empty_section_resolves_to_classic() {
        let arr = LanesSection::default().resolve();
        assert_eq!(arr, crate::presets::classic());
    }

    #[test]
    fn named_preset_wins_over_order() {
        let section = LanesSection {
            preset: crate::presets::LanePreset::NxTypeB,
            order: Some(vec!["RD".into(), "LC".into()]), // ignored: preset != custom
            ..Default::default()
        };
        assert_eq!(section.resolve(), crate::presets::nx_type_b());
    }

    #[test]
    fn custom_order_reorders_and_splits() {
        let section = LanesSection {
            preset: crate::presets::LanePreset::Custom,
            order: Some(
                ["LC", "HH", "HHO", "LP", "SD", "HT", "BD", "LT", "FT", "CY", "RD"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            ),
            map: Some([("HHO".to_string(), "HHO".to_string())].into()),
            ..Default::default()
        };
        let arr = section.resolve();
        assert_eq!(arr.lanes.len(), 11);
        let hho = arr.lane_index_of(EChannel::HiHatOpen).unwrap();
        let hh = arr.lane_index_of(EChannel::HiHatClose).unwrap();
        assert_ne!(hho, hh, "HHO split out into its own lane");
        assert_eq!(arr.lanes[hho].id, "HHO");
        assert!(!arr.is_secondary(EChannel::HiHatOpen), "own lane => primary");
    }

    #[test]
    fn custom_widths_clamped() {
        let section = LanesSection {
            preset: crate::presets::LanePreset::Custom,
            widths: Some(
                [("SD".to_string(), 999.0), ("BD".to_string(), 1.0)].into(),
            ),
            ..Default::default()
        };
        let arr = section.resolve();
        let sd = arr.lane_index_of(EChannel::Snare).unwrap();
        let bd = arr.lane_index_of(EChannel::BassDrum).unwrap();
        assert_eq!(arr.lanes[sd].width, crate::lanes::MAX_LANE_WIDTH);
        assert_eq!(arr.lanes[bd].width, crate::lanes::MIN_LANE_WIDTH);
    }

    #[test]
    fn unknown_lane_ids_and_channels_dropped() {
        let section = LanesSection {
            preset: crate::presets::LanePreset::Custom,
            order: Some(vec!["HH".into(), "NOPE".into(), "SD".into()]),
            map: Some(
                [
                    ("XX".to_string(), "HH".to_string()),   // unknown channel
                    ("CY".to_string(), "NOPE".to_string()), // unknown lane target
                ]
                .into(),
            ),
            ..Default::default()
        };
        let arr = section.resolve();
        assert!(arr.lanes.iter().all(|l| l.id != "NOPE"));
        // CY fell back: its classic lane ("CY") isn't in the order, so it
        // remaps to the first lane rather than vanishing.
        assert!(arr.lane_index_of(EChannel::Cymbal).is_some());
    }

    #[test]
    fn channels_never_unmapped_even_when_lane_removed() {
        let section = LanesSection {
            preset: crate::presets::LanePreset::Custom,
            order: Some(vec!["SD".into(), "BD".into()]), // 2-lane extreme
            ..Default::default()
        };
        let arr = section.resolve();
        for ch in crate::lanes::DRUM_CHANNELS {
            assert!(arr.lane_index_of(ch).is_some(), "{ch:?} must stay mapped");
        }
    }

    #[test]
    fn resolve_round_trips_through_section() {
        let arr = crate::presets::nx_type_d();
        let section = LanesSection::from_arrangement(&arr);
        assert_eq!(section.resolve(), arr);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p dtx-layout file`
Expected: FAIL — `LanesSection` not defined.

- [ ] **Step 3: Implement `LanesSection` + resolution**

```rust
//! `layout.toml` schema + resolution to runtime types.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::lanes::{
    channel_from_short, channel_short_name, default_lane_width, DisplayLane, LaneArrangement,
    DRUM_CHANNELS, MAX_LANE_WIDTH, MIN_LANE_WIDTH,
};
use crate::presets::{arrangement_for, classic, LanePreset};

/// `[lanes]` section of layout.toml. All fields optional — absent = preset default.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LanesSection {
    #[serde(default)]
    pub preset: LanePreset,
    /// Display order (lane ids = channel short names). Only used when
    /// `preset = "custom"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<String>>,
    /// Per-lane ref-px width overrides, keyed by lane id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widths: Option<HashMap<String, f32>>,
    /// Channel→lane overrides, keyed by channel short name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<HashMap<String, String>>,
}

impl LanesSection {
    /// Build the runtime arrangement. Named preset → its table verbatim.
    /// Custom → classic base + order/widths/map overrides, with graceful
    /// fallbacks (unknown ids dropped + warned, unmapped channels repaired).
    pub fn resolve(&self) -> LaneArrangement {
        if self.preset != LanePreset::Custom {
            return arrangement_for(self.preset);
        }

        let base = classic();

        // 1. Lane list from `order` (default: classic order). Unknown ids dropped.
        let order: Vec<String> = self
            .order
            .clone()
            .unwrap_or_else(|| base.lanes.iter().map(|l| l.id.clone()).collect())
            .into_iter()
            .filter(|id| {
                let known = channel_from_short(id).is_some();
                if !known {
                    eprintln!("dtx-layout: unknown lane id {id:?} dropped");
                }
                known
            })
            .collect();
        let order = if order.is_empty() {
            base.lanes.iter().map(|l| l.id.clone()).collect()
        } else {
            order
        };

        let mut lanes: Vec<DisplayLane> = order
            .iter()
            .map(|id| {
                base.lanes.iter().find(|l| &l.id == id).cloned().unwrap_or_else(|| {
                    let primary = channel_from_short(id).expect("filtered above");
                    DisplayLane {
                        id: id.clone(),
                        label: id.clone(),
                        width: default_lane_width(primary),
                        color: None,
                        primary,
                    }
                })
            })
            .collect();

        // 2. Width overrides, clamped.
        if let Some(widths) = &self.widths {
            for lane in &mut lanes {
                if let Some(w) = widths.get(&lane.id) {
                    lane.width = w.clamp(MIN_LANE_WIDTH, MAX_LANE_WIDTH);
                }
            }
        }

        // 3. Channel map: classic base + overrides; repair anything pointing
        //    at a lane not in the list (classic lane if present, else lane 0).
        let mut map: HashMap<dtx_core::EChannel, String> = base.map.clone();
        if let Some(overrides) = &self.map {
            for (ch_name, lane_id) in overrides {
                let Some(ch) = channel_from_short(ch_name) else {
                    eprintln!("dtx-layout: unknown channel {ch_name:?} in map dropped");
                    continue;
                };
                if lanes.iter().any(|l| &l.id == lane_id) {
                    map.insert(ch, lane_id.clone());
                } else {
                    eprintln!("dtx-layout: map target lane {lane_id:?} unknown, dropped");
                }
            }
        }
        for ch in DRUM_CHANNELS {
            let id = map.get(&ch).cloned().unwrap_or_default();
            if !lanes.iter().any(|l| l.id == id) {
                map.insert(ch, lanes[0].id.clone());
            }
        }

        LaneArrangement {
            preset: LanePreset::Custom,
            lanes,
            map,
        }
    }

    /// Inverse of `resolve` for saving (always writes the explicit custom form
    /// unless the arrangement IS a named preset).
    pub fn from_arrangement(arr: &LaneArrangement) -> Self {
        if arr.preset != LanePreset::Custom {
            return Self {
                preset: arr.preset,
                ..Default::default()
            };
        }
        Self {
            preset: LanePreset::Custom,
            order: Some(arr.lanes.iter().map(|l| l.id.clone()).collect()),
            widths: Some(
                arr.lanes
                    .iter()
                    .map(|l| (l.id.clone(), l.width))
                    .collect(),
            ),
            map: Some(
                arr.map
                    .iter()
                    .filter_map(|(ch, id)| {
                        channel_short_name(*ch).map(|n| (n.to_string(), id.clone()))
                    })
                    .collect(),
            ),
        }
    }
}
```

Add to `lib.rs`:

```rust
pub mod file;
pub use file::LanesSection;
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p dtx-layout`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-layout
git commit -m "feat(dtx-layout): [lanes] section schema + custom resolution with fallbacks"
```

---

### Task 4: `layout.toml` load/save + version + migration hook

**Files:**
- Modify: `crates/dtx-layout/src/file.rs`
- Modify: `crates/dtx-layout/src/lib.rs`

- [ ] **Step 1: Write failing tests (append to `file.rs` tests)**

```rust
    #[test]
    fn layout_file_round_trip() {
        let file = LayoutFile {
            version: LATEST_VERSION,
            lanes: LanesSection::from_arrangement(&crate::presets::nx_type_b()),
        };
        let toml_str = toml::to_string_pretty(&file).unwrap();
        let back: LayoutFile = toml::from_str(&toml_str).unwrap();
        assert_eq!(back, file);
    }

    #[test]
    fn missing_file_loads_defaults() {
        let loaded = crate::load(std::path::Path::new("/nonexistent/layout.toml"));
        assert_eq!(loaded.lanes.resolve(), crate::presets::classic());
    }

    #[test]
    fn corrupt_file_loads_defaults() {
        let dir = std::env::temp_dir().join("dtx-layout-test-corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("layout.toml");
        std::fs::write(&path, "this is [ not toml").unwrap();
        let loaded = crate::load(&path);
        assert_eq!(loaded.lanes.resolve(), crate::presets::classic());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join("dtx-layout-test-save");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("layout.toml");
        let file = LayoutFile {
            version: LATEST_VERSION,
            lanes: LanesSection::from_arrangement(&crate::presets::nx_type_d()),
        };
        crate::save(&path, &file).unwrap();
        let loaded = crate::load(&path);
        assert_eq!(loaded, file);
    }

    #[test]
    fn version_zero_migrates_to_latest() {
        // v0 = same schema, version field absent. Migration chain = identity
        // today; this test pins the hook so future migrations have a slot.
        let loaded: LayoutFile = parse_with_migrations("[lanes]\npreset = \"classic\"\n");
        assert_eq!(loaded.version, LATEST_VERSION);
    }

    #[test]
    fn newer_version_still_parses_best_effort() {
        let loaded: LayoutFile =
            parse_with_migrations("version = 999\n[lanes]\npreset = \"nx-type-b\"\n");
        assert_eq!(loaded.lanes.preset, crate::presets::LanePreset::NxTypeB);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p dtx-layout file`
Expected: FAIL — `LayoutFile`, `LATEST_VERSION`, `parse_with_migrations` missing.

- [ ] **Step 3: Implement file type + migrations in `file.rs`**

```rust
pub const LATEST_VERSION: u32 = 1;

/// Whole layout.toml. Plan 2 adds `scene: SceneSection` for HUD widgets —
/// the schema is intentionally a struct (not just lanes) from day one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutFile {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub lanes: LanesSection,
}

impl Default for LayoutFile {
    fn default() -> Self {
        Self {
            version: LATEST_VERSION,
            lanes: LanesSection::default(),
        }
    }
}

/// Parse raw TOML, running the version migration chain. Best-effort on
/// newer-than-known versions (parse what matches, warn).
pub fn parse_with_migrations(raw: &str) -> LayoutFile {
    let mut file: LayoutFile = match toml::from_str(raw) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("dtx-layout: parse failed: {e}; using defaults");
            return LayoutFile::default();
        }
    };
    if file.version > LATEST_VERSION {
        eprintln!(
            "dtx-layout: layout.toml version {} newer than supported {}; best-effort load",
            file.version, LATEST_VERSION
        );
        return file;
    }
    // Migration chain: match each historical version in order. v0 (missing
    // field) → v1 is schema-identical.
    #[allow(clippy::single_match)]
    match file.version {
        0 => file.version = 1,
        _ => {}
    }
    file
}
```

And in `lib.rs` add path + I/O (dtx-config pattern):

```rust
use std::path::{Path, PathBuf};

use thiserror::Error;

pub use file::{parse_with_migrations, LanesSection, LayoutFile, LATEST_VERSION};

#[derive(Debug, Error)]
pub enum LayoutError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialize: {0}")]
    Serialize(#[from] toml::ser::Error),
}

/// `$XDG_CONFIG_HOME/dtxmaniars/layout.toml` → `$HOME/.config/dtxmaniars/layout.toml`
/// → `layout.toml` (cwd fallback). Same directory as dtx-config's config.toml.
pub fn default_path() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        let mut p = PathBuf::from(xdg);
        p.push("dtxmaniars");
        p.push("layout.toml");
        return p;
    }
    if let Some(home) = std::env::var_os("HOME") {
        let mut p = PathBuf::from(home);
        p.push(".config");
        p.push("dtxmaniars");
        p.push("layout.toml");
        return p;
    }
    PathBuf::from("layout.toml")
}

/// Load layout. Missing/corrupt file → defaults; never panics, never writes.
pub fn load(path: &Path) -> LayoutFile {
    match std::fs::read_to_string(path) {
        Ok(s) => parse_with_migrations(&s),
        Err(_) => LayoutFile::default(),
    }
}

/// Save layout, creating parent dirs.
pub fn save(path: &Path, file: &LayoutFile) -> Result<(), LayoutError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, toml::to_string_pretty(file)?)?;
    Ok(())
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p dtx-layout`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-layout
git commit -m "feat(dtx-layout): layout.toml load/save with version + migration hook"
```

---

### Task 5: gameplay-drums `Lanes` resource (replaces `lane_geometry.rs`)

**Files:**
- Create: `crates/gameplay-drums/src/lanes.rs`
- Delete: `crates/gameplay-drums/src/lane_geometry.rs` (in Task 9, after consumers migrate)
- Modify: `crates/gameplay-drums/Cargo.toml` (add `dtx-layout = { path = "../dtx-layout" }`)
- Modify: `crates/gameplay-drums/src/lib.rs` (add `pub mod lanes;`)

This task creates the new module alongside the old one; consumers switch in
Tasks 6-9, and the old file dies in Task 9. Both compile in parallel meanwhile.

- [ ] **Step 1: Write failing tests (in new `lanes.rs`)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::EChannel;

    /// Golden parity: classic arrangement reproduces the legacy COLUMNS
    /// geometry exactly (positions were `ref_x - 295` offsets from strip left).
    #[test]
    fn classic_matches_legacy_geometry() {
        let lanes = Lanes::default();
        let legacy: [(&str, f32, f32); 10] = [
            ("LC", 0.0, 72.0),
            ("HH", 72.0, 49.0),
            ("LP", 121.0, 51.0),
            ("SD", 172.0, 57.0),
            ("HT", 229.0, 49.0),
            ("BD", 278.0, 69.0),
            ("LT", 347.0, 49.0),
            ("FT", 396.0, 54.0),
            ("CY", 450.0, 70.0),
            ("RD", 520.0, 38.0),
        ];
        assert_eq!(lanes.count(), 10);
        for (i, (label, off, w)) in legacy.iter().enumerate() {
            assert_eq!(lanes.label(i), *label);
            assert!((lanes.ref_offset(i) - off).abs() < 0.01, "lane {label} offset");
            assert!((lanes.ref_width(i) - w).abs() < 0.01, "lane {label} width");
        }
        assert!((lanes.strip_ref_width() - 558.0).abs() < 0.01);
    }

    #[test]
    fn col_of_matches_legacy_mapping() {
        let lanes = Lanes::default();
        assert_eq!(lanes.col_of(EChannel::LeftCymbal), Some(0));
        assert_eq!(
            lanes.col_of(EChannel::HiHatOpen),
            lanes.col_of(EChannel::HiHatClose)
        );
        assert_eq!(
            lanes.col_of(EChannel::LeftBassDrum),
            lanes.col_of(EChannel::BassDrum)
        );
        assert_eq!(lanes.col_of(EChannel::BGM), None);
    }

    #[test]
    fn secondary_chips_hollow_and_tinted() {
        let lanes = Lanes::default();
        assert!(lanes.is_hollow(EChannel::HiHatOpen));
        assert!(lanes.is_hollow(EChannel::LeftBassDrum));
        assert!(!lanes.is_hollow(EChannel::HiHatClose));
        assert_ne!(
            lanes.chip_color(EChannel::HiHatOpen),
            lanes.chip_color(EChannel::HiHatClose)
        );
        assert_ne!(
            lanes.chip_color(EChannel::LeftBassDrum),
            lanes.chip_color(EChannel::BassDrum)
        );
    }

    #[test]
    fn split_arrangement_gives_hho_its_own_column() {
        let section = dtx_layout::LanesSection {
            preset: dtx_layout::LanePreset::Custom,
            order: Some(
                ["LC", "HH", "HHO", "LP", "SD", "HT", "BD", "LT", "FT", "CY", "RD"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            ),
            map: Some([("HHO".to_string(), "HHO".to_string())].into()),
            ..Default::default()
        };
        let lanes = Lanes(section.resolve());
        assert_eq!(lanes.count(), 11);
        assert_ne!(
            lanes.col_of(EChannel::HiHatOpen),
            lanes.col_of(EChannel::HiHatClose)
        );
        assert!(!lanes.is_hollow(EChannel::HiHatOpen));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p gameplay-drums lanes`
Expected: FAIL — module/type missing.

- [ ] **Step 3: Implement `Lanes` resource**

```rust
//! Runtime lane arrangement resource (display axis).
//!
//! Wraps `dtx_layout::LaneArrangement`. Replaces the old compile-time
//! `lane_geometry::COLUMNS`. Judgment-side grouping stays in `drum_groups.rs`.

use bevy::prelude::*;
use dtx_core::EChannel;

/// Display lane arrangement. Default = classic (legacy NX Type-A geometry).
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct Lanes(pub dtx_layout::LaneArrangement);

impl Default for Lanes {
    fn default() -> Self {
        Self(dtx_layout::classic())
    }
}

impl Lanes {
    pub fn count(&self) -> usize {
        self.0.lanes.len()
    }

    pub fn label(&self, i: usize) -> &str {
        &self.0.lanes[i].label
    }

    /// Ref-px offset of column `i` from the strip's left edge.
    pub fn ref_offset(&self, i: usize) -> f32 {
        self.0.lane_ref_offset(i)
    }

    pub fn ref_width(&self, i: usize) -> f32 {
        self.0.lanes[i].width
    }

    pub fn strip_ref_width(&self) -> f32 {
        self.0.strip_ref_width()
    }

    /// Visual column for a channel (None for non-drum chips).
    pub fn col_of(&self, channel: EChannel) -> Option<usize> {
        self.0.lane_index_of(channel)
    }

    fn lane_base_color(&self, i: usize) -> (f32, f32, f32) {
        self.0.lanes[i].color.unwrap_or_else(|| {
            // Derive from the primary channel's classic color.
            let primary = self.0.lanes[i].primary;
            let classic = dtx_layout::classic();
            classic
                .lane_index_of(primary)
                .and_then(|ci| classic.lanes[ci].color)
                .unwrap_or((1.0, 1.0, 1.0))
        })
    }

    /// Column base color as a Bevy `Color`.
    pub fn column_color(&self, i: usize) -> Color {
        let (r, g, b) = if i < self.count() {
            self.lane_base_color(i)
        } else {
            (1.0, 1.0, 1.0)
        };
        Color::srgb(r, g, b)
    }

    /// Chip color: lane base, with the legacy secondary variants (HHO reads
    /// brighter, LBD darker) applied when the chip is a secondary on its lane.
    pub fn chip_color(&self, channel: EChannel) -> Color {
        let Some(col) = self.col_of(channel) else {
            return Color::WHITE;
        };
        let (r, g, b) = self.lane_base_color(col);
        if !self.0.is_secondary(channel) {
            return Color::srgb(r, g, b);
        }
        match channel {
            EChannel::HiHatOpen => {
                Color::srgb((r + 0.25).min(1.0), (g + 0.15).min(1.0), 1.0)
            }
            EChannel::LeftBassDrum => Color::srgb(r * 0.6, g * 0.6, b * 0.6),
            _ => Color::srgb(r, g, b),
        }
    }

    /// Secondary chips render hollow (outline) to stay distinct from the
    /// filled primary sharing their lane.
    pub fn is_hollow(&self, channel: EChannel) -> bool {
        self.0.is_secondary(channel)
    }
}
```

`crates/gameplay-drums/Cargo.toml` — add to `[dependencies]`:

```toml
dtx-layout = { path = "../dtx-layout" }
```

`crates/gameplay-drums/src/lib.rs` — add `pub mod lanes;` next to the other
modules, and in the plugin body add `.init_resource::<lanes::Lanes>()`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p gameplay-drums lanes`
Expected: PASS. Also `cargo build -p gameplay-drums` — old `lane_geometry` still present + compiling (unused-parallel is fine).

- [ ] **Step 5: Format the new file only, then commit**

```bash
rustfmt crates/gameplay-drums/src/lanes.rs
git status   # only intended files
git add crates/gameplay-drums
git commit -m "feat(gameplay-drums): Lanes resource wrapping dtx-layout arrangement"
```

---

### Task 6: `PlayfieldLayout` computes columns from `Lanes`

**Files:**
- Modify: `crates/gameplay-drums/src/layout.rs`

- [ ] **Step 1: Update layout tests (same file) to the new constructor and add a lanes-driven case**

Replace the existing `tests` module in `layout.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lanes::Lanes;
    use dtx_ui::theme::REF_WIDTH;

    fn classic_layout() -> PlayfieldLayout {
        PlayfieldLayout::from_size(REF_WIDTH, REF_HEIGHT, &Lanes::default())
    }

    #[test]
    fn judge_below_lane_top() {
        let layout = classic_layout();
        assert!(layout.judge_y() > layout.lane_top());
    }

    #[test]
    fn lane_height_spans_to_judge() {
        let layout = classic_layout();
        assert!(
            (layout.lane_top() + layout.lane_height() - layout.judge_y()).abs() < 1.0,
            "lane bottom should align with judge line"
        );
    }

    #[test]
    fn strip_centered_at_default_scale() {
        let layout = classic_layout(); // scale 1.0 at 1280x720
        let expected_left = (REF_WIDTH - 558.0) / 2.0; // 361.0
        assert!((layout.strip_left() - expected_left).abs() < 0.01);
        assert!((layout.col_left(0) - expected_left).abs() < 0.01);
        let last = layout.col_count() - 1;
        assert!(
            (layout.col_left(last) + layout.col_width(last) - (expected_left + 558.0)).abs()
                < 0.5,
            "strip right edge should be centered"
        );
    }

    #[test]
    fn columns_monotonic() {
        let layout = classic_layout();
        for c in 1..layout.col_count() {
            assert!(layout.col_left(c) > layout.col_left(c - 1));
        }
    }

    #[test]
    fn strip_width_matches_ref() {
        let layout = classic_layout();
        assert!((layout.strip_width() - 558.0).abs() < 0.5);
    }

    #[test]
    fn wider_arrangement_widens_and_recenters_strip() {
        let section = dtx_layout::LanesSection {
            preset: dtx_layout::LanePreset::Custom,
            order: Some(
                ["LC", "HH", "HHO", "LP", "SD", "HT", "BD", "LT", "FT", "CY", "RD"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            ),
            map: Some([("HHO".to_string(), "HHO".to_string())].into()),
            ..Default::default()
        };
        let lanes = Lanes(section.resolve());
        let layout = PlayfieldLayout::from_size(REF_WIDTH, REF_HEIGHT, &lanes);
        assert_eq!(layout.col_count(), 11);
        assert!((layout.strip_width() - (558.0 + 49.0)).abs() < 0.01);
        let expected_left = (REF_WIDTH - (558.0 + 49.0)) / 2.0;
        assert!((layout.strip_left() - expected_left).abs() < 0.01);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p gameplay-drums layout`
Expected: FAIL — `from_size` signature, `col_count` missing.

- [ ] **Step 3: Rework `PlayfieldLayout`**

Replace the struct, constructors, column methods, and the two systems.
The scalar methods (`judge_y`, `lane_top`, `px`, `note_height`, `key_*`,
`backboard_*`, `progress_bar_*`, `combo_*`, `measure_label_left`) keep their
bodies — only strip/column internals change. Remove the
`use crate::lane_geometry::…` import and the `STRIP_REF_CENTERED_LEFT` const
and `col_ref_x`/`ref_phrase_x`/`ref_hud_right_x` free functions (they become
methods):

```rust
use crate::lanes::Lanes;

#[derive(Resource, Clone, Debug)]
pub struct PlayfieldLayout {
    pub scale: f32,
    pub width: f32,
    pub height: f32,
    /// Total strip width in ref px (sum of lane widths).
    strip_ref_width: f32,
    /// Per-column (ref-offset-from-strip-left, ref-width).
    cols: Vec<(f32, f32)>,
}

impl Default for PlayfieldLayout {
    fn default() -> Self {
        Self::from_size(REF_WIDTH, REF_HEIGHT, &Lanes::default())
    }
}

impl PlayfieldLayout {
    pub fn from_window(window: &Window, lanes: &Lanes) -> Self {
        Self::from_size(window.width(), window.height(), lanes)
    }

    pub fn from_size(width: f32, height: f32, lanes: &Lanes) -> Self {
        let scale = (width / REF_WIDTH).min(height / REF_HEIGHT);
        let cols = (0..lanes.count())
            .map(|i| (lanes.ref_offset(i), lanes.ref_width(i)))
            .collect();
        Self {
            scale,
            width,
            height,
            strip_ref_width: lanes.strip_ref_width(),
            cols,
        }
    }

    /// Centered strip left edge in REF px (was `STRIP_REF_CENTERED_LEFT`).
    pub fn ref_strip_left(&self) -> f32 {
        (REF_WIDTH - self.strip_ref_width) / 2.0
    }

    pub fn ref_strip_width(&self) -> f32 {
        self.strip_ref_width
    }

    /// Phrase meter ref-x, just right of the strip (was free fn `ref_phrase_x`).
    pub fn ref_phrase_x(&self) -> f32 {
        self.ref_strip_left() + self.strip_ref_width + 15.0
    }

    /// Right HUD column ref-x anchor (was free fn `ref_hud_right_x`).
    pub fn ref_hud_right_x(&self) -> f32 {
        self.ref_strip_left() + self.strip_ref_width + 24.0
    }

    pub fn col_count(&self) -> usize {
        self.cols.len()
    }

    pub fn col_left(&self, col: usize) -> f32 {
        (self.ref_strip_left() + self.cols[col].0) * self.scale
    }

    pub fn col_width(&self, col: usize) -> f32 {
        self.cols[col].1 * self.scale
    }

    pub fn strip_left(&self) -> f32 {
        self.ref_strip_left() * self.scale
    }

    pub fn strip_width(&self) -> f32 {
        self.strip_ref_width * self.scale
    }

    // …all remaining scalar methods unchanged from the current file…
}
```

Update the two systems to feed `Lanes` in and react to changes:

```rust
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<PlayfieldLayout>()
        .add_systems(Startup, init_playfield_layout)
        .add_systems(
            Update,
            sync_playfield_layout
                .run_if(in_state(AppState::Performance).or_else(in_state(AppState::SongLoading))),
        );
}

fn init_playfield_layout(
    mut layout: ResMut<PlayfieldLayout>,
    lanes: Res<Lanes>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    if let Ok(window) = windows.single() {
        *layout = PlayfieldLayout::from_window(window, &lanes);
    }
}

fn sync_playfield_layout(
    mut resize_events: MessageReader<WindowResized>,
    windows: Query<&Window, With<PrimaryWindow>>,
    lanes: Res<Lanes>,
    mut layout: ResMut<PlayfieldLayout>,
    mut dirty: Local<bool>,
) {
    if resize_events.read().next().is_some() || lanes.is_changed() {
        *dirty = true;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    if !*dirty {
        // Cheap early-out: nothing changed since last sync.
        let next_scale = (window.width() / REF_WIDTH).min(window.height() / REF_HEIGHT);
        if next_scale == layout.scale && window.width() == layout.width {
            return;
        }
    }
    *layout = PlayfieldLayout::from_window(window, &lanes);
    *dirty = false;
}
```

Note `PlayfieldLayout` loses `Copy` (holds a `Vec`); it was only ever passed
by reference or as a resource, so no call-site changes beyond what later tasks
already touch. If the compiler flags a stray `*layout` copy elsewhere, clone
explicitly.

- [ ] **Step 4: Run tests + build**

Run: `cargo test -p gameplay-drums layout`
Expected: layout tests PASS. `cargo build -p gameplay-drums` will FAIL at
`hud.rs`/`scroll.rs`/`keyboard_viz.rs` call sites (old free fns gone) — that
is expected; fix them in Tasks 7-9 before committing if the crate must build,
OR keep the old free fns as thin deprecated wrappers for now. **Do the
latter**: add at the bottom of `layout.rs` temporary wrappers so the crate
keeps building between tasks:

```rust
/// TEMPORARY (removed in Task 9): legacy free-fn shims for unmigrated callers.
pub const STRIP_REF_CENTERED_LEFT: f32 = (REF_WIDTH - 558.0) / 2.0;

pub fn ref_phrase_x() -> f32 {
    STRIP_REF_CENTERED_LEFT + 558.0 + 15.0
}

pub fn ref_hud_right_x() -> f32 {
    STRIP_REF_CENTERED_LEFT + 558.0 + 24.0
}
```

Run: `cargo test -p gameplay-drums`
Expected: PASS (whole crate).

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/layout.rs
git commit -m "feat(gameplay-drums): PlayfieldLayout derives columns from Lanes resource"
```

---

### Task 7: migrate `scroll.rs` to `Lanes`

**Files:**
- Modify: `crates/gameplay-drums/src/scroll.rs`

- [ ] **Step 1: Swap imports and add the resource param**

Replace `use crate::lane_geometry::{chip_color, column_of, is_hollow};` with
`use crate::lanes::Lanes;`.

In `spawn_notes_system`, add param `lanes: Res<Lanes>` and change the body:

```rust
        let Some(col) = lanes.col_of(chip.channel) else {
            continue;
        };
        let top = top_for_note(target_ms, now, judge_y, px_per_ms);
        let left = layout.col_left(col) + 2.0;
        let color = lanes.chip_color(chip.channel);
        let hollow = lanes.is_hollow(chip.channel);
```

In `reposition_notes_on_layout_change`, add param `lanes: Res<Lanes>`:

```rust
    for (note, mut node) in &mut notes {
        let Some(channel) = lane_channel(note.lane) else {
            continue;
        };
        let Some(col) = lanes.col_of(channel) else {
            continue;
        };
        node.left = Val::Px(layout.col_left(col) + 2.0);
        node.width = Val::Px(layout.note_width(col));
        node.height = Val::Px(layout.note_height());
        node.border = if lanes.is_hollow(channel) {
            UiRect::all(Val::Px(2.0 * layout.scale))
        } else {
            UiRect::ZERO
        };
    }
```

`reposition_notes_on_layout_change` already runs on
`resource_changed::<PlayfieldLayout>`, and Task 6 made `PlayfieldLayout`
change whenever `Lanes` changes — so notes re-anchor on arrangement change
with no extra wiring.

- [ ] **Step 2: Build + run crate tests**

Run: `cargo test -p gameplay-drums`
Expected: PASS (scroll unit tests unchanged — they test pure math).

- [ ] **Step 3: Commit**

```bash
git add crates/gameplay-drums/src/scroll.rs
git commit -m "refactor(gameplay-drums): notes read lane columns from Lanes resource"
```

---

### Task 8: migrate `keyboard_viz.rs` + respawn caps on arrangement change

**Files:**
- Modify: `crates/gameplay-drums/src/keyboard_viz.rs`
- Modify: `crates/gameplay-drums/src/hud.rs` (spawn_key_caps call passes lanes)

- [ ] **Step 1: Swap to `Lanes` lookups**

Replace `use crate::lane_geometry::{column_color, column_of, COLUMNS, COLUMN_COUNT};`
with `use crate::lanes::Lanes;`.

`spawn_key_caps` gains a `lanes: &Lanes` param:

```rust
pub fn spawn_key_caps(
    commands: &mut Commands,
    parent: Entity,
    layout: &PlayfieldLayout,
    lanes: &Lanes,
    theme: &dtx_ui::theme::Theme,
) {
    let cap_h = layout.key_cap_height();
    for col in 0..lanes.count() {
        let rim = lanes.column_color(col);
        commands.entity(parent).with_children(|row| {
            row.spawn((
                KeyCap { col: col as u8 },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(layout.col_left(col)),
                    top: Val::Px(layout.key_viz_top()),
                    width: Val::Px(layout.col_width(col)),
                    height: Val::Px(cap_h),
                    border: UiRect::all(Val::Px(2.0 * layout.scale)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: key_cap_border_radius(cap_h, layout.scale),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.11, 0.11, 0.13)),
                BorderColor::all(rim),
                children![(
                    Text::new(lanes.label(col).to_string()),
                    Theme::font(13.0 * layout.scale),
                    TextColor(theme.text_primary),
                )],
            ));
        });
    }
}
```

`flash_key_caps_on_hit` — the `to_col` closure gains lanes:

```rust
fn flash_key_caps_on_hit(
    mut lane_hits: MessageReader<LaneHit>,
    mut events: MessageReader<JudgmentEvent>,
    lanes: Res<Lanes>,
    mut caps: Query<(&KeyCap, &mut BackgroundColor)>,
) {
    let to_col = |lane: u8| lane_channel(lane).and_then(|ch| lanes.col_of(ch));
```

and the judgment-flash color becomes `lanes.column_color(col).with_alpha(0.85)`.

Column count can now change at runtime → replace the layout-reapply system
with a **respawn** on lanes change and keep a cheap reposition for pure
resizes. In the plugin fn:

```rust
pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            flash_key_caps_on_hit,
            respawn_key_caps_on_lanes_change.run_if(resource_changed::<crate::lanes::Lanes>),
            apply_key_cap_layout.run_if(resource_changed::<PlayfieldLayout>),
        )
            .run_if(in_state(AppState::Performance)),
    );
}

/// Lane count/order can change at runtime (layout editor); rebuild the row.
fn respawn_key_caps_on_lanes_change(
    mut commands: Commands,
    lanes: Res<crate::lanes::Lanes>,
    layout: Res<PlayfieldLayout>,
    theme: Res<dtx_ui::theme::ThemeResource>,
    caps: Query<Entity, With<KeyCap>>,
    hud_root: Query<Entity, With<crate::hud::HudRoot>>,
) {
    let Ok(root) = hud_root.single() else {
        return;
    };
    for e in &caps {
        commands.entity(e).despawn();
    }
    spawn_key_caps(&mut commands, root, &layout, &lanes, &theme.0);
}
```

`apply_key_cap_layout` guards out-of-range cols (count may have shrunk in the
same frame before respawn ran):

```rust
fn apply_key_cap_layout(layout: Res<PlayfieldLayout>, mut caps: Query<(&KeyCap, &mut Node)>) {
    for (cap, mut node) in &mut caps {
        let col = cap.col as usize;
        if col >= layout.col_count() {
            continue;
        }
        // …body unchanged…
    }
}
```

- [ ] **Step 2: Fix the `hud.rs` call site**

In `spawn_hud` (crates/gameplay-drums/src/hud.rs), the call becomes:

```rust
    keyboard_viz::spawn_key_caps(&mut commands, root, &layout, &lanes, &t);
```

with `lanes: Res<crate::lanes::Lanes>` added to `spawn_hud`'s params.

- [ ] **Step 3: Build + tests**

Run: `cargo test -p gameplay-drums`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/gameplay-drums/src/keyboard_viz.rs crates/gameplay-drums/src/hud.rs
git commit -m "refactor(gameplay-drums): key caps driven by Lanes, respawn on arrangement change"
```

---

### Task 9: migrate `hud.rs` strip anchors, delete `lane_geometry.rs` + shims

**Files:**
- Modify: `crates/gameplay-drums/src/hud.rs`
- Modify: `crates/gameplay-drums/src/layout.rs` (remove Task 6 shims)
- Delete: `crates/gameplay-drums/src/lane_geometry.rs`
- Modify: `crates/gameplay-drums/src/lib.rs` (drop `mod lane_geometry;`)

- [ ] **Step 1: Replace strip-ref usages in `hud.rs`**

Current usages (hud.rs:146-186) use `STRIP_REF_CENTERED_LEFT`,
`lane_geometry::STRIP_REF_WIDTH`, `ref_phrase_x()`, `ref_hud_right_x()`.
Replace with layout methods (layout is already in scope in `spawn_hud`):

```rust
    frame_chrome::spawn_frame_chrome(
        &mut commands,
        root,
        &t,
        s,
        layout.ref_strip_left(),
        layout.ref_strip_left() + layout.ref_strip_width(),
    );
    score_detailed::spawn_score_detailed_panel(&mut commands, root, &t, s);
    phrase_meter::spawn_phrase_meter(&mut commands, root, &t, s, layout.ref_phrase_x());
    song_progress::spawn_song_progress(
        &mut commands,
        root,
        &t,
        s,
        layout.ref_strip_left(),
        layout.ref_strip_width(),
    );
    playfield_speed::spawn_playfield_speed(&mut commands, root, &t, s, 24.0, 470.0);
    let hud_right = layout.ref_hud_right_x();
    now_playing::spawn_now_playing(&mut commands, root, &t, s, hud_right);
    let combo_ref_x = layout.ref_strip_left() + layout.ref_strip_width() / 2.0 - 180.0;
    perf_combo::spawn_perf_combo(&mut commands, root, &t, s, combo_ref_x, 150.0);
```

Remove the now-dead `use crate::lane_geometry;` /
`use crate::layout::{STRIP_REF_CENTERED_LEFT, ref_phrase_x, ref_hud_right_x}`
imports from `hud.rs` (match whatever the current import lines are).

- [ ] **Step 2: Delete the shims from `layout.rs`**

Remove the `STRIP_REF_CENTERED_LEFT` const and `ref_phrase_x`/`ref_hud_right_x`
free fns added in Task 6 Step 4.

- [ ] **Step 3: Delete `lane_geometry.rs`**

```bash
git rm crates/gameplay-drums/src/lane_geometry.rs
```

Remove `mod lane_geometry;` (or `pub mod lane_geometry;`) from
`crates/gameplay-drums/src/lib.rs`. Grep for stragglers:

```bash
grep -rn "lane_geometry" crates/ --include="*.rs"
```

Expected: only a stale doc comment in `crates/dtx-ui/src/theme.rs:130` —
update that comment to say `gameplay-drums lanes.rs / dtx-layout classic()`.
`crates/gameplay-drums/src/lane_map.rs` doc header mentions lane_geometry in
prose only — update the sentence.

- [ ] **Step 4: Full crate tests**

Run: `cargo test -p gameplay-drums && cargo test -p dtx-ui`
Expected: PASS. The 7 tests that lived in `lane_geometry.rs` are superseded by
`lanes.rs` tests (`classic_matches_legacy_geometry` covers order/contiguity/
strip bounds; `col_of_matches_legacy_mapping` covers hho/lbd merge + non-drum
None; `secondary_chips_hollow_and_tinted` covers hollow/color variants).

- [ ] **Step 5: Commit**

```bash
git add -A crates/gameplay-drums crates/dtx-ui
git commit -m "refactor(gameplay-drums): delete lane_geometry consts; all geometry via Lanes"
```

---

### Task 10: startup load of layout.toml + integration tests

**Files:**
- Modify: `crates/gameplay-drums/src/lib.rs` (or wherever the drums plugin body lives — add startup system)
- Create: `crates/gameplay-drums/tests/lane_arrangement.rs`

- [ ] **Step 1: Write the failing integration test**

`crates/gameplay-drums/tests/lane_arrangement.rs` — follows the hand-wired
`MinimalPlugins` harness style of `tests/practice_mode.rs` (no real plugin;
per project gotcha these tests exercise systems, not the schedule):

```rust
//! Integration: lane arrangement drives note columns + playfield geometry.

use bevy::prelude::*;
use gameplay_drums::lanes::Lanes;
use gameplay_drums::layout::PlayfieldLayout;

fn lanes_from_section(section: dtx_layout::LanesSection) -> Lanes {
    Lanes(section.resolve())
}

fn split_hho_section() -> dtx_layout::LanesSection {
    dtx_layout::LanesSection {
        preset: dtx_layout::LanePreset::Custom,
        order: Some(
            ["LC", "HH", "HHO", "LP", "SD", "HT", "BD", "LT", "FT", "CY", "RD"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        ),
        map: Some([("HHO".to_string(), "HHO".to_string())].into()),
        ..Default::default()
    }
}

#[test]
fn default_lanes_reproduce_legacy_note_positions() {
    // Golden values: col_left at 1280x720 scale=1.0 equals legacy
    // STRIP_REF_CENTERED_LEFT (361) + (COLUMNS[i].ref_x - 295).
    let lanes = Lanes::default();
    let layout = PlayfieldLayout::from_size(1280.0, 720.0, &lanes);
    let legacy_ref_x = [
        295.0, 367.0, 416.0, 467.0, 524.0, 573.0, 642.0, 691.0, 745.0, 815.0,
    ];
    for (i, rx) in legacy_ref_x.iter().enumerate() {
        let expected = 361.0 + (rx - 295.0);
        assert!(
            (layout.col_left(i) - expected).abs() < 0.01,
            "col {i}: got {}, want {expected}",
            layout.col_left(i)
        );
    }
}

#[test]
fn split_arrangement_moves_hho_chips_to_own_column() {
    let lanes = lanes_from_section(split_hho_section());
    let hho = lanes.col_of(dtx_core::EChannel::HiHatOpen).unwrap();
    let hh = lanes.col_of(dtx_core::EChannel::HiHatClose).unwrap();
    assert_eq!(lanes.count(), 11);
    assert_ne!(hho, hh);
    // Split-out chip is a primary now: solid, not hollow.
    assert!(!lanes.is_hollow(dtx_core::EChannel::HiHatOpen));
}

#[test]
fn merge_cy_into_rd_lane() {
    let section = dtx_layout::LanesSection {
        preset: dtx_layout::LanePreset::Custom,
        order: Some(
            ["LC", "HH", "LP", "SD", "HT", "BD", "LT", "FT", "RD"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        ),
        map: Some([("CY".to_string(), "RD".to_string())].into()),
        ..Default::default()
    };
    let lanes = lanes_from_section(section);
    assert_eq!(lanes.count(), 9);
    assert_eq!(
        lanes.col_of(dtx_core::EChannel::Cymbal),
        lanes.col_of(dtx_core::EChannel::RideCymbal)
    );
    assert!(lanes.is_hollow(dtx_core::EChannel::Cymbal), "CY secondary on RD lane");
}

#[test]
fn lanes_change_recomputes_playfield_layout() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(Lanes::default());
    app.insert_resource(PlayfieldLayout::from_size(1280.0, 720.0, &Lanes::default()));
    app.add_systems(
        Update,
        |lanes: Res<Lanes>, mut layout: ResMut<PlayfieldLayout>| {
            if lanes.is_changed() {
                *layout = PlayfieldLayout::from_size(layout.width, layout.height, &lanes);
            }
        },
    );
    app.update();

    let before = app.world().resource::<PlayfieldLayout>().col_count();
    assert_eq!(before, 10);

    *app.world_mut().resource_mut::<Lanes>() = lanes_from_section(split_hho_section());
    app.update();

    let after = app.world().resource::<PlayfieldLayout>().col_count();
    assert_eq!(after, 11);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p gameplay-drums --test lane_arrangement`
Expected: FAIL to compile if `lanes`/`layout` modules aren't `pub` in
`lib.rs` — make `pub mod lanes;` and confirm `pub mod layout;` (check current
visibility; `layout` is referenced by tests in practice_mode.rs already).
Then tests should PASS except any actual behavior bug they catch.

- [ ] **Step 3: Startup load wiring**

In the gameplay-drums plugin body (`crates/gameplay-drums/src/lib.rs`), add a
startup system next to `init_resource::<lanes::Lanes>()`:

```rust
        .add_systems(Startup, load_lane_arrangement)
```

and the system at the bottom of `lib.rs`:

```rust
/// Load the user's lane arrangement from layout.toml (defaults on absence).
fn load_lane_arrangement(mut lanes: ResMut<lanes::Lanes>) {
    let file = dtx_layout::load(&dtx_layout::default_path());
    lanes.0 = file.lanes.resolve();
}
```

(Change detection: `ResMut` deref-write marks `Lanes` changed on the first
frame → `sync_playfield_layout` picks it up even when the file matches
defaults; that's a single redundant recompute, harmless.)

- [ ] **Step 4: Full workspace verification**

Run: `cargo test --workspace`
Expected: PASS everywhere. Then verify the real schedule builds by running the
binary briefly if a display is available (optional, manual):
`cargo run 2>&1 | head -20` — no panic during startup.

- [ ] **Step 5: Format new test file, commit**

```bash
rustfmt crates/gameplay-drums/tests/lane_arrangement.rs
git status
git add crates/gameplay-drums
git commit -m "feat(gameplay-drums): load lane arrangement from layout.toml at startup"
```

---

### Task 11: workspace docs touch-up + plan wrap

**Files:**
- Modify: `docs/superpowers/specs/2026-07-07-layout-editor-design.md` (status note only)

- [ ] **Step 1: Mark plan-1 scope delivered in the spec**

Add under the spec's `Status:` line:

```markdown
Progress: plan 1 (lane arrangement — dtx-layout crate, Lanes resource,
layout.toml `[lanes]`) implemented. Plans 2 (widget registry) and 3 (editor
overlay) pending.
```

- [ ] **Step 2: Final full test run**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/specs/2026-07-07-layout-editor-design.md
git commit -m "docs(specs): mark lane-arrangement plan delivered"
```

---

## Manual verification checklist (post-merge, needs display/audio)

- Default run: gameplay looks pixel-identical to before (classic parity).
- Hand-edit `~/.config/dtxmaniars/layout.toml` → `[lanes] preset = "nx-type-b"` → pedals share a lane, SD sits left of them.
- Custom split: order with `"HHO"` + `map = { HHO = "HHO" }` → 11 columns, HHO solid chips in its own lane; key caps show 11 caps; notes land correctly.
- Window resize with a custom arrangement keeps the strip centered.

## Explicitly out of scope (later plans)

- Widget registry / `[scene.gameplay]` section — plan 2.
- Editor overlay, lane drag UX, preset dropdown UI, hit-group dropdowns — plan 3.
- Any judgment change — hit groups already exist in `drum_groups.rs`/`DrumsConfig`.
