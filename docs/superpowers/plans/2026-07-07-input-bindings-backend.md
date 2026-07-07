# Input Bindings Backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the four disjoint hardcoded keybind/MIDI-mapping homes with one persisted `bindings.toml` (`EChannel`-keyed) that gameplay keyboard + MIDI input actually read.

**Architecture:** New serde types in `dtx-config/src/bindings.rs` (file schema `BindingsFile` + runtime `InputBindings`, mirroring the `layout.toml` `LanesSection::resolve` pattern). `dtx-input`'s `MidiSource` trait changes to emit raw `MidiEvent`s (mapping moves out). `gameplay-drums` gains a `BindResolver` resource (KeyCode→LaneId, note→LaneId, velocity threshold) built from `InputBindings`, consumed by the existing keyboard-capture and midi-consumer systems. Dead BocuD ports (`key_assign.rs`, `config_key_assign.rs`) are deleted.

**Tech Stack:** Rust, Bevy 0.19, serde + toml 0.8. Workspace crates: `dtx-core`, `dtx-config`, `dtx-input`, `dtx-layout`, `gameplay-drums`, `game-menu`.

**Spec:** `docs/superpowers/specs/2026-07-07-customize-surface-design.md` §3 (this plan = data model + wiring). Surface UI (§4–5) comes in follow-up plans after this lands.

**Conventions that bite:**
- NEVER run bare `cargo fmt --all` (local rustfmt version drift reformats unrelated files). Format only files you touched: `rustfmt --edition 2021 <files>`.
- Green unit tests do NOT prove the FixedUpdate schedule builds. Always finish with the full workspace test run which includes the headless schedule guard tests.
- Run tests with `cargo test -p <crate>` per task, `cargo test --workspace` at the end.
- Commits: conventional, no AI co-author lines.

**Intentional behavior changes (call out in commit messages):**
1. MIDI note 49 maps to LeftCymbal (GM Crash 1), not HiHatClose as today's `mapping.rs` wrongly does.
2. MIDI NoteOff no longer produces a gameplay hit (today's `midi_consumer` writes Release events as presses — latent bug).
3. Toms/cymbals GM notes now mapped (closes the "M6c partial" TODO).

---

## File structure

| File | Action | Responsibility |
|---|---|---|
| `crates/dtx-core/src/channel.rs` | Modify | `EChannel::short_name()` / `from_short_name()` for the 12 drum channels |
| `crates/dtx-layout/src/lanes.rs` | Modify | delegate `channel_short_name`/`channel_from_short` to dtx-core |
| `crates/dtx-config/src/bindings.rs` | Create | `BindSource`, `MidiDeviceConfig`, `BindingsFile` (serde), `InputBindings` (runtime), defaults, load/save/migrations |
| `crates/dtx-config/src/lib.rs` | Modify | export bindings module; drop `key_assign` |
| `crates/dtx-config/src/key_assign.rs` | Delete | unused BocuD port |
| `crates/dtx-config/Cargo.toml` | Modify | add `dtx-core` dep |
| `crates/dtx-input/src/lib.rs` | Modify | re-export `KeyCode`; module doc update |
| `crates/dtx-input/src/midi.rs` | Modify | `MidiSource::poll` emits `MidiEvent`s; drop `to_lane_hit` |
| `crates/dtx-input/src/mapping.rs` | Delete | hardcoded GM subset (constants move into dtx-config defaults) |
| `crates/gameplay-drums/src/bindings.rs` | Create | `BindResolver` resource + load-on-enter system |
| `crates/gameplay-drums/src/input.rs` | Modify | keyboard capture reads `BindResolver` |
| `crates/gameplay-drums/src/lib.rs` | Modify | midi_consumer resolves via `BindResolver` + velocity threshold; register module |
| `crates/gameplay-drums/src/lane_map.rs` | Modify | delete `LaneMap` struct (keep `LANE_ORDER`, `lane_of`, `lane_channel`, `LaneId`, `LANE_COUNT`) |
| `crates/game-menu/src/config_key_assign.rs` | Delete | unreachable UI stub |
| `crates/game-menu/src/lib.rs` (or wherever the module is registered) | Modify | drop module + plugin registration |

---

### Task 1: `EChannel` short names in dtx-core

**Files:**
- Modify: `crates/dtx-core/src/channel.rs`

- [ ] **Step 1: Write the failing tests**

Append to the existing `#[cfg(test)] mod tests` in `channel.rs` (create the mod if absent — check the file end first):

```rust
#[test]
fn drum_short_names_round_trip() {
    use EChannel::*;
    for ch in [
        HiHatClose, Snare, BassDrum, HighTom, LowTom, Cymbal, FloorTom,
        HiHatOpen, RideCymbal, LeftCymbal, LeftPedal, LeftBassDrum,
    ] {
        let name = ch.short_name().expect("drum channel has short name");
        assert_eq!(EChannel::from_short_name(name), Some(ch));
    }
}

#[test]
fn short_name_values_match_layout_convention() {
    assert_eq!(EChannel::HiHatClose.short_name(), Some("HH"));
    assert_eq!(EChannel::HiHatOpen.short_name(), Some("HHO"));
    assert_eq!(EChannel::LeftBassDrum.short_name(), Some("LBD"));
}

#[test]
fn non_drum_channel_has_no_short_name() {
    assert_eq!(EChannel::BGM.short_name(), None);
    assert_eq!(EChannel::from_short_name("NOPE"), None);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dtx-core short_name`
Expected: FAIL — `no method named short_name`

- [ ] **Step 3: Implement**

Add inside `impl EChannel` (there is an existing `impl EChannel` block with `from_byte`). The names MUST match `dtx-layout/src/lanes.rs::channel_short_name` exactly (check `crates/dtx-layout/src/lanes.rs:31-53` — the 12 pairs are HH/SD/BD/HT/LT/CY/FT/HHO/RD/LC/LP/LBD):

```rust
/// Short display/config name for drum channels ("HH", "HHO", …).
/// None for non-drum channels. Matches dtx-layout lane ids.
pub const fn short_name(self) -> Option<&'static str> {
    Some(match self {
        Self::HiHatClose => "HH",
        Self::Snare => "SD",
        Self::BassDrum => "BD",
        Self::HighTom => "HT",
        Self::LowTom => "LT",
        Self::Cymbal => "CY",
        Self::FloorTom => "FT",
        Self::HiHatOpen => "HHO",
        Self::RideCymbal => "RD",
        Self::LeftCymbal => "LC",
        Self::LeftPedal => "LP",
        Self::LeftBassDrum => "LBD",
        _ => return None,
    })
}

/// Inverse of [`short_name`].
pub fn from_short_name(name: &str) -> Option<Self> {
    Some(match name {
        "HH" => Self::HiHatClose,
        "SD" => Self::Snare,
        "BD" => Self::BassDrum,
        "HT" => Self::HighTom,
        "LT" => Self::LowTom,
        "CY" => Self::Cymbal,
        "FT" => Self::FloorTom,
        "HHO" => Self::HiHatOpen,
        "RD" => Self::RideCymbal,
        "LC" => Self::LeftCymbal,
        "LP" => Self::LeftPedal,
        "LBD" => Self::LeftBassDrum,
        _ => return None,
    })
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p dtx-core`
Expected: PASS (all, including pre-existing)

- [ ] **Step 5: Delegate dtx-layout helpers (DRY)**

In `crates/dtx-layout/src/lanes.rs`, replace the bodies of `channel_short_name` and `channel_from_short` (around lines 31-53; keep signatures — callers exist at `lanes.rs:52,135` and elsewhere):

```rust
/// Short name for a drum channel (delegates to dtx-core).
pub fn channel_short_name(ch: EChannel) -> Option<&'static str> {
    ch.short_name()
}

/// Inverse lookup (delegates to dtx-core).
pub fn channel_from_short(name: &str) -> Option<EChannel> {
    EChannel::from_short_name(name)
}
```

Delete the now-unused match tables inside them (keep any doc comments).

- [ ] **Step 6: Run layout tests**

Run: `cargo test -p dtx-layout`
Expected: PASS

- [ ] **Step 7: Format + commit**

```bash
rustfmt --edition 2021 crates/dtx-core/src/channel.rs crates/dtx-layout/src/lanes.rs
git add crates/dtx-core/src/channel.rs crates/dtx-layout/src/lanes.rs
git commit -m "feat(dtx-core): EChannel drum short names; dtx-layout delegates"
```

---

### Task 2: `dtx-config/src/bindings.rs` — schema, defaults, I/O

**Files:**
- Create: `crates/dtx-config/src/bindings.rs`
- Modify: `crates/dtx-config/Cargo.toml` (add `dtx-core = { path = "../dtx-core" }` under `[dependencies]`)
- Modify: `crates/dtx-config/src/lib.rs` (add `pub mod bindings;` next to `pub mod drums;`)
- Modify: `crates/dtx-input/src/lib.rs` (add re-export)

Context for the engineer:
- `dtx-config` already depends on `dtx-input`, and `dtx-input`'s bevy dep enables the `serialize` feature (`crates/dtx-input/Cargo.toml:21`), so Bevy `KeyCode` serializes as its variant name string (`"KeyX"`, `"Space"`).
- Keyboard defaults are ported from `crates/gameplay-drums/src/lane_map.rs:76-97` (BocuD `tSetDefaultKeyAssignments`), re-keyed by channel.
- File pattern mirrors `crates/dtx-layout/src/file.rs` (`LATEST_VERSION`, `parse_with_migrations`, warn-and-default on corrupt) and `crates/dtx-config/src/lib.rs:287-328` (`default_path`, `load`, `save`).

- [ ] **Step 1: Add the KeyCode re-export to dtx-input**

In `crates/dtx-input/src/lib.rs` after `pub use events::{LaneHit, LaneHitKind, LaneId};`:

```rust
/// Re-export for config crates that serialize key bindings without a direct
/// bevy dependency.
pub use bevy::input::keyboard::KeyCode;
```

- [ ] **Step 2: Write the failing tests**

Create `crates/dtx-config/src/bindings.rs` with the test module first (types stubbed by the implementation step; write the whole file in step 4 — for TDD discipline run the test compile in between):

Tests to include (final content shown in step 4): defaults cover all 12 channels; no duplicate source across channels; round-trip serde; unknown channel key in TOML warn-skipped; corrupt file → defaults; version 0 migrates to 1; GM note 49 → LC; velocity threshold default 0.

- [ ] **Step 3: Run to verify fail**

Run: `cargo test -p dtx-config bindings`
Expected: FAIL to compile — module doesn't exist yet (add `pub mod bindings;` to `lib.rs` first so failure is about missing types, not missing module)

- [ ] **Step 4: Full implementation**

`crates/dtx-config/src/bindings.rs`:

```rust
//! Input bindings — `bindings.toml` schema + runtime types.
//!
//! Design: docs/superpowers/specs/2026-07-07-customize-surface-design.md §3.
//! One source (key or MIDI note) maps to exactly one channel; one channel may
//! have many sources. File schema keys channels by dtx-core short names.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use dtx_core::EChannel;
use dtx_input::KeyCode;
use serde::{Deserialize, Serialize};

/// Current bindings.toml schema version.
pub const BINDINGS_VERSION: u32 = 1;

/// The 12 bindable drum channels, in BocuD lane order.
pub const BINDABLE_CHANNELS: [EChannel; 12] = [
    EChannel::HiHatClose,
    EChannel::Snare,
    EChannel::BassDrum,
    EChannel::HighTom,
    EChannel::LowTom,
    EChannel::FloorTom,
    EChannel::Cymbal,
    EChannel::HiHatOpen,
    EChannel::RideCymbal,
    EChannel::LeftCymbal,
    EChannel::LeftPedal,
    EChannel::LeftBassDrum,
];

/// One input source bound to a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BindSource {
    /// Keyboard key (Bevy `KeyCode`, serialized as its variant name).
    Key(KeyCode),
    /// MIDI note number (device-agnostic in v1).
    Midi { note: u8 },
}

/// MIDI device options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MidiDeviceConfig {
    /// Substring filter for the input port name; None = first available.
    pub port: Option<String>,
    /// NoteOn velocities at or below this value are ignored.
    pub velocity_threshold: u8,
}

impl Default for MidiDeviceConfig {
    fn default() -> Self {
        Self { port: None, velocity_threshold: 0 }
    }
}

/// On-disk schema (`bindings.toml`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BindingsFile {
    /// Schema version for migrations.
    pub version: u32,
    /// MIDI device options.
    pub midi: MidiDeviceConfig,
    /// Channel short name → sources. BTreeMap for stable file output.
    pub map: BTreeMap<String, Vec<BindSource>>,
}

impl Default for BindingsFile {
    fn default() -> Self {
        InputBindings::default().to_file()
    }
}

/// Runtime bindings, channel-keyed.
#[derive(Debug, Clone, PartialEq)]
pub struct InputBindings {
    /// MIDI device options.
    pub midi: MidiDeviceConfig,
    /// Channel → sources. One source appears under at most one channel.
    pub map: HashMap<EChannel, Vec<BindSource>>,
}

impl Default for InputBindings {
    fn default() -> Self {
        use BindSource::{Key, Midi};
        use EChannel::*;
        use KeyCode as K;
        let mut map: HashMap<EChannel, Vec<BindSource>> = HashMap::new();
        // Keyboard: BocuD tSetDefaultKeyAssignments
        // (ported from gameplay-drums lane_map::default_drums).
        // MIDI: General MIDI percussion map, completed for toms/cymbals
        // (fixes old mapping.rs 49→HH; GM 49 = Crash 1 → LC).
        map.insert(HiHatClose, vec![Key(K::KeyX), Midi { note: 42 }]);
        map.insert(Snare, vec![Key(K::KeyC), Key(K::KeyD), Midi { note: 38 }, Midi { note: 40 }]);
        map.insert(BassDrum, vec![Key(K::Space), Key(K::Convert), Midi { note: 36 }, Midi { note: 35 }]);
        map.insert(HighTom, vec![Key(K::KeyV), Key(K::KeyF), Midi { note: 48 }, Midi { note: 50 }]);
        map.insert(LowTom, vec![Key(K::KeyB), Key(K::KeyG), Midi { note: 45 }, Midi { note: 47 }]);
        map.insert(FloorTom, vec![Key(K::KeyN), Key(K::KeyH), Midi { note: 43 }, Midi { note: 41 }]);
        map.insert(Cymbal, vec![Key(K::KeyM), Key(K::KeyJ), Midi { note: 57 }, Midi { note: 52 }]);
        map.insert(HiHatOpen, vec![Key(K::KeyS), Midi { note: 46 }]);
        map.insert(RideCymbal, vec![Key(K::Comma), Key(K::KeyK), Midi { note: 51 }, Midi { note: 59 }]);
        map.insert(LeftCymbal, vec![Key(K::KeyZ), Key(K::KeyA), Midi { note: 49 }, Midi { note: 55 }]);
        map.insert(LeftPedal, vec![Key(K::NonConvert), Midi { note: 44 }]);
        map.insert(LeftBassDrum, vec![Key(K::AltLeft)]);
        Self { midi: MidiDeviceConfig::default(), map }
    }
}

impl InputBindings {
    /// Channel for a keyboard key, if bound.
    pub fn channel_for_key(&self, key: KeyCode) -> Option<EChannel> {
        self.channel_for(BindSource::Key(key))
    }

    /// Channel for a MIDI note, if bound.
    pub fn channel_for_note(&self, note: u8) -> Option<EChannel> {
        self.channel_for(BindSource::Midi { note })
    }

    fn channel_for(&self, src: BindSource) -> Option<EChannel> {
        self.map
            .iter()
            .find(|(_, v)| v.contains(&src))
            .map(|(ch, _)| *ch)
    }

    /// Bind `src` to `channel`, removing it from any other channel first
    /// (steal semantics — UI confirms before calling).
    pub fn bind(&mut self, channel: EChannel, src: BindSource) {
        for v in self.map.values_mut() {
            v.retain(|s| *s != src);
        }
        self.map.entry(channel).or_default().push(src);
    }

    /// Serialize to the on-disk schema.
    pub fn to_file(&self) -> BindingsFile {
        let mut map = BTreeMap::new();
        for ch in BINDABLE_CHANNELS {
            let name = ch.short_name().expect("bindable channel has short name");
            let sources = self.map.get(&ch).cloned().unwrap_or_default();
            map.insert(name.to_string(), sources);
        }
        BindingsFile { version: BINDINGS_VERSION, midi: self.midi.clone(), map }
    }
}

impl BindingsFile {
    /// Resolve to runtime bindings. Unknown channel names are skipped with a
    /// warning; duplicate sources keep the first occurrence (BTreeMap order).
    pub fn resolve(&self) -> InputBindings {
        let mut map: HashMap<EChannel, Vec<BindSource>> = HashMap::new();
        let mut seen: Vec<BindSource> = Vec::new();
        for (name, sources) in &self.map {
            let Some(ch) = EChannel::from_short_name(name) else {
                eprintln!("dtx-config: bindings.toml unknown channel {name:?}; skipped");
                continue;
            };
            let entry = map.entry(ch).or_default();
            for src in sources {
                if seen.contains(src) {
                    eprintln!("dtx-config: bindings.toml duplicate source {src:?}; kept first");
                    continue;
                }
                seen.push(*src);
                entry.push(*src);
            }
        }
        InputBindings { midi: self.midi.clone(), map }
    }
}

/// Parse raw TOML, running the version migration chain (same policy as
/// dtx-layout `parse_with_migrations`).
pub fn parse_with_migrations(raw: &str) -> BindingsFile {
    let mut file: BindingsFile = match toml::from_str(raw) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("dtx-config: bindings parse failed: {e}; using defaults");
            return BindingsFile::default();
        }
    };
    if file.version > BINDINGS_VERSION {
        eprintln!(
            "dtx-config: bindings.toml version {} newer than supported {}; best-effort load",
            file.version, BINDINGS_VERSION
        );
        return file;
    }
    #[allow(clippy::single_match)]
    match file.version {
        0 => file.version = 1,
        _ => {}
    }
    file
}

/// `$XDG_CONFIG_HOME/dtxmaniars/bindings.toml` (same directory scheme as
/// config.toml, see `crate::default_path`).
pub fn default_bindings_path() -> PathBuf {
    let mut p = crate::default_path();
    p.set_file_name("bindings.toml");
    p
}

/// Load bindings; missing/corrupt file → defaults.
pub fn load_bindings(path: &Path) -> InputBindings {
    match std::fs::read_to_string(path) {
        Ok(raw) => parse_with_migrations(&raw).resolve(),
        Err(_) => InputBindings::default(),
    }
}

/// Save bindings. Creates parent dirs.
pub fn save_bindings(path: &Path, b: &InputBindings) -> Result<(), crate::ConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let s = toml::to_string_pretty(&b.to_file())?;
    std::fs::write(path, s)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_cover_all_bindable_channels() {
        let b = InputBindings::default();
        for ch in BINDABLE_CHANNELS {
            assert!(b.map.contains_key(&ch), "{ch:?} missing");
        }
    }

    #[test]
    fn defaults_have_no_duplicate_sources() {
        let b = InputBindings::default();
        let all: Vec<_> = b.map.values().flatten().collect();
        let mut dedup = all.clone();
        dedup.sort_by_key(|s| format!("{s:?}"));
        dedup.dedup();
        assert_eq!(all.len(), dedup.len());
    }

    #[test]
    fn gm_note_49_is_left_cymbal_not_hh() {
        let b = InputBindings::default();
        assert_eq!(b.channel_for_note(49), Some(EChannel::LeftCymbal));
        assert_eq!(b.channel_for_note(42), Some(EChannel::HiHatClose));
        assert_eq!(b.channel_for_note(46), Some(EChannel::HiHatOpen));
        assert_eq!(b.channel_for_note(48), Some(EChannel::HighTom)); // tom now mapped
    }

    #[test]
    fn key_lookup_matches_bocud_defaults() {
        let b = InputBindings::default();
        assert_eq!(b.channel_for_key(KeyCode::KeyX), Some(EChannel::HiHatClose));
        assert_eq!(b.channel_for_key(KeyCode::Space), Some(EChannel::BassDrum));
        assert_eq!(b.channel_for_key(KeyCode::AltLeft), Some(EChannel::LeftBassDrum));
        assert_eq!(b.channel_for_key(KeyCode::KeyQ), None);
    }

    #[test]
    fn round_trip_file_resolve() {
        let b = InputBindings::default();
        let s = toml::to_string_pretty(&b.to_file()).unwrap();
        let back = parse_with_migrations(&s).resolve();
        assert_eq!(back, b);
    }

    #[test]
    fn bind_steals_from_other_channel() {
        let mut b = InputBindings::default();
        // KeyX starts on HH; bind it to SD.
        b.bind(EChannel::Snare, BindSource::Key(KeyCode::KeyX));
        assert_eq!(b.channel_for_key(KeyCode::KeyX), Some(EChannel::Snare));
        // HH must no longer contain it.
        assert!(!b.map[&EChannel::HiHatClose].contains(&BindSource::Key(KeyCode::KeyX)));
    }

    #[test]
    fn unknown_channel_name_skipped() {
        let raw = r#"
version = 1
[midi]
velocity_threshold = 10
[map]
NOPE = [{ key = "KeyQ" }]
HH = [{ key = "KeyX" }]
"#;
        let b = parse_with_migrations(raw).resolve();
        assert_eq!(b.channel_for_key(KeyCode::KeyQ), None);
        assert_eq!(b.channel_for_key(KeyCode::KeyX), Some(EChannel::HiHatClose));
        assert_eq!(b.midi.velocity_threshold, 10);
    }

    #[test]
    fn corrupt_file_yields_defaults() {
        let f = parse_with_migrations("not = valid = [[toml");
        assert_eq!(f, BindingsFile::default());
    }

    #[test]
    fn version_zero_migrates_to_one() {
        let f = parse_with_migrations("version = 0");
        assert_eq!(f.version, 1);
    }

    #[test]
    fn duplicate_source_in_file_keeps_first() {
        let raw = r#"
version = 1
[map]
BD = [{ key = "Space" }]
SD = [{ key = "Space" }]
"#;
        let b = parse_with_migrations(raw).resolve();
        // BTreeMap order: BD before SD → BD wins.
        assert_eq!(b.channel_for_key(KeyCode::Space), Some(EChannel::BassDrum));
    }

    #[test]
    fn save_load_round_trip() {
        let tmp = std::env::temp_dir().join("dtxmaniars_bindings_test");
        let _ = std::fs::remove_dir_all(&tmp);
        let p = tmp.join("bindings.toml");
        let mut b = InputBindings::default();
        b.midi.velocity_threshold = 24;
        b.bind(EChannel::Snare, BindSource::Midi { note: 99 });
        save_bindings(&p, &b).unwrap();
        let back = load_bindings(&p);
        assert_eq!(back, b);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn missing_file_yields_defaults() {
        let b = load_bindings(Path::new("/nonexistent/dtxmaniars/bindings.toml"));
        assert_eq!(b, InputBindings::default());
    }

    #[test]
    fn default_bindings_path_filename() {
        assert_eq!(default_bindings_path().file_name().unwrap(), "bindings.toml");
    }
}
```

In `crates/dtx-config/src/lib.rs` add next to `pub mod drums;`:

```rust
pub mod bindings;
```

and next to the other `pub use` lines:

```rust
pub use bindings::{
    default_bindings_path, load_bindings, save_bindings, BindSource, BindingsFile,
    InputBindings, MidiDeviceConfig, BINDABLE_CHANNELS,
};
```

In `crates/dtx-config/Cargo.toml` under `[dependencies]`:

```toml
dtx-core = { path = "../dtx-core" }
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p dtx-config`
Expected: PASS. If `Convert`/`NonConvert` variant names differ in Bevy 0.19, check with `rg "Convert" crates/gameplay-drums/src/lane_map.rs` (they're used there today, so they exist).

- [ ] **Step 6: Format + commit**

```bash
rustfmt --edition 2021 crates/dtx-config/src/bindings.rs crates/dtx-config/src/lib.rs crates/dtx-input/src/lib.rs
git add crates/dtx-config crates/dtx-input/src/lib.rs
git commit -m "feat(dtx-config): InputBindings schema + bindings.toml I/O

Channel-keyed key/MIDI bindings with steal semantics, GM defaults
completed for toms/cymbals (note 49 now LC per GM, was HH)."
```

---

### Task 3: `MidiSource` emits raw events (mapping leaves dtx-input)

**Files:**
- Modify: `crates/dtx-input/src/midi.rs`
- Delete: `crates/dtx-input/src/mapping.rs`
- Modify: `crates/dtx-input/src/lib.rs` (drop `pub mod mapping;`, update module-map doc)

- [ ] **Step 1: Rewrite trait + VirtualSource**

In `midi.rs`:
- Remove `use crate::events::{LaneHit, LaneHitKind};` and `use crate::mapping::midi_note_to_drum_lane;`.
- Change the trait:

```rust
/// A source of MIDI events. Implementations may be real (via midir) or
/// virtual (for tests). Note→channel mapping is the consumer's job
/// (see dtx-config `InputBindings`).
pub trait MidiSource: Send {
    /// Drain pending events into `out`. Returns the number of events pushed.
    fn poll(&mut self, out: &mut Vec<MidiEvent>) -> usize;

    /// True if the source has data available without consuming it.
    fn has_events(&self) -> bool;
}
```

- `impl MidiSource for VirtualSource`:

```rust
impl MidiSource for VirtualSource {
    fn poll(&mut self, out: &mut Vec<MidiEvent>) -> usize {
        let before = out.len();
        while let Some(ev) = self.events.pop_front() {
            out.push(ev);
        }
        out.len() - before
    }

    fn has_events(&self) -> bool {
        !self.events.is_empty()
    }
}
```

- Delete `impl MidiEvent { pub fn to_lane_hit(...) }` entirely.

- [ ] **Step 2: Rewrite midi.rs tests**

Replace the test module (old tests referenced lanes + mapping):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_source_starts_empty() {
        let s = VirtualSource::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn poll_drains_all_events_verbatim() {
        let mut s = VirtualSource::new();
        s.note_on(36, 100, 500);
        s.note_off(36, 700);
        s.push(MidiEvent::ControlChange { controller: 4, value: 90, audio_ms: 800 });
        let mut out = Vec::new();
        let n = s.poll(&mut out);
        assert_eq!(n, 3);
        assert!(s.is_empty());
        assert_eq!(out[0], MidiEvent::NoteOn { note: 36, velocity: 100, audio_ms: 500 });
        assert_eq!(out[1], MidiEvent::NoteOff { note: 36, audio_ms: 700 });
    }

    #[test]
    fn has_events_reflects_queue() {
        let mut s = VirtualSource::new();
        assert!(!s.has_events());
        s.note_on(38, 90, 0);
        assert!(s.has_events());
    }
}
```

- [ ] **Step 3: Delete mapping.rs and its module registration**

```bash
git rm crates/dtx-input/src/mapping.rs
```

In `crates/dtx-input/src/lib.rs`: remove `pub mod mapping;` and the `- [mapping] — note/byte → LaneId helpers` doc line.

- [ ] **Step 4: Build the crate — expect downstream breakage listed**

Run: `cargo test -p dtx-input`
Expected: PASS for dtx-input itself.

Run: `cargo check -p gameplay-drums`
Expected: FAIL in `gameplay-drums/src/lib.rs` `midi_consumer` (`poll` type mismatch). That's Task 4. Do NOT commit yet if the workspace must stay green per-commit — instead proceed to Task 4 and commit both together.

---

### Task 4: gameplay-drums `BindResolver` + consumers

**Files:**
- Create: `crates/gameplay-drums/src/bindings.rs`
- Modify: `crates/gameplay-drums/src/input.rs`
- Modify: `crates/gameplay-drums/src/lib.rs` (module registration + midi_consumer)
- Modify: `crates/gameplay-drums/src/lane_map.rs` (delete `LaneMap` struct + its `KeyLaneMap` impl + its tests; KEEP `LaneId`, `LANE_COUNT`, `LANE_ORDER`, `lane_of`, `lane_channel` and their tests — judge/autoplay/hit_sound/drum_groups depend on them)

- [ ] **Step 1: Write `bindings.rs` with failing tests**

```rust
//! Runtime bind resolution: `InputBindings` → per-frame lookup tables.
//!
//! `BindResolver` flattens channel-keyed bindings into KeyCode→LaneId and
//! note→LaneId maps using the fixed BocuD lane order (`lane_map::lane_of`).
//! Rebuilt on Performance enter (config may have changed on disk).

use std::collections::HashMap;

use bevy::prelude::*;
use dtx_config::{BindSource, InputBindings};

use crate::lane_map::{lane_of, LaneId};

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<BindResolver>().add_systems(
        OnEnter(game_shell::AppState::Performance),
        reload_bindings,
    );
}

/// Flattened lookup tables derived from `InputBindings`.
#[derive(Resource, Debug, Clone)]
pub struct BindResolver {
    key_to_lane: HashMap<KeyCode, LaneId>,
    note_to_lane: HashMap<u8, LaneId>,
    /// NoteOn velocities at or below this are ignored.
    pub velocity_threshold: u8,
}

impl Default for BindResolver {
    fn default() -> Self {
        Self::from_bindings(&InputBindings::default())
    }
}

impl BindResolver {
    /// Build lookup tables from channel-keyed bindings.
    pub fn from_bindings(b: &InputBindings) -> Self {
        let mut key_to_lane = HashMap::new();
        let mut note_to_lane = HashMap::new();
        for (ch, sources) in &b.map {
            let Some(lane) = lane_of(*ch) else { continue };
            for src in sources {
                match src {
                    BindSource::Key(k) => {
                        key_to_lane.insert(*k, lane);
                    }
                    BindSource::Midi { note } => {
                        note_to_lane.insert(*note, lane);
                    }
                }
            }
        }
        Self {
            key_to_lane,
            note_to_lane,
            velocity_threshold: b.midi.velocity_threshold,
        }
    }

    /// Lane for a keyboard key, if bound.
    pub fn lane_for_key(&self, key: KeyCode) -> Option<LaneId> {
        self.key_to_lane.get(&key).copied()
    }

    /// Lane for a MIDI note, if bound.
    pub fn lane_for_note(&self, note: u8) -> Option<LaneId> {
        self.note_to_lane.get(&note).copied()
    }
}

/// Reload bindings.toml on entering Performance (mirrors config load style,
/// see lib.rs `load(&default_path())` call sites).
fn reload_bindings(mut resolver: ResMut<BindResolver>) {
    let b = dtx_config::load_bindings(&dtx_config::default_bindings_path());
    *resolver = BindResolver::from_bindings(&b);
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::EChannel;

    #[test]
    fn default_resolver_matches_bocud_keys() {
        let r = BindResolver::default();
        assert_eq!(r.lane_for_key(KeyCode::KeyX), Some(0)); // HH
        assert_eq!(r.lane_for_key(KeyCode::KeyC), Some(1)); // SD
        assert_eq!(r.lane_for_key(KeyCode::KeyD), Some(1)); // SD alt
        assert_eq!(r.lane_for_key(KeyCode::Space), Some(2)); // BD
        assert_eq!(r.lane_for_key(KeyCode::KeyS), Some(7)); // HHO
        assert_eq!(r.lane_for_key(KeyCode::AltLeft), Some(11)); // LBD
        assert_eq!(r.lane_for_key(KeyCode::KeyQ), None);
    }

    #[test]
    fn default_resolver_maps_gm_notes_to_lanes() {
        let r = BindResolver::default();
        assert_eq!(r.lane_for_note(36), Some(2)); // BD
        assert_eq!(r.lane_for_note(38), Some(1)); // SD
        assert_eq!(r.lane_for_note(42), Some(0)); // HH close
        assert_eq!(r.lane_for_note(46), Some(7)); // HH open
        assert_eq!(r.lane_for_note(49), Some(9)); // Crash 1 → LC (GM fix)
        assert_eq!(r.lane_for_note(51), Some(8)); // Ride
        assert_eq!(r.lane_for_note(48), Some(3)); // High tom — newly mapped
        assert_eq!(r.lane_for_note(43), Some(5)); // Floor tom — newly mapped
        assert_eq!(r.lane_for_note(20), None);
    }

    #[test]
    fn custom_binding_reroutes_lane() {
        let mut b = InputBindings::default();
        b.bind(EChannel::Snare, dtx_config::BindSource::Key(KeyCode::KeyX));
        let r = BindResolver::from_bindings(&b);
        assert_eq!(r.lane_for_key(KeyCode::KeyX), Some(1)); // now SD
    }

    #[test]
    fn threshold_copied_from_bindings() {
        let mut b = InputBindings::default();
        b.midi.velocity_threshold = 30;
        let r = BindResolver::from_bindings(&b);
        assert_eq!(r.velocity_threshold, 30);
    }
}
```

- [ ] **Step 2: Register module in lib.rs**

In `crates/gameplay-drums/src/lib.rs`: add `mod bindings;` next to the other module declarations (alphabetical position near `mod lane_map;` — check existing order), replace `.init_resource::<lane_map::LaneMap>()` (line ~105) with nothing (BindResolver init happens in its plugin), and add `bindings::plugin,` to the plugin tuple where `midi_consumer::plugin` is added (line ~189 area).

- [ ] **Step 3: Switch keyboard capture to resolver**

In `crates/gameplay-drums/src/input.rs`:
- Replace `use crate::lane_map::LaneMap;` with `use crate::bindings::BindResolver;`.
- In `capture_key_to_lane_input`, change the param `lane_map: Res<LaneMap>` → `resolver: Res<BindResolver>` and the lookup `lane_map.lane_for_key(*key)` → `resolver.lane_for_key(*key)`.
- Replace the first two tests (they construct `LaneMap`) with:

```rust
#[test]
fn resolver_drums_default_matches_bocud() {
    // BocuD default: X = HH, Space = BD.
    let r = crate::bindings::BindResolver::default();
    assert_eq!(r.lane_for_key(KeyCode::KeyX), Some(0));
    assert_eq!(r.lane_for_key(KeyCode::Space), Some(2));
}
```

(keep `input_audio_ms_is_compensated_for_capture_delay` unchanged; drop `guitar_mode_does_not_match_drums_keys` — gating is untestable at that level and the lane-map claim is gone).

- [ ] **Step 4: Switch midi_consumer to resolver + threshold**

In `crates/gameplay-drums/src/lib.rs` `mod midi_consumer`, replace `poll_midi`:

```rust
fn poll_midi(
    mut source: ResMut<VirtualSource>,
    resolver: Res<crate::bindings::BindResolver>,
    chart: Res<ActiveChart>,
    clock: Res<GameplayClock>,
    mut hits: MessageWriter<LaneHit>,
) {
    if source.is_empty() {
        return;
    }
    if chart.chart.chips.is_empty() {
        return;
    }
    if !clock.is_ready() {
        return;
    }
    let mut buf: Vec<dtx_input::midi::MidiEvent> = Vec::new();
    (*source).poll(&mut buf);
    for ev in buf {
        // NoteOff / CC ignored (HH pedal CC handling is v2).
        let dtx_input::midi::MidiEvent::NoteOn { note, velocity, audio_ms } = ev else {
            continue;
        };
        if velocity == 0 || velocity <= resolver.velocity_threshold {
            continue;
        }
        let Some(lane) = resolver.lane_for_note(note) else {
            continue;
        };
        hits.write(LaneHit {
            lane,
            audio_ms: if audio_ms != 0 { audio_ms } else { clock.current_ms },
        });
    }
}
```

Update the module's imports: `use dtx_input::midi::{MidiSource, VirtualSource};` stays (MidiSource for `.poll`).

- [ ] **Step 5: Delete `LaneMap`**

In `crates/gameplay-drums/src/lane_map.rs`:
- Delete: the `LaneMap` struct, its `Default` impl, its `impl dtx_input::keyboard::KeyLaneMap` block, `default_drums`, `lane_for_key`, and the `use std::collections::HashMap;` + `use bevy::prelude::KeyCode;` imports if now unused.
- Delete tests: `default_maps_bocud_drums`, `default_labels_match_lane_order`, `key_lane_map_trait_impl`.
- Keep: `LaneId`, `LANE_COUNT`, `LANE_ORDER`, `lane_of`, `lane_channel` and tests `lane_order_matches_bocud`, `lane_of_non_drum_is_none`, `lane_channel_round_trip`, `lane_channel_out_of_range`, `lane_count_is_twelve`.
- Update the module doc: keybinds now live in `dtx-config bindings` / `crate::bindings`.

- [ ] **Step 6: Build + test**

Run: `cargo test -p gameplay-drums`
Expected: PASS. Chase any remaining `LaneMap` references the compiler finds (grep says only input.rs + lib.rs init).

- [ ] **Step 7: Format + commit (with Task 3 changes)**

```bash
rustfmt --edition 2021 crates/dtx-input/src/midi.rs crates/dtx-input/src/lib.rs crates/gameplay-drums/src/bindings.rs crates/gameplay-drums/src/input.rs crates/gameplay-drums/src/lib.rs crates/gameplay-drums/src/lane_map.rs
git add crates/dtx-input crates/gameplay-drums
git commit -m "feat(input): gameplay reads InputBindings; MidiSource emits raw events

BindResolver flattens channel bindings to lane tables; keyboard capture
and midi consumer resolve through it. Velocity threshold applied to
NoteOn; NoteOff no longer emitted as a hit (was a latent bug). Deletes
hardcoded LaneMap keys and dtx-input mapping.rs."
```

---

### Task 5: Delete dead BocuD ports

**Files:**
- Delete: `crates/dtx-config/src/key_assign.rs`
- Modify: `crates/dtx-config/src/lib.rs` (drop `pub mod key_assign;` and `pub use key_assign::{KeyAssignPad, KeyAssignPart, KeyAssignTable, STKeyAssign};`; update the crate doc header lines that mention KeyAssign)
- Delete: `crates/game-menu/src/config_key_assign.rs`
- Modify: `crates/game-menu/src/lib.rs` (find with `rg -n "config_key_assign" crates/game-menu/src` — remove the `mod` declaration and any plugin registration; the stub's plugin only does `init_resource::<KeyAssignState>()`)

- [ ] **Step 1: Verify nothing else references them**

Run: `rg -n "key_assign|KeyAssign" crates app --include='*.rs' | grep -v "crates/dtx-config/src/key_assign.rs\|crates/game-menu/src/config_key_assign.rs"`
Expected: only the `lib.rs` registration lines you're about to remove. If anything else shows up, STOP and report.

- [ ] **Step 2: Delete + unregister**

```bash
git rm crates/dtx-config/src/key_assign.rs crates/game-menu/src/config_key_assign.rs
```

Apply the lib.rs edits above.

- [ ] **Step 3: Workspace check**

Run: `cargo check --workspace`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "refactor: delete unused BocuD key-assign port and UI stub

KeyAssignTable was never persisted or read by gameplay; the
CActConfigKeyAssign stub was unreachable. Superseded by
dtx-config::bindings."
```

---

### Task 6: Full verification

- [ ] **Step 1: Full workspace tests (includes headless schedule guard tests)**

Run: `cargo test --workspace`
Expected: PASS. The plugin-schedule guard tests must pass — green unit tests alone don't prove the FixedUpdate schedule builds.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean (repo convention — verify with `rg -n "clippy" Cargo.toml .github -r` if unsure whether -D warnings is enforced; match whatever CI does).

- [ ] **Step 3: Manual smoke (run the app)**

Run the desktop app, enter a song, verify: X/C/Space still hit HH/SD/BD; `$XDG_CONFIG_HOME/dtxmaniars/bindings.toml` does NOT get created (load-only; file appears only after UI saves — that's plan 3). Then hand-write a minimal `bindings.toml` swapping X to SD, re-enter a song, verify X now hits SD.

- [ ] **Step 4: Final commit if any fixups**

```bash
git add -A && git commit -m "test: bindings backend fixups"
```

---

## Self-review notes

- Spec §3 coverage: 3.1 types/rules/defaults → Tasks 1–2; 3.2 wiring/deletions → Tasks 3–5. Spec §4–5 (surface) deliberately out of scope — follow-up plans.
- `InputBindings` lives in dtx-config per spec; KeyCode via existing dtx-input dep (serialize feature confirmed at `crates/dtx-input/Cargo.toml:21`).
- Type names consistent across tasks: `BindSource`, `InputBindings`, `BindingsFile`, `BindResolver`, `lane_for_key`, `lane_for_note`.
- `LANE_ORDER` family retained — judge.rs/autoplay.rs/hit_sound.rs/drum_groups.rs depend on it (verified by grep).
