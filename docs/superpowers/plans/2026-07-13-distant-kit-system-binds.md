# Distant-Kit System Binds Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A drummer seated at an electronic kit can pause, quit, restart, and cancel a load without leaving the throne, by binding `Pause` / `Restart` to a spare pad note (or key) that no lane owns.

**Architecture:** `InputBindings` gains a second map — `system: HashMap<SystemVerb, Vec<BindSource>>` — parallel to the existing channel map; `BindResolver` flattens it into `note_to_system` / `key_to_system` tables, **skipping any source a lane already owns** (lanes win ties, enforced at the resolver, not just the UI). A resolved hit emits `SystemVerbHit` from the `DrumsSets::Input` set **before** the `gameplay_ready` gate — the same place `PadNavHit` is peeled off — so the verb fires during live play without ever travelling the `NavAction` path. `Pause` toggles `PauseState` (opening the overlay that already carries pad grammar + a Quit row, closing F1 and F3 together); `Restart` re-requests `AppState::SongLoading`. F4 (cancel a load from the kit) is independent: `NavContext::Loading` + a `NavAction` reader on `watch_cancel_key`.

**Tech Stack:** Rust, Bevy 0.19 (`Message` / `MessageReader` / `MessageWriter` / `add_message`), serde + toml, crates `dtx-input`, `gameplay-drums`, `game-menu`.

---

## Source of truth

- **Spec (authoritative):** `docs/superpowers/specs/2026-07-13-distant-kit-system-binds-design.md`
- **Research base:** `docs/notes/2026-07-13-distant-kit-research.md`
- **House rules:** `AGENTS.md` — no `unwrap()` under `crates/*` (`expect` in tests only), conventional commits, **no AI co-author trailers**.

## One deliberate deviation from the spec, stated up front

The spec's data model (`InputBindings.system` + a `BindingsFile` `[system]` table) predates the **profile registry split**: `bindings.toml` is now only a *legacy migration input*. The live persistence path is
`keyboard-profiles.toml` / `midi-profiles.toml` via `KeyboardProfile` / `MidiProfile`
(`gameplay-drums/src/editor/mod.rs:249` splits `LiveBindings` into the two profile drafts; `profile_bar_ui.rs` saves them; `bindings.rs:251` recomposes `LiveBindings` from them on every Performance enter).

Consequently, **`InputBindings.system` alone would not survive a single screen transition.** Task 3 therefore carries the system map through `KeyboardProfile.system` / `MidiProfile.system` and `split_bindings` / `compose_bindings`. Everything else follows the spec verbatim. The `BindingsFile` `[system]` table (Task 1) is still built exactly as specced — it is the migration source and keeps `save_bindings`/`load_bindings` round-tripping.

Task 3 is the only task not in the spec's or the brief's list. It is not optional.

---

## File Structure

### Created
_None._ Every change lands in an existing file.

### Modified

| File | Responsibility after this plan |
|---|---|
| `crates/dtx-input/src/bindings.rs` | Adds `SystemVerb`, `SYSTEM_VERBS`, `InputBindings.system`, `bind_system` / `system_sources`, the `BindingsFile` `[system]` table (no version bump), and the single-source-of-truth collision rule `lane_owner`. |
| `crates/dtx-input/src/lib.rs` | Re-exports `SystemVerb`, `SYSTEM_VERBS`, `lane_owner`. |
| `crates/dtx-input/src/profiles.rs` | `KeyboardProfile.system` / `MidiProfile.system` + serde; `split_bindings` partitions system binds so they persist to the profile registries. |
| `crates/gameplay-drums/src/bindings.rs` | `BindResolver.key_to_system` / `note_to_system` built in **both** `from_bindings` and `from_profiles`, skipping lane-owned sources with a `warn!`; `compose_bindings` recomposes the system map. |
| `crates/gameplay-drums/src/events.rs` | `SystemVerbHit { verb }` message. |
| `crates/gameplay-drums/src/lib.rs` | Registers `SystemVerbHit`; `consume_midi_events` emits it before the `gameplay_ready` gate, after the velocity threshold. |
| `crates/gameplay-drums/src/input.rs` | `keyboard_system_verbs` — keyboard-bound verbs on the same message, **not** gated on `PauseState::Running` (a paused game must be un-pausable). |
| `crates/gameplay-drums/src/pause.rs` | `system_verb_pause` (toggles both directions, sets `PracticePauseSurface::Overlay` before pausing) and `system_verb_restart` (`request_transition(SongLoading)`), both gated `in_state(Performance)` + `editor_closed`. |
| `crates/gameplay-drums/src/menu_nav.rs` | `NavContext::Loading` + the `AppState::SongLoading` arm in `active_context`. |
| `crates/game-menu/src/song_loading.rs` | `watch_cancel_key` cancels on `NavVerb::Back` (SD) as well as Esc. |
| `crates/gameplay-drums/src/editor/bindings_capture.rs` | `CaptureState::SystemKey` / `SystemMidi` (carrying an in-place `refused` lane), the pure `system_capture_step`, and `highlight_selected_system_row`. |
| `crates/gameplay-drums/src/editor/bindings_panel.rs` | The **System** card: a row per verb, segment-filtered chips, `×` remove, `+` capture; `system_segment_rows`, `last_system_source_index`. |
| `crates/gameplay-drums/src/editor/controls_panel.rs` | `ControlsRow` (lane rows then system rows), `SelectedSystem` cursor, generic `step_row`; the nav consumer walks both. |
| `crates/gameplay-drums/src/editor/capture_modal.rs` | Modal text for the two system-capture states, incl. the refusal caption. |
| `crates/gameplay-drums/src/editor/footer.rs` | Footer text for the two system-capture states. |
| `crates/gameplay-drums/src/editor/mod.rs` | `init_resource::<SelectedSystem>()`. |

**Untouched by contract:** `editor/keyboard_nav.rs::pad_excluded` and the `pad_exclusion_matches_controls_contract` test. Pads must not navigate the Controls tab. Do not touch them.

---

## Task 1 — `SystemVerb` + `InputBindings.system` + the `[system]` file table

**Files:** `crates/dtx-input/src/bindings.rs` (add types after `BINDABLE_CHANNELS` L32; extend `InputBindings` L74-80, its `Default` L82-173, `to_file` L226-239, `BindingsFile` L55-64, `resolve` L245-263), `crates/dtx-input/src/lib.rs` (L38-41).

**No version bump.** `BindingsFile` is `#[serde(default)]`, so an old `bindings.toml` with no `[system]` table loads with an empty map. That is the whole migration.

- [ ] Add the failing round-trip test to the `tests` module at the bottom of `crates/dtx-input/src/bindings.rs`:

```rust
    #[test]
    fn old_file_without_system_table_loads_empty_system_map() {
        let raw = r#"
version = 1
[midi]
velocity_threshold = 10
[map]
HH = [{ key = "KeyX" }]
"#;
        let b = parse_with_migrations(raw).resolve();
        assert!(b.system.is_empty(), "no [system] table → empty system map");
        assert_eq!(b.channel_for_key(KeyCode::KeyX), Some(EChannel::HiHatClose));
    }

    #[test]
    fn system_binds_round_trip_through_the_file() {
        let mut b = InputBindings::default();
        b.bind_system(SystemVerb::Pause, BindSource::Midi { note: 37 });
        b.bind_system(SystemVerb::Restart, BindSource::Key(KeyCode::F9));
        let s = toml::to_string_pretty(&b.to_file()).expect("bindings serialize");
        assert!(s.contains("[system]"), "{s}");
        let back = parse_with_migrations(&s).resolve();
        assert_eq!(back, b);
        assert_eq!(
            back.system_sources(SystemVerb::Pause),
            [BindSource::Midi { note: 37 }]
        );
    }

    #[test]
    fn system_verb_file_keys_are_stable() {
        assert_eq!(SystemVerb::Pause.key(), "pause");
        assert_eq!(SystemVerb::Restart.key(), "restart");
        assert_eq!(SystemVerb::from_key("pause"), Some(SystemVerb::Pause));
        assert_eq!(SystemVerb::from_key("nope"), None);
        assert_eq!(SYSTEM_VERBS.len(), 2);
    }
```

- [ ] Run `cargo test -p dtx-input --lib bindings::` — expect compile failure: `cannot find type SystemVerb`, `no method named bind_system`, `no field system`.

- [ ] Add the verb type immediately after the `BINDABLE_CHANNELS` const (L32) in `crates/dtx-input/src/bindings.rs`:

```rust
/// A non-lane action a key or pad can trigger.
///
/// System verbs are **not** DTX chart channels, so `EChannel` gains no pseudo-
/// variants; they live in a parallel map on `InputBindings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SystemVerb {
    /// Toggle the pause overlay during a performance.
    Pause,
    /// Restart the current song from the top.
    Restart,
}

/// Every bindable system verb, in Controls-tab row order.
pub const SYSTEM_VERBS: [SystemVerb; 2] = [SystemVerb::Pause, SystemVerb::Restart];

impl SystemVerb {
    /// Stable on-disk key (the TOML table key under `[system]`). Mirrors the
    /// channel short-name scheme: the file never depends on Rust variant names.
    pub fn key(self) -> &'static str {
        match self {
            SystemVerb::Pause => "pause",
            SystemVerb::Restart => "restart",
        }
    }

    /// Inverse of [`SystemVerb::key`]; unknown keys are skipped on load.
    pub fn from_key(key: &str) -> Option<Self> {
        SYSTEM_VERBS.into_iter().find(|verb| verb.key() == key)
    }

    /// Human label for the Controls-tab row.
    pub fn label(self) -> &'static str {
        match self {
            SystemVerb::Pause => "Pause",
            SystemVerb::Restart => "Restart",
        }
    }
}

/// The lane channel that already owns `src`, if any. A system verb may not
/// share an input with a lane: the same hit would both judge and fire the verb.
///
/// One-directional: lane binds are never refused — lanes win ties. The editor's
/// capture path and `BindResolver` are the only two callers, and they are the
/// only two places the rule is enforced.
pub fn lane_owner(bindings: &InputBindings, src: &BindSource) -> Option<EChannel> {
    BINDABLE_CHANNELS
        .into_iter()
        .find(|ch| bindings.map.get(ch).is_some_and(|v| v.contains(src)))
}
```

- [ ] Add the `system` field to `BindingsFile` (after `map`, L63 — TOML tables must follow scalars, and `system` sorts last):

```rust
    /// Channel short name → sources. BTreeMap for stable file output.
    pub map: BTreeMap<String, Vec<BindSource>>,
    /// System-verb key (`SystemVerb::key`) → sources. Empty by default; an
    /// older file with no `[system]` table loads clean (container `serde(default)`).
    pub system: BTreeMap<String, Vec<BindSource>>,
```

- [ ] Add the `system` field to `InputBindings` (after `map`, L79):

```rust
    /// System verb → sources. Empty by default: Escape keeps working, and note
    /// maps vary by brand, so we never guess a pad on the user's behalf.
    pub system: HashMap<SystemVerb, Vec<BindSource>>,
```

- [ ] In `impl Default for InputBindings` (L168-172), replace the trailing struct literal with:

```rust
        Self {
            midi: MidiDeviceConfig::default(),
            map,
            system: HashMap::new(),
        }
```

- [ ] Add the two accessors to `impl InputBindings`, right before `to_file` (L225):

```rust
    /// Sources bound to `verb` (empty when unbound).
    pub fn system_sources(&self, verb: SystemVerb) -> &[BindSource] {
        self.system.get(&verb).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Bind `src` to `verb`. Never steals: one source may drive several verbs,
    /// and a lane-owned source is refused by the caller (`lane_owner`), not here.
    pub fn bind_system(&mut self, verb: SystemVerb, src: BindSource) {
        let entry = self.system.entry(verb).or_default();
        if !entry.contains(&src) {
            entry.push(src);
        }
    }
```

- [ ] In `to_file` (L226-239), replace the returned `BindingsFile` literal with:

```rust
        BindingsFile {
            version: BINDINGS_VERSION,
            midi: self.midi.clone(),
            map,
            system: SYSTEM_VERBS
                .into_iter()
                .filter_map(|verb| {
                    self.system
                        .get(&verb)
                        .filter(|sources| !sources.is_empty())
                        .map(|sources| (verb.key().to_owned(), sources.clone()))
                })
                .collect(),
        }
```

- [ ] In `BindingsFile::resolve` (L245-263), build the system map and return it. Replace the final `InputBindings { .. }` literal (L259-262) with:

```rust
        let mut system: HashMap<SystemVerb, Vec<BindSource>> = HashMap::new();
        for (name, sources) in &self.system {
            let Some(verb) = SystemVerb::from_key(name) else {
                eprintln!("dtx-input: bindings.toml unknown system verb {name:?}; skipped");
                continue;
            };
            let entry = system.entry(verb).or_default();
            for src in sources {
                if !entry.contains(src) {
                    entry.push(*src);
                }
            }
        }
        InputBindings {
            midi: self.midi.clone(),
            map,
            system,
        }
```

- [ ] Re-export the new items in `crates/dtx-input/src/lib.rs` — replace the `pub use bindings::{...}` block (L38-41) with:

```rust
pub use bindings::{
    default_bindings_path, lane_owner, load_bindings, save_bindings, BindSource, BindingsFile,
    InputBindings, MidiDeviceConfig, SystemVerb, BINDABLE_CHANNELS, SYSTEM_VERBS,
};
```

- [ ] Run `cargo test -p dtx-input --lib` — expect all tests to pass, including the three new ones. (`toml` emits `[system]` after `[map]` because both are tables and `system` sorts last; the `system_binds_round_trip_through_the_file` assertion on `s.contains("[system]")` proves it.)

- [ ] Commit:

```sh
git add crates/dtx-input/src/bindings.rs crates/dtx-input/src/lib.rs
git commit -m "feat(input): add SystemVerb and a system bind map to InputBindings"
```

---

## Task 2 — `lane_owner` tests (the collision rule)

**Files:** `crates/dtx-input/src/bindings.rs` (tests module, bottom of file).

The function itself shipped in Task 1 (it sits beside `SystemVerb` because both are the vocabulary of the rule). This task pins its behavior.

- [ ] Add to the `tests` module in `crates/dtx-input/src/bindings.rs`:

```rust
    #[test]
    fn lane_owner_names_the_channel_holding_the_source() {
        let b = InputBindings::default();
        assert_eq!(
            lane_owner(&b, &BindSource::Midi { note: 38 }),
            Some(EChannel::Snare)
        );
        assert_eq!(
            lane_owner(&b, &BindSource::Key(KeyCode::Space)),
            Some(EChannel::BassDrum)
        );
    }

    #[test]
    fn lane_owner_is_none_for_a_free_source() {
        let b = InputBindings::default();
        // Zone notes a 12-channel chart cannot address: xstick 37, ride bell 53.
        assert_eq!(lane_owner(&b, &BindSource::Midi { note: 37 }), None);
        assert_eq!(lane_owner(&b, &BindSource::Midi { note: 53 }), None);
        assert_eq!(lane_owner(&b, &BindSource::Key(KeyCode::F9)), None);
    }

    #[test]
    fn lane_owner_ignores_system_binds_lanes_win_ties() {
        // A source bound ONLY to a verb has no lane owner — the rule is
        // one-directional: it never refuses a lane bind.
        let mut b = InputBindings::default();
        b.bind_system(SystemVerb::Pause, BindSource::Midi { note: 37 });
        assert_eq!(lane_owner(&b, &BindSource::Midi { note: 37 }), None);
        // ...and once a lane takes it, the lane is reported.
        b.bind_shared(EChannel::Snare, BindSource::Midi { note: 37 });
        assert_eq!(
            lane_owner(&b, &BindSource::Midi { note: 37 }),
            Some(EChannel::Snare)
        );
    }

    #[test]
    fn lane_owner_returns_the_first_owner_in_lane_order() {
        let mut b = InputBindings::default();
        b.bind_shared(EChannel::Snare, BindSource::Midi { note: 42 }); // 42 = HH default
        // HiHatClose precedes Snare in BINDABLE_CHANNELS.
        assert_eq!(
            lane_owner(&b, &BindSource::Midi { note: 42 }),
            Some(EChannel::HiHatClose)
        );
    }
```

- [ ] Run `cargo test -p dtx-input --lib lane_owner` — expect 4 passing tests.

- [ ] Commit:

```sh
git add crates/dtx-input/src/bindings.rs
git commit -m "test(input): pin the lane_owner collision rule"
```

---

## Task 3 — Profiles carry system binds (persistence)

**Files:** `crates/dtx-input/src/profiles.rs` (`KeyboardProfile` L73-122, `MidiProfile` L124-200, helpers L202-233, `split_bindings` L310-330, tests).

Without this, a system bind made in the Controls tab is destroyed by the next `reload_profiles` (`gameplay-drums/src/bindings.rs:251`) and never reaches disk. See "One deliberate deviation" above.

- [ ] Add the failing tests to the `tests` module in `crates/dtx-input/src/profiles.rs`:

```rust
    #[test]
    fn keyboard_profile_round_trips_system_binds() {
        let mut profile = KeyboardProfile::default();
        profile.add_system_key(SystemVerb::Pause, KeyCode::F9);
        let raw = toml::to_string_pretty(&profile).expect("profile serializes");
        let back: KeyboardProfile = toml::from_str(&raw).expect("profile parses");
        assert_eq!(back.system[&SystemVerb::Pause], vec![KeyCode::F9]);
        assert_eq!(back.map, profile.map, "channel map survives");
    }

    #[test]
    fn midi_profile_round_trips_system_binds() {
        let mut profile = MidiProfile::default();
        profile.bind_system_note(SystemVerb::Pause, 37);
        let raw = toml::to_string_pretty(&profile).expect("profile serializes");
        let back: MidiProfile = toml::from_str(&raw).expect("profile parses");
        assert_eq!(back.system[&SystemVerb::Pause], vec![37]);
        assert_eq!(back.map, profile.map, "channel map survives");
    }

    #[test]
    fn old_profile_without_system_table_loads_empty() {
        let profile: KeyboardProfile =
            toml::from_str("HH = [\"KeyX\"]").expect("legacy keyboard profile parses");
        assert!(profile.system.is_empty());
        assert_eq!(profile.map[&EChannel::HiHatClose], vec![KeyCode::KeyX]);
    }

    #[test]
    fn split_bindings_partitions_system_binds_by_device() {
        use crate::{BindSource, InputBindings};
        let mut b = InputBindings::default();
        b.bind_system(SystemVerb::Pause, BindSource::Midi { note: 37 });
        b.bind_system(SystemVerb::Restart, BindSource::Key(KeyCode::F9));
        let (keyboard, midi) = split_bindings(&b);
        assert_eq!(midi.system[&SystemVerb::Pause], vec![37]);
        assert_eq!(keyboard.system[&SystemVerb::Restart], vec![KeyCode::F9]);
        assert!(keyboard.system.get(&SystemVerb::Pause).is_none());
        assert!(midi.system.get(&SystemVerb::Restart).is_none());
    }
```

- [ ] Run `cargo test -p dtx-input --lib profiles::` — expect compile failure: `no field system on KeyboardProfile`, `no method add_system_key`.

- [ ] Import the verb types. In `crates/dtx-input/src/profiles.rs`, replace the `use crate::bindings::{...}` line (L15) with:

```rust
use crate::bindings::{
    BindSource, BindingsFile, InputBindings, SystemVerb, BINDABLE_CHANNELS, SYSTEM_VERBS,
};
```

- [ ] Add the `system` field + mutator to `KeyboardProfile` (replace L73-102):

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct KeyboardProfile {
    pub map: HashMap<EChannel, Vec<KeyCode>>,
    /// System verbs bound to keys. Serialized under the profile's `system`
    /// table; absent in older files (`old_profile_without_system_table_loads_empty`).
    pub system: HashMap<SystemVerb, Vec<KeyCode>>,
}

impl Default for KeyboardProfile {
    fn default() -> Self {
        split_default_bindings().0
    }
}

impl KeyboardProfile {
    pub fn add_key(&mut self, channel: EChannel, key: KeyCode) {
        let keys = self.map.entry(channel).or_default();
        if !keys.contains(&key) {
            keys.push(key);
        }
    }

    pub fn key_owners(&self, key: KeyCode) -> Vec<EChannel> {
        BINDABLE_CHANNELS
            .into_iter()
            .filter(|channel| {
                self.map
                    .get(channel)
                    .is_some_and(|keys| keys.contains(&key))
            })
            .collect()
    }

    /// Bind `key` to `verb`. Never steals from a lane — the caller refuses a
    /// lane-owned key up front (`bindings::lane_owner`).
    pub fn add_system_key(&mut self, verb: SystemVerb, key: KeyCode) {
        let keys = self.system.entry(verb).or_default();
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
}
```

- [ ] Replace `KeyboardProfile`'s `Serialize` / `Deserialize` impls (L104-122) with ones that carry the system table. Channel arrays are emitted **before** the `system` table (TOML forbids a value after a table at the same level; `system` is lowercase so it also sorts last under `BTreeMap`):

```rust
impl Serialize for KeyboardProfile {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;
        let channels = channel_map(&self.map);
        let system = verb_map(&self.system);
        let mut map =
            serializer.serialize_map(Some(channels.len() + usize::from(!system.is_empty())))?;
        for (name, keys) in &channels {
            map.serialize_entry(name, keys)?;
        }
        if !system.is_empty() {
            map.serialize_entry("system", &system)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for KeyboardProfile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        /// A profile entry is either a channel's key array or the `system` table.
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Entry {
            Keys(Vec<KeyCode>),
            System(BTreeMap<String, Vec<KeyCode>>),
        }
        let raw = BTreeMap::<String, Entry>::deserialize(deserializer)?;
        let mut channels: BTreeMap<String, Vec<KeyCode>> = BTreeMap::new();
        let mut system: BTreeMap<String, Vec<KeyCode>> = BTreeMap::new();
        for (name, entry) in raw {
            match entry {
                Entry::Keys(keys) => {
                    channels.insert(name, keys);
                }
                Entry::System(table) if name == "system" => system = table,
                Entry::System(_) => {} // unknown nested table: skipped
            }
        }
        Ok(Self {
            map: parse_channel_map(channels),
            system: parse_verb_map(system),
        })
    }
}
```

- [ ] Add the `system` field + mutator to `MidiProfile` (replace L124-164):

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct MidiProfile {
    pub port: Option<String>,
    pub velocity_threshold: u8,
    pub map: HashMap<EChannel, Vec<u8>>,
    /// System verbs bound to MIDI notes. A spare zone note (xstick 37, ride
    /// bell 53, HH edge 22/26) costs no gameplay pad — see the research note.
    pub system: HashMap<SystemVerb, Vec<u8>>,
}

impl Default for MidiProfile {
    fn default() -> Self {
        split_default_bindings().1
    }
}

impl MidiProfile {
    pub fn note_owner(&self, note: u8) -> Option<EChannel> {
        BINDABLE_CHANNELS.into_iter().find(|channel| {
            self.map
                .get(channel)
                .is_some_and(|notes| notes.contains(&note))
        })
    }

    /// Caller confirms before this replaces another channel's note binding.
    pub fn bind_note(&mut self, channel: EChannel, note: u8) {
        for notes in self.map.values_mut() {
            notes.retain(|bound| *bound != note);
        }
        let notes = self.map.entry(channel).or_default();
        if !notes.contains(&note) {
            notes.push(note);
        }
    }

    /// Append `note` to `channel` without removing it from other channels.
    pub fn bind_note_shared(&mut self, channel: EChannel, note: u8) {
        let notes = self.map.entry(channel).or_default();
        if !notes.contains(&note) {
            notes.push(note);
        }
    }

    /// Bind `note` to `verb`. Never steals from a lane — the caller refuses a
    /// lane-owned note up front (`bindings::lane_owner`).
    pub fn bind_system_note(&mut self, verb: SystemVerb, note: u8) {
        let notes = self.system.entry(verb).or_default();
        if !notes.contains(&note) {
            notes.push(note);
        }
    }
}
```

- [ ] Extend `MidiProfileDto` (L166-172) and both MIDI serde impls. Replace L166-200 with:

```rust
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct MidiProfileDto {
    port: Option<String>,
    velocity_threshold: u8,
    map: BTreeMap<String, Vec<u8>>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    system: BTreeMap<String, Vec<u8>>,
}

impl Serialize for MidiProfile {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        MidiProfileDto {
            port: self.port.clone(),
            velocity_threshold: self.velocity_threshold,
            map: channel_map(&self.map),
            system: verb_map(&self.system),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for MidiProfile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let dto = MidiProfileDto::deserialize(deserializer)?;
        Ok(Self {
            port: dto.port.filter(|port| !port.is_empty()),
            velocity_threshold: dto.velocity_threshold,
            map: parse_channel_map(dto.map),
            system: parse_verb_map(dto.system),
        })
    }
}
```

- [ ] Add the two verb-map helpers next to `channel_map` / `parse_channel_map` (after L233):

```rust
/// Serialize a verb map with stable, brand-independent keys (`SystemVerb::key`).
fn verb_map<T: Clone>(map: &HashMap<SystemVerb, Vec<T>>) -> BTreeMap<String, Vec<T>> {
    SYSTEM_VERBS
        .into_iter()
        .filter_map(|verb| {
            map.get(&verb)
                .filter(|values| !values.is_empty())
                .map(|values| (verb.key().to_owned(), values.clone()))
        })
        .collect()
}

/// Deserialize a verb map, dropping unknown verb keys and deduping within each
/// verb (a key/note bound twice to one verb must fire it once).
fn parse_verb_map<T: PartialEq>(map: BTreeMap<String, Vec<T>>) -> HashMap<SystemVerb, Vec<T>> {
    map.into_iter()
        .filter_map(|(name, values)| {
            SystemVerb::from_key(&name).map(|verb| {
                let mut unique = Vec::with_capacity(values.len());
                for value in values {
                    if !unique.contains(&value) {
                        unique.push(value);
                    }
                }
                (verb, unique)
            })
        })
        .collect()
}
```

- [ ] Partition the system map in `split_bindings` (replace L310-330):

```rust
pub fn split_bindings(bindings: &InputBindings) -> (KeyboardProfile, MidiProfile) {
    let mut keyboard = KeyboardProfile {
        map: HashMap::new(),
        system: HashMap::new(),
    };
    let mut midi = MidiProfile {
        port: bindings.midi.port.clone(),
        velocity_threshold: bindings.midi.velocity_threshold,
        map: HashMap::new(),
        system: HashMap::new(),
    };

    for (channel, sources) in &bindings.map {
        for source in sources {
            match source {
                BindSource::Key(key) => keyboard.add_key(*channel, *key),
                BindSource::Midi { note } => midi.bind_note_shared(*channel, *note),
            }
        }
    }
    for (verb, sources) in &bindings.system {
        for source in sources {
            match source {
                BindSource::Key(key) => keyboard.add_system_key(*verb, *key),
                BindSource::Midi { note } => midi.bind_system_note(*verb, *note),
            }
        }
    }

    (keyboard, midi)
}
```

- [ ] Fix the three existing test struct literals in `crates/dtx-input/src/profiles.rs` that now miss a field — add `system: HashMap::new(),` to each:
  - `keyboard_registry_round_trips_spec_shape` (~L733): the `KeyboardProfile { map: ... }` literal.
  - `midi_registry_round_trips_spec_shape` (~L758): the `MidiProfile { port, velocity_threshold, map }` literal.
  - `revert_is_registry_noop` (~L901): the `KeyboardProfile { map: ... }` literal.

- [ ] Run `cargo test -p dtx-input --lib` — expect every test to pass, including the four new ones.

- [ ] Commit:

```sh
git add crates/dtx-input/src/profiles.rs
git commit -m "feat(input): persist system binds through keyboard and MIDI profiles"
```

---

## Task 4 — `BindResolver` system tables (collision skipped at the resolver)

**Files:** `crates/gameplay-drums/src/bindings.rs` (`BindResolver` L48-139, `compose_bindings` L204-225, tests).

A hand-edited file cannot produce a note that both judges and pauses: the resolver drops it. The footgun is closed here, not merely in the UI.

- [ ] Add the failing tests to the `tests` module in `crates/gameplay-drums/src/bindings.rs`:

```rust
    #[test]
    fn note_bound_to_a_lane_and_a_verb_resolves_to_the_lane_only() {
        use dtx_input::SystemVerb;
        let mut b = InputBindings::default();
        // 38 is the Snare default: a hand-edited file binding it to Pause too.
        b.bind_system(SystemVerb::Pause, dtx_input::BindSource::Midi { note: 38 });
        let r = BindResolver::from_bindings(&b);
        assert_eq!(r.lane_for_note(38), Some(1), "the lane still judges");
        assert_eq!(
            r.system_for_note(38).count(),
            0,
            "the colliding system source is skipped"
        );
    }

    #[test]
    fn free_note_resolves_to_the_verb_and_no_lane() {
        use dtx_input::SystemVerb;
        let mut b = InputBindings::default();
        b.bind_system(SystemVerb::Pause, dtx_input::BindSource::Midi { note: 37 });
        let r = BindResolver::from_bindings(&b);
        assert_eq!(r.lane_for_note(37), None);
        assert_eq!(
            r.system_for_note(37).collect::<Vec<_>>(),
            vec![SystemVerb::Pause]
        );
    }

    #[test]
    fn free_key_resolves_to_the_verb_and_no_lane() {
        use dtx_input::SystemVerb;
        let mut b = InputBindings::default();
        b.bind_system(SystemVerb::Restart, dtx_input::BindSource::Key(KeyCode::F9));
        let r = BindResolver::from_bindings(&b);
        assert_eq!(r.lane_for_key(KeyCode::F9), None);
        assert_eq!(
            r.system_for_key(KeyCode::F9).collect::<Vec<_>>(),
            vec![SystemVerb::Restart]
        );
    }

    #[test]
    fn from_profiles_builds_system_tables_and_skips_lane_collisions() {
        use dtx_input::SystemVerb;
        let mut keyboard = KeyboardProfile::default();
        let mut midi = MidiProfile::default();
        keyboard.add_system_key(SystemVerb::Restart, KeyCode::F9);
        midi.bind_system_note(SystemVerb::Pause, 37); // free zone note
        midi.bind_system_note(SystemVerb::Pause, 38); // Snare's note: refused
        let r = BindResolver::from_profiles(&keyboard, &midi);
        assert_eq!(
            r.system_for_note(37).collect::<Vec<_>>(),
            vec![SystemVerb::Pause]
        );
        assert_eq!(r.system_for_note(38).count(), 0, "lane wins the tie");
        assert_eq!(r.lane_for_note(38), Some(1));
        assert_eq!(
            r.system_for_key(KeyCode::F9).collect::<Vec<_>>(),
            vec![SystemVerb::Restart]
        );
    }

    #[test]
    fn compose_bindings_recomposes_the_system_map() {
        use dtx_input::SystemVerb;
        let mut keyboard = KeyboardProfile::default();
        let mut midi = MidiProfile::default();
        keyboard.add_system_key(SystemVerb::Restart, KeyCode::F9);
        midi.bind_system_note(SystemVerb::Pause, 37);
        let b = compose_bindings(&keyboard, &midi);
        assert_eq!(
            b.system_sources(SystemVerb::Pause),
            [dtx_input::BindSource::Midi { note: 37 }]
        );
        assert_eq!(
            b.system_sources(SystemVerb::Restart),
            [dtx_input::BindSource::Key(KeyCode::F9)]
        );
    }
```

- [ ] Run `cargo test -p gameplay-drums --lib bindings::` — expect compile failure: `no method named system_for_note`.

- [ ] Import the verb types. In `crates/gameplay-drums/src/bindings.rs`, replace the `use dtx_input::{...}` line (L15) with:

```rust
use dtx_input::{lane_owner, BindSource, InputBindings, SystemVerb, BINDABLE_CHANNELS, SYSTEM_VERBS};
```

- [ ] Add the two tables to `BindResolver` (replace L48-54):

```rust
/// Flattened lookup tables derived from `InputBindings`.
#[derive(Resource, Debug, Clone)]
pub struct BindResolver {
    key_to_lanes: HashMap<KeyCode, Vec<LaneId>>,
    note_to_lanes: HashMap<u8, Vec<LaneId>>,
    /// System verbs a key fires. A source a lane owns never reaches this table.
    key_to_system: HashMap<KeyCode, Vec<SystemVerb>>,
    /// System verbs a MIDI note fires. A source a lane owns never reaches this table.
    note_to_system: HashMap<u8, Vec<SystemVerb>>,
    /// NoteOn velocities at or below this are ignored.
    pub velocity_threshold: u8,
}
```

- [ ] Build the tables in `from_profiles` (replace L66-83):

```rust
    pub fn from_profiles(keyboard: &KeyboardProfile, midi: &MidiProfile) -> Self {
        let mut key_to_lanes = HashMap::new();
        let mut note_to_lanes = HashMap::new();
        for ch in BINDABLE_CHANNELS {
            let Some(lane) = lane_of(ch) else { continue };
            for key in keyboard.map.get(&ch).into_iter().flatten() {
                key_to_lanes.entry(*key).or_insert_with(Vec::new).push(lane);
            }
            for note in midi.map.get(&ch).into_iter().flatten() {
                note_to_lanes.entry(*note).or_insert_with(Vec::new).push(lane);
            }
        }
        let mut key_to_system: HashMap<KeyCode, Vec<SystemVerb>> = HashMap::new();
        let mut note_to_system: HashMap<u8, Vec<SystemVerb>> = HashMap::new();
        for verb in SYSTEM_VERBS {
            for key in keyboard.system.get(&verb).into_iter().flatten() {
                match keyboard.key_owners(*key).first() {
                    Some(owner) => warn!(
                        "system bind {verb:?} ignored: key {key:?} already drives lane {owner:?}"
                    ),
                    None => key_to_system.entry(*key).or_default().push(verb),
                }
            }
            for note in midi.system.get(&verb).into_iter().flatten() {
                match midi.note_owner(*note) {
                    Some(owner) => warn!(
                        "system bind {verb:?} ignored: note {note} already drives lane {owner:?}"
                    ),
                    None => note_to_system.entry(*note).or_default().push(verb),
                }
            }
        }
        Self {
            key_to_lanes,
            note_to_lanes,
            key_to_system,
            note_to_system,
            velocity_threshold: midi.velocity_threshold,
        }
    }
```

- [ ] Build the tables in `from_bindings` (replace L86-110):

```rust
    pub fn from_bindings(b: &InputBindings) -> Self {
        let mut key_to_lanes = HashMap::new();
        let mut note_to_lanes = HashMap::new();
        for ch in BINDABLE_CHANNELS {
            let Some(lane) = lane_of(ch) else { continue };
            let Some(sources) = b.map.get(&ch) else {
                continue;
            };
            for src in sources {
                match src {
                    BindSource::Key(k) => {
                        key_to_lanes.entry(*k).or_insert_with(Vec::new).push(lane);
                    }
                    BindSource::Midi { note } => {
                        note_to_lanes.entry(*note).or_insert_with(Vec::new).push(lane);
                    }
                }
            }
        }
        let mut key_to_system: HashMap<KeyCode, Vec<SystemVerb>> = HashMap::new();
        let mut note_to_system: HashMap<u8, Vec<SystemVerb>> = HashMap::new();
        for verb in SYSTEM_VERBS {
            for src in b.system.get(&verb).into_iter().flatten() {
                // Lanes win ties. A hand-edited bindings.toml cannot make one
                // note both judge and pause: the colliding source is dropped here.
                if let Some(owner) = lane_owner(b, src) {
                    warn!("system bind {verb:?} ignored: {src:?} already drives lane {owner:?}");
                    continue;
                }
                match src {
                    BindSource::Key(k) => key_to_system.entry(*k).or_default().push(verb),
                    BindSource::Midi { note } => {
                        note_to_system.entry(*note).or_default().push(verb)
                    }
                }
            }
        }
        Self {
            key_to_lanes,
            note_to_lanes,
            key_to_system,
            note_to_system,
            velocity_threshold: b.midi.velocity_threshold,
        }
    }
```

- [ ] Add the two accessors after `lanes_for_note` (L138, inside `impl BindResolver`):

```rust
    /// System verbs a MIDI note fires (empty unless bound and lane-free).
    pub fn system_for_note(&self, note: u8) -> impl Iterator<Item = SystemVerb> + '_ {
        self.note_to_system
            .get(&note)
            .into_iter()
            .flat_map(|verbs| verbs.iter().copied())
    }

    /// System verbs a keyboard key fires (empty unless bound and lane-free).
    pub fn system_for_key(&self, key: KeyCode) -> impl Iterator<Item = SystemVerb> + '_ {
        self.key_to_system
            .get(&key)
            .into_iter()
            .flat_map(|verbs| verbs.iter().copied())
    }
```

- [ ] Recompose the system map in `compose_bindings` — replace its body (L204-225) with:

```rust
pub(crate) fn compose_bindings(keyboard: &KeyboardProfile, midi: &MidiProfile) -> InputBindings {
    let mut bindings = InputBindings {
        midi: dtx_input::MidiDeviceConfig {
            port: midi.port.clone(),
            velocity_threshold: midi.velocity_threshold,
        },
        map: HashMap::new(),
        system: HashMap::new(),
    };
    for ch in BINDABLE_CHANNELS {
        let mut sources = Vec::new();
        for key in keyboard.map.get(&ch).into_iter().flatten() {
            sources.push(BindSource::Key(*key));
        }
        for note in midi.map.get(&ch).into_iter().flatten() {
            sources.push(BindSource::Midi { note: *note });
        }
        if !sources.is_empty() {
            bindings.map.insert(ch, sources);
        }
    }
    for verb in SYSTEM_VERBS {
        let mut sources = Vec::new();
        for key in keyboard.system.get(&verb).into_iter().flatten() {
            sources.push(BindSource::Key(*key));
        }
        for note in midi.system.get(&verb).into_iter().flatten() {
            sources.push(BindSource::Midi { note: *note });
        }
        if !sources.is_empty() {
            bindings.system.insert(verb, sources);
        }
    }
    bindings
}
```

- [ ] Run `cargo test -p gameplay-drums --lib bindings::` — expect all tests to pass, including the five new ones.

- [ ] Commit:

```sh
git add crates/gameplay-drums/src/bindings.rs
git commit -m "feat(drums): resolve system verbs, skipping any source a lane owns"
```

---

## Task 5 — `SystemVerbHit` message, emitted from the MIDI path

**Files:** `crates/gameplay-drums/src/events.rs` (append), `crates/gameplay-drums/src/lib.rs` (message registration ~L145-150; `midi_consumer` L534-607 + its tests).

The emit sits **after** the velocity-threshold `continue` and **before** the `gameplay_ready` gate — exactly where `PadNavHit` already sits. That is why the verb fires during live play though `menu_nav::active_context` returns `None` there: it never travels the `NavAction` path.

- [ ] Add the message to the bottom of `crates/gameplay-drums/src/events.rs`:

```rust
/// A bound system verb fired by a key or a pad. Emitted from `DrumsSets::Input`
/// before the gameplay-ready gate, so it works during live play; consumers gate
/// themselves (`pause.rs`).
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemVerbHit {
    pub verb: dtx_input::SystemVerb,
}
```

- [ ] Register it in `crates/gameplay-drums/src/lib.rs` — add one line after `.add_message::<events::EmptyHit>()` (L149):

```rust
    .add_message::<events::SystemVerbHit>()
```

- [ ] Add the failing tests to the `midi_consumer::tests` module in `crates/gameplay-drums/src/lib.rs`:

```rust
        #[test]
        fn system_verb_fires_while_gameplay_is_not_ready() {
            use dtx_input::{BindSource, InputBindings, SystemVerb};
            let mut b = InputBindings::default();
            b.bind_system(SystemVerb::Pause, BindSource::Midi { note: 37 });
            let resolver = crate::bindings::BindResolver::from_bindings(&b);
            let mut last = LastMidiHit::default();

            // gameplay_ready = false — the live-play/menu case, and the whole feature.
            let out = consume_midi_events(
                [dtx_input::midi::MidiEvent::NoteOn {
                    note: 37,
                    velocity: 90,
                    audio_ms: 0,
                    captured_at: std::time::Instant::now(),
                }],
                &resolver,
                false,
                0,
                &mut last,
            );

            assert_eq!(out.verbs, vec![SystemVerb::Pause]);
            assert!(out.hits.is_empty());
            assert!(out.nav_lanes.is_empty(), "a system note is not a lane");
        }

        #[test]
        fn sub_threshold_system_note_emits_nothing() {
            use dtx_input::{BindSource, InputBindings, SystemVerb};
            let mut b = InputBindings::default();
            b.midi.velocity_threshold = 20;
            b.bind_system(SystemVerb::Pause, BindSource::Midi { note: 37 });
            let resolver = crate::bindings::BindResolver::from_bindings(&b);
            let mut last = LastMidiHit::default();

            let out = consume_midi_events(
                [dtx_input::midi::MidiEvent::NoteOn {
                    note: 37,
                    velocity: 15,
                    audio_ms: 0,
                    captured_at: std::time::Instant::now(),
                }],
                &resolver,
                true,
                0,
                &mut last,
            );

            assert!(out.verbs.is_empty(), "noise must not pause the song");
        }

        #[test]
        fn a_lane_note_never_emits_a_system_verb() {
            use dtx_input::{BindSource, InputBindings, SystemVerb};
            let mut b = InputBindings::default();
            // The footgun: 38 is the Snare's note, also hand-bound to Pause.
            b.bind_system(SystemVerb::Pause, BindSource::Midi { note: 38 });
            let resolver = crate::bindings::BindResolver::from_bindings(&b);
            let mut last = LastMidiHit::default();

            let out = consume_midi_events(
                [dtx_input::midi::MidiEvent::NoteOn {
                    note: 38,
                    velocity: 90,
                    audio_ms: 0,
                    captured_at: std::time::Instant::now(),
                }],
                &resolver,
                true,
                0,
                &mut last,
            );

            assert!(out.verbs.is_empty(), "a lane hit must never pause");
            assert_eq!(out.hits.len(), 1, "it still judges");
        }
```

- [ ] Run `cargo test -p gameplay-drums --lib midi_consumer::` — expect compile failure: `no field verbs on ConsumedMidi`.

- [ ] Add the field to `ConsumedMidi` (L559-564 in `crates/gameplay-drums/src/lib.rs`):

```rust
    struct ConsumedMidi {
        hits: Vec<InputHit>,
        /// Lanes for `PadNavHit`; emitted even when gameplay is not ready so
        /// pads can steer menus outside a run.
        nav_lanes: Vec<u8>,
        /// System verbs fired by this batch. Emitted on the same
        /// unconditional path as `nav_lanes` — the verb must work mid-song.
        verbs: Vec<dtx_input::SystemVerb>,
    }
```

- [ ] Emit the verbs in `consume_midi_events` (L566-607). Replace the body from `let mut hits = Vec::new();` to the final `ConsumedMidi { hits, nav_lanes }` with:

```rust
        let mut hits = Vec::new();
        let mut nav_lanes = Vec::new();
        let mut verbs = Vec::new();
        for ev in events {
            let dtx_input::midi::MidiEvent::NoteOn {
                note,
                velocity,
                audio_ms,
                captured_at,
            } = ev
            else {
                continue;
            };
            *last = LastMidiHit {
                note,
                velocity,
                below_threshold: velocity <= resolver.velocity_threshold,
                at: Some(std::time::Instant::now()),
            };
            if velocity == 0 || velocity <= resolver.velocity_threshold {
                continue;
            }
            // Before the gameplay_ready gate, exactly like nav_lanes: the verb
            // must fire mid-song, and a system note was never gameplay input.
            verbs.extend(resolver.system_for_note(note));
            let lanes: Vec<_> = resolver.lanes_for_note(note).collect();
            if let Some(&lane) = lanes.first() {
                nav_lanes.push(lane);
                if gameplay_ready {
                    hits.push(InputHit {
                        lanes,
                        audio_ms: stamp_audio_ms(Some(clock_ms), audio_ms),
                        captured_at,
                    });
                }
            }
        }
        ConsumedMidi {
            hits,
            nav_lanes,
            verbs,
        }
```

- [ ] Write the verbs out of `poll_midi` (L534-557). Add the writer param and the drain loop — replace the whole `poll_midi` fn with:

```rust
    fn poll_midi(
        mut source: ResMut<VirtualSource>,
        resolver: Res<crate::bindings::BindResolver>,
        chart: Res<crate::resources::ActiveChart>,
        clock: Res<GameplayClock>,
        mut hits: MessageWriter<InputHit>,
        mut nav_hits: MessageWriter<PadNavHit>,
        mut verb_hits: MessageWriter<super::events::SystemVerbHit>,
        mut last: ResMut<LastMidiHit>,
    ) {
        if source.is_empty() {
            return;
        }
        let mut buf: Vec<dtx_input::midi::MidiEvent> = Vec::new();
        (*source).poll(&mut buf);
        let gameplay_ready = !chart.chart.chips.is_empty() && clock.is_ready();
        let consumed =
            consume_midi_events(buf, &resolver, gameplay_ready, clock.current_ms, &mut last);
        for hit in consumed.hits {
            hits.write(hit);
        }
        for lane in consumed.nav_lanes {
            nav_hits.write(PadNavHit { lane });
        }
        for verb in consumed.verbs {
            verb_hits.write(super::events::SystemVerbHit { verb });
        }
    }
```

- [ ] Fix the two existing `midi_consumer` tests that destructure or assert on `ConsumedMidi` — they use field access (`hits.hits`, `out.nav_lanes`), which still compiles. Run `cargo test -p gameplay-drums --lib midi_consumer::` — expect all tests to pass, including the three new ones.

- [ ] Commit:

```sh
git add crates/gameplay-drums/src/events.rs crates/gameplay-drums/src/lib.rs
git commit -m "feat(drums): emit SystemVerbHit from the MIDI path before the gameplay gate"
```

---

## Task 6 — Keyboard-bound system verbs on the same message

**Files:** `crates/gameplay-drums/src/input.rs` (plugin L22-41; new system; tests).

The lane path (`capture_key_to_lane_input`) is gated `in_state(PauseState::Running)`. A system verb must **not** be: a paused game has to be un-pausable from the same key.

- [ ] Add the failing test to the `tests` module in `crates/gameplay-drums/src/input.rs`:

```rust
    #[test]
    fn bound_key_emits_the_system_verb() {
        use crate::editor::bindings_capture::CaptureState;
        use crate::events::SystemVerbHit;
        use bevy::prelude::*;
        use dtx_input::{BindSource, InputBindings, SystemVerb};

        let mut bindings = InputBindings::default();
        bindings.bind_system(SystemVerb::Pause, BindSource::Key(KeyCode::F9));

        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<CaptureState>()
            .insert_resource(crate::bindings::BindResolver::from_bindings(&bindings))
            .add_message::<SystemVerbHit>()
            .add_systems(Update, keyboard_system_verbs);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F9);
        app.update();

        let hits: Vec<SystemVerbHit> = app
            .world()
            .resource::<Messages<SystemVerbHit>>()
            .iter_current_update_messages()
            .copied()
            .collect();
        assert_eq!(hits, vec![SystemVerbHit { verb: SystemVerb::Pause }]);
    }

    #[test]
    fn an_armed_capture_swallows_the_system_verb() {
        use crate::editor::bindings_capture::CaptureState;
        use crate::events::SystemVerbHit;
        use bevy::prelude::*;
        use dtx_input::{BindSource, InputBindings, SystemVerb};

        let mut bindings = InputBindings::default();
        bindings.bind_system(SystemVerb::Pause, BindSource::Key(KeyCode::F9));

        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(CaptureState::Keyboard(dtx_core::EChannel::Snare))
            .insert_resource(crate::bindings::BindResolver::from_bindings(&bindings))
            .add_message::<SystemVerbHit>()
            .add_systems(Update, keyboard_system_verbs);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F9);
        app.update();

        assert_eq!(
            app.world()
                .resource::<Messages<SystemVerbHit>>()
                .iter_current_update_messages()
                .count(),
            0,
            "a key pressed while capturing must not fire a verb"
        );
    }
```

- [ ] Run `cargo test -p gameplay-drums --lib input::` — expect compile failure: `cannot find function keyboard_system_verbs`.

- [ ] Add the system to `crates/gameplay-drums/src/input.rs`, after `capture_key_to_lane_input` (L78):

```rust
/// Keyboard-bound system verbs → `SystemVerbHit`, on the same message the MIDI
/// path writes. Deliberately NOT gated on `PauseState::Running`: the key that
/// paused the song has to be able to un-pause it. Consumers (`pause.rs`) carry
/// their own gates.
fn keyboard_system_verbs(
    keys: Res<ButtonInput<KeyCode>>,
    resolver: Res<BindResolver>,
    capture: Res<crate::editor::bindings_capture::CaptureState>,
    mut out: MessageWriter<crate::events::SystemVerbHit>,
) {
    if !matches!(
        *capture,
        crate::editor::bindings_capture::CaptureState::Idle
    ) {
        return; // the capture flow owns the keyboard
    }
    for key in keys.get_just_pressed() {
        for verb in resolver.system_for_key(*key) {
            out.write(crate::events::SystemVerbHit { verb });
        }
    }
}
```

- [ ] Register it in `input::plugin` — replace the `PreUpdate` `add_systems` block (L34-40 boundary) so both systems run there. The final `plugin` body reads:

```rust
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<PendingLaneInputs>()
        .add_systems(
            PreUpdate,
            // Not gated on `editor_closed`: while the Customize surface is open we
            // still want captured hits to reach `LaneHit` (flash + feedback sound).
            // Scoring is gated instead — see `judge::judge_lane_hit_system`.
            capture_key_to_lane_input
                .after(bevy::input::InputSystems)
                .run_if(in_state(game_shell::AppState::Performance))
                .run_if(in_state(game_shell::PauseState::Running)),
        )
        .add_systems(
            PreUpdate,
            // NO PauseState gate: this is the key that has to un-pause the song.
            keyboard_system_verbs
                .after(bevy::input::InputSystems)
                .run_if(in_state(game_shell::AppState::Performance)),
        )
        .add_systems(
            FixedUpdate,
            emit_pending_lane_hits
                .in_set(super::DrumsSets::Input)
                .run_if(in_state(game_shell::AppState::Performance))
                .run_if(in_state(game_shell::PauseState::Running)),
        );
}
```

- [ ] Run `cargo test -p gameplay-drums --lib input::` — expect all tests to pass, including the two new ones.

- [ ] Commit:

```sh
git add crates/gameplay-drums/src/input.rs
git commit -m "feat(drums): emit system verbs from keyboard binds, ungated by pause"
```

---

## Task 7 — `SystemVerb::Pause` consumer

**Files:** `crates/gameplay-drums/src/pause.rs` (plugin L83-110; new system after `toggle_pause` L131; tests).

Toggles both directions and sets `PracticePauseSurface::Overlay` before pausing, so the practice rail does not steal the surface. This one slot closes **F1 and F3 together**: it opens the overlay that already carries pad grammar (HH/CY/BD/SD) and a Quit row.

- [ ] Add the failing tests to the `tests` module in `crates/gameplay-drums/src/pause.rs`:

```rust
    fn verb_world(state: PauseState, verb: dtx_input::SystemVerb) -> World {
        use bevy::ecs::message::Messages;
        let mut world = World::new();
        world.init_resource::<Messages<crate::events::SystemVerbHit>>();
        world.init_resource::<Messages<TransitionRequest>>();
        world.insert_resource(State::new(state));
        world.init_resource::<NextState<PauseState>>();
        world.insert_resource(PracticePauseSurface::Rail);
        world.write_message(crate::events::SystemVerbHit { verb });
        world
    }

    #[test]
    fn pause_verb_opens_the_overlay_surface() {
        let mut world = verb_world(PauseState::Running, dtx_input::SystemVerb::Pause);
        world
            .run_system_once(system_verb_pause)
            .expect("system_verb_pause runs");
        assert_eq!(
            *world.resource::<PracticePauseSurface>(),
            PracticePauseSurface::Overlay,
            "the practice rail must not steal the surface"
        );
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Paused)
        ));
    }

    #[test]
    fn pause_verb_while_paused_resumes() {
        let mut world = verb_world(PauseState::Paused, dtx_input::SystemVerb::Pause);
        world
            .run_system_once(system_verb_pause)
            .expect("system_verb_pause runs");
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Running)
        ));
    }

    #[test]
    fn restart_verb_does_not_toggle_pause() {
        let mut world = verb_world(PauseState::Running, dtx_input::SystemVerb::Restart);
        world
            .run_system_once(system_verb_pause)
            .expect("system_verb_pause runs");
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Unchanged
        ));
    }
```

- [ ] Run `cargo test -p gameplay-drums --lib pause::` — expect compile failure: `cannot find function system_verb_pause`.

- [ ] Add the consumer to `crates/gameplay-drums/src/pause.rs`, right after `toggle_pause` (L131):

```rust
/// `SystemVerb::Pause` from a pad or a bound key — the distant-kit equivalent of
/// Escape. Shares `toggle_pause`'s body: it toggles, and firing it while paused
/// resumes. Gated to Performance with the editor closed (see `plugin`).
fn system_verb_pause(
    mut hits: MessageReader<crate::events::SystemVerbHit>,
    state: Res<State<PauseState>>,
    mut next: ResMut<NextState<PauseState>>,
    mut surface: ResMut<PracticePauseSurface>,
) {
    if !hits
        .read()
        .any(|hit| hit.verb == dtx_input::SystemVerb::Pause)
    {
        return;
    }
    match state.get() {
        PauseState::Running => {
            *surface = PracticePauseSurface::Overlay;
            next.set(PauseState::Paused);
        }
        PauseState::Paused => next.set(PauseState::Running),
    }
}
```

- [ ] Register it beside `toggle_pause` in `pause::plugin` — replace the `toggle_pause` `add_systems` block (L90-95) with:

```rust
        .add_systems(
            Update,
            (toggle_pause, system_verb_pause)
                .run_if(in_state(AppState::Performance))
                .run_if(crate::editor::editor_closed),
        )
```

- [ ] Run `cargo test -p gameplay-drums --lib pause::` — expect all tests to pass, including the three new ones.

- [ ] Commit:

```sh
git add crates/gameplay-drums/src/pause.rs
git commit -m "feat(drums): pause from the kit with SystemVerb::Pause"
```

---

## Task 8 — `SystemVerb::Restart` consumer

**Files:** `crates/gameplay-drums/src/pause.rs` (plugin; new system after `system_verb_pause`; tests).

Same action the pause menu's Retry row takes (`pause_menu_input`, `PauseItemKind::Retry`): resume, then `request_transition(SongLoading)` — which preserves `SelectedSong` and `PracticeIntent`. It fires during `Performance` whether running or paused.

> **Named risk, accepted (from the spec).** A stray hit on the bound note restarts the song. The note is one the user deliberately chose and can unbind. If it proves annoying, the fix is a `in_state(PauseState::Paused)` gate — at the cost of the convenience that motivated the slot.

- [ ] Add the failing test to the `tests` module in `crates/gameplay-drums/src/pause.rs`:

```rust
    #[test]
    fn restart_verb_requests_song_loading_while_running() {
        use bevy::ecs::message::Messages;
        let mut world = verb_world(PauseState::Running, dtx_input::SystemVerb::Restart);
        world
            .run_system_once(system_verb_restart)
            .expect("system_verb_restart runs");
        let targets: Vec<AppState> = world
            .resource::<Messages<TransitionRequest>>()
            .iter_current_update_messages()
            .map(|r| r.0)
            .collect();
        assert_eq!(targets, vec![AppState::SongLoading]);
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Running)
        ));
    }

    #[test]
    fn restart_verb_also_fires_while_paused() {
        use bevy::ecs::message::Messages;
        let mut world = verb_world(PauseState::Paused, dtx_input::SystemVerb::Restart);
        world
            .run_system_once(system_verb_restart)
            .expect("system_verb_restart runs");
        let targets: Vec<AppState> = world
            .resource::<Messages<TransitionRequest>>()
            .iter_current_update_messages()
            .map(|r| r.0)
            .collect();
        assert_eq!(targets, vec![AppState::SongLoading]);
    }

    #[test]
    fn pause_verb_does_not_restart() {
        use bevy::ecs::message::Messages;
        let mut world = verb_world(PauseState::Running, dtx_input::SystemVerb::Pause);
        world
            .run_system_once(system_verb_restart)
            .expect("system_verb_restart runs");
        assert_eq!(
            world
                .resource::<Messages<TransitionRequest>>()
                .iter_current_update_messages()
                .count(),
            0
        );
    }
```

- [ ] Run `cargo test -p gameplay-drums --lib pause::` — expect compile failure: `cannot find function system_verb_restart`.

- [ ] Add the consumer to `crates/gameplay-drums/src/pause.rs`, right after `system_verb_pause`:

```rust
/// `SystemVerb::Restart` — re-request `SongLoading`, exactly as the pause menu's
/// Retry row does, preserving `SelectedSong` and `PracticeIntent`. Fires during
/// Performance whether running or paused.
fn system_verb_restart(
    mut hits: MessageReader<crate::events::SystemVerbHit>,
    mut next_pause: ResMut<NextState<PauseState>>,
    mut requests: MessageWriter<TransitionRequest>,
) {
    if !hits
        .read()
        .any(|hit| hit.verb == dtx_input::SystemVerb::Restart)
    {
        return;
    }
    next_pause.set(PauseState::Running);
    request_transition(&mut requests, AppState::SongLoading);
}
```

- [ ] Register it beside the other two — replace the `add_systems` block from Task 7 with:

```rust
        .add_systems(
            Update,
            (toggle_pause, system_verb_pause, system_verb_restart)
                .run_if(in_state(AppState::Performance))
                .run_if(crate::editor::editor_closed),
        )
```

- [ ] Run `cargo test -p gameplay-drums --lib pause::` — expect all tests to pass, including the three new ones.

- [ ] Commit:

```sh
git add crates/gameplay-drums/src/pause.rs
git commit -m "feat(drums): restart the song from the kit with SystemVerb::Restart"
```

---

## Task 9 — F4: cancel a load from the kit (SD)

**Files:** `crates/gameplay-drums/src/menu_nav.rs` (`NavContext` L27-39, `active_context` L102-127, its test ~L287-296), `crates/game-menu/src/song_loading.rs` (imports L33, `watch_cancel_key` L484-502, the "Esc — cancel" label L471-475, tests).

**Independent of Tasks 1-8.** It needs no hardware, no bind, and no new grammar — it can be implemented and reviewed on its own. SD-as-back is the established grammar everywhere else, loading is not live play, and the existing 500 ms `ENTER_GRACE` in `NavGuard` stops the hit that confirmed the song from cancelling its own load.

- [ ] In `crates/gameplay-drums/src/menu_nav.rs`, add the variant to `NavContext` (after `Editor`, L38):

```rust
    /// Customize (settings) overlay during a performance.
    Editor,
    /// Chart/audio load in progress. Pads may cancel it (SD = Back).
    Loading,
```

- [ ] Add the arm to `active_context` — replace the `_ => None` fallthrough (L125) with:

```rust
        AppState::SongLoading => Some(NavContext::Loading),
        _ => None,
```

- [ ] Update the existing `no_context_during_live_play_or_capture` test (~L287-296): the `AppState::SongLoading` assertion now expects `Some(NavContext::Loading)`:

```rust
        assert_eq!(
            active_context(
                &AppState::SongLoading,
                &PauseState::Running,
                false,
                false,
                false
            ),
            Some(NavContext::Loading)
        );
```

- [ ] Run `cargo test -p gameplay-drums --lib menu_nav::` — expect all tests to pass.

- [ ] Add the failing test to the `tests` module at the bottom of `crates/game-menu/src/song_loading.rs`:

```rust
    #[test]
    fn pad_back_cancels_the_load() {
        use bevy::ecs::message::Messages;
        use bevy::ecs::system::RunSystemOnce;
        use game_shell::{NavAction, NavSource, NavVerb};

        let mut world = World::new();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<Messages<NavAction>>();
        world.init_resource::<CancelRequested>();
        world.insert_resource(LoadPhase::Parsing);
        world.write_message(NavAction {
            verb: NavVerb::Back,
            source: NavSource::Pad,
            coarse: false,
        });

        world
            .run_system_once(watch_cancel_key)
            .expect("watch_cancel_key runs");

        assert!(world.resource::<CancelRequested>().0, "SD cancels the load");
    }

    #[test]
    fn pad_confirm_does_not_cancel_the_load() {
        use bevy::ecs::message::Messages;
        use bevy::ecs::system::RunSystemOnce;
        use game_shell::{NavAction, NavSource, NavVerb};

        let mut world = World::new();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<Messages<NavAction>>();
        world.init_resource::<CancelRequested>();
        world.insert_resource(LoadPhase::Parsing);
        world.write_message(NavAction {
            verb: NavVerb::Confirm,
            source: NavSource::Pad,
            coarse: false,
        });

        world
            .run_system_once(watch_cancel_key)
            .expect("watch_cancel_key runs");

        assert!(!world.resource::<CancelRequested>().0);
    }
```

- [ ] Run `cargo test -p game-menu --lib song_loading::` — expect compile failure: `watch_cancel_key` takes 3 params, not 4 / `NavAction` unused.

- [ ] Add the `NavAction` reader. In `crates/game-menu/src/song_loading.rs`, extend the `game_shell` import (L33):

```rust
use game_shell::{
    AppState, NavAction, NavVerb, TransitionRequest, despawn_stage, request_transition,
};
```

- [ ] Replace `watch_cancel_key` (L481-502) with:

```rust
/// Watch for a cancel during load: Esc on the keyboard, or `NavVerb::Back` — SD
/// — from the kit (`menu_nav` emits it while `NavContext::Loading` is active).
/// On cancel, mark the load; the next `poll_chart_parse` tick sees the flag and
/// fails fast, and `advance_when_loaded` routes back to SongSelect.
fn watch_cancel_key(
    keys: Res<ButtonInput<KeyCode>>,
    mut actions: MessageReader<NavAction>,
    mut cancel: ResMut<CancelRequested>,
    phase: Res<LoadPhase>,
) {
    if cancel.0 {
        actions.clear();
        return;
    }
    if matches!(
        *phase,
        LoadPhase::Idle | LoadPhase::Ready | LoadPhase::Failed
    ) {
        actions.clear();
        return;
    }
    let pad_back = actions.read().any(|action| action.verb == NavVerb::Back);
    if keys.just_pressed(KeyCode::Escape) || pad_back {
        info!("SongLoading: cancel requested — cancelling load");
        cancel.0 = true;
    }
}
```

- [ ] Update the on-screen hint so the pad path is discoverable — replace the `Text::new("Esc — cancel")` (L472) with:

```rust
                        Text::new("Esc / SD — cancel"),
```

- [ ] Run `cargo test -p game-menu --lib song_loading::` — expect all tests to pass, including the two new ones.

- [ ] Commit:

```sh
git add crates/gameplay-drums/src/menu_nav.rs crates/game-menu/src/song_loading.rs
git commit -m "feat(menu): cancel a song load from the kit with SD"
```

---

## Task 10 — Controls-tab capture for system verbs (refusal in place)

**Files:** `crates/gameplay-drums/src/editor/bindings_capture.rs` (`CaptureState` L36-70, `capture_binding` L280-485, `sync_selected_channel_on_capture` L530-541, plugin L125-141, tests), `crates/gameplay-drums/src/editor/capture_modal.rs` (`modal_lines` L114-157, `listening_midi` L253), `crates/gameplay-drums/src/editor/footer.rs` (`capture_footer_text` L121-141).

System capture has **no Arrived stage**: there is nothing to share or move. A colliding source is **refused in place, naming the owning lane**, and the capture stays armed for another try. The refusal rides inside `CaptureState` so `modal_lines`' `PartialEq` signature repaints the modal for free.

- [ ] Add the failing tests to the `tests` module in `crates/gameplay-drums/src/editor/bindings_capture.rs`:

```rust
    #[test]
    fn system_capture_refuses_a_lane_owned_source() {
        use dtx_core::EChannel;
        assert_eq!(
            system_capture_step(
                false,
                Some(BindSource::Midi { note: 38 }),
                Some(EChannel::Snare)
            ),
            SystemCaptureStep::Refused(EChannel::Snare)
        );
    }

    #[test]
    fn system_capture_binds_a_free_source() {
        assert_eq!(
            system_capture_step(false, Some(BindSource::Midi { note: 37 }), None),
            SystemCaptureStep::Bind(BindSource::Midi { note: 37 })
        );
        assert_eq!(
            system_capture_step(false, None, None),
            SystemCaptureStep::Pending
        );
        assert_eq!(
            system_capture_step(true, Some(BindSource::Midi { note: 37 }), None),
            SystemCaptureStep::Cancelled
        );
    }

    #[test]
    fn system_key_capture_commits_a_free_key() {
        use dtx_input::SystemVerb;
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<CaptureState>()
            .init_resource::<LiveBindings>()
            .init_resource::<BindingsRev>()
            .init_resource::<crate::LastMidiHit>()
            .init_resource::<GameplayClock>()
            .init_resource::<MouseArrivedInput>()
            .add_message::<LaneHit>()
            .insert_resource(CaptureState::SystemKey {
                verb: SystemVerb::Pause,
                refused: None,
            })
            .add_systems(Update, capture_binding);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F9);
        app.update();

        let live = app.world().resource::<LiveBindings>();
        assert_eq!(
            live.0.system_sources(SystemVerb::Pause),
            [BindSource::Key(KeyCode::F9)]
        );
        assert!(matches!(
            *app.world().resource::<CaptureState>(),
            CaptureState::Idle
        ));
    }

    #[test]
    fn system_key_capture_refuses_a_lane_key_in_place() {
        use dtx_core::EChannel;
        use dtx_input::SystemVerb;
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<CaptureState>()
            .init_resource::<LiveBindings>()
            .init_resource::<BindingsRev>()
            .init_resource::<crate::LastMidiHit>()
            .init_resource::<GameplayClock>()
            .init_resource::<MouseArrivedInput>()
            .add_message::<LaneHit>()
            .insert_resource(CaptureState::SystemKey {
                verb: SystemVerb::Pause,
                refused: None,
            })
            .add_systems(Update, capture_binding);
        // Space is the BassDrum default.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.update();

        let live = app.world().resource::<LiveBindings>();
        assert!(
            live.0.system_sources(SystemVerb::Pause).is_empty(),
            "a lane key must not bind a verb"
        );
        assert!(
            matches!(
                *app.world().resource::<CaptureState>(),
                CaptureState::SystemKey {
                    refused: Some(EChannel::BassDrum),
                    ..
                }
            ),
            "the capture stays armed, naming the owning lane"
        );
    }
```

- [ ] Run `cargo test -p gameplay-drums --lib bindings_capture::` — expect compile failure: `cannot find SystemCaptureStep` / `no variant SystemKey`.

- [ ] Add the two variants to `CaptureState` in `crates/gameplay-drums/src/editor/bindings_capture.rs` (after `Midi(EChannel)`, L44):

```rust
    /// `Learn pad`: listening for the next new NoteOn for this channel.
    Midi(dtx_core::EChannel),
    /// System-verb key capture. `refused` names the lane that owns the last key
    /// tried — the bind is refused in place and the capture stays armed.
    SystemKey {
        verb: dtx_input::SystemVerb,
        refused: Option<dtx_core::EChannel>,
    },
    /// System-verb pad capture. `refused` as above.
    SystemMidi {
        verb: dtx_input::SystemVerb,
        refused: Option<dtx_core::EChannel>,
    },
```

- [ ] Add the pure step reducer after `midi_capture_step` (L220):

```rust
/// Outcome of one system-verb capture step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemCaptureStep {
    Pending,
    Cancelled,
    /// The source belongs to a lane. Refused in place, naming the owner —
    /// unlike NX, which silently auto-unbinds the lane (`CConfigIni.cs:1524`).
    Refused(dtx_core::EChannel),
    Bind(BindSource),
}

/// Pure system-capture decision. Esc cancels; a source a lane owns is refused,
/// never stolen (lanes win ties); a free source binds immediately — there is
/// nothing to share or move, so no `Arrived` stage.
pub fn system_capture_step(
    escape: bool,
    src: Option<BindSource>,
    owner: Option<dtx_core::EChannel>,
) -> SystemCaptureStep {
    if escape {
        return SystemCaptureStep::Cancelled;
    }
    match (src, owner) {
        (Some(_), Some(channel)) => SystemCaptureStep::Refused(channel),
        (Some(src), None) => SystemCaptureStep::Bind(src),
        (None, _) => SystemCaptureStep::Pending,
    }
}
```

- [ ] Add the two arms to the `match std::mem::take(&mut *capture)` in `capture_binding` — insert them after the `CaptureState::Midi(channel)` arm (L389, before `CaptureState::KeyArrived`):

```rust
        CaptureState::SystemKey { verb, refused } => {
            let key = if modifier_held(&keys) {
                None
            } else {
                keys.get_just_pressed().copied().find(|k| !is_reserved(*k))
            };
            let src = key.map(BindSource::Key);
            let owner = src.and_then(|src| dtx_input::lane_owner(&live.0, &src));
            let step = system_capture_step(
                keys.just_pressed(KeyCode::Escape) || mouse_cancel,
                src,
                owner,
            );
            match step {
                SystemCaptureStep::Pending => CaptureState::SystemKey { verb, refused },
                SystemCaptureStep::Cancelled => CaptureState::Idle,
                SystemCaptureStep::Refused(channel) => CaptureState::SystemKey {
                    verb,
                    refused: Some(channel),
                },
                SystemCaptureStep::Bind(src) => {
                    live.0.bind_system(verb, src);
                    rev.0 = rev.0.wrapping_add(1);
                    CaptureState::Idle
                }
            }
        }
        CaptureState::SystemMidi { verb, refused } => {
            // Advancing `seen_midi_at` dedupes a held/sustained note so it
            // can't re-bind every frame.
            let new_note = strictly_new_note(
                last_midi.note,
                last_midi.velocity,
                last_midi.at,
                *seen_midi_at,
            );
            if new_note.is_some() {
                *seen_midi_at = last_midi.at;
            }
            let src = new_note.map(|note| BindSource::Midi { note });
            let owner = src.and_then(|src| dtx_input::lane_owner(&live.0, &src));
            let step = system_capture_step(
                keys.just_pressed(KeyCode::Escape) || mouse_cancel,
                src,
                owner,
            );
            match step {
                SystemCaptureStep::Pending => CaptureState::SystemMidi { verb, refused },
                SystemCaptureStep::Cancelled => CaptureState::Idle,
                SystemCaptureStep::Refused(channel) => CaptureState::SystemMidi {
                    verb,
                    refused: Some(channel),
                },
                SystemCaptureStep::Bind(src) => {
                    live.0.bind_system(verb, src);
                    rev.0 = rev.0.wrapping_add(1);
                    CaptureState::Idle
                }
            }
        }
```

- [ ] Make `sync_selected_channel_on_capture` exhaustive again (L530-541) — a system capture targets no channel, so it leaves the lane selection alone. Replace its `match`:

```rust
    let channel = match &*capture {
        CaptureState::Idle
        | CaptureState::SystemKey { .. }
        | CaptureState::SystemMidi { .. } => return,
        CaptureState::Keyboard(ch)
        | CaptureState::Midi(ch)
        | CaptureState::KeyArrived { channel: ch, .. }
        | CaptureState::MidiArrived { channel: ch, .. } => *ch,
    };
```

- [ ] Make `modal_lines` exhaustive in `crates/gameplay-drums/src/editor/capture_modal.rs` — add two arms before the closing brace of the `match` (after the `MidiArrived` arm, L155):

```rust
        CaptureState::SystemKey { verb, refused } => Some(ModalLines {
            title: format!("Press a key for {}", verb.label()),
            subtitle: Some("Esc cancel".to_string()),
            arrived: None,
            owners_caption: refused.map(|ch| {
                format!(
                    "already bound to the {} lane — pick another",
                    channel_name(ch)
                )
            }),
            choice: None,
        }),
        CaptureState::SystemMidi { verb, refused } => Some(ModalLines {
            title: format!("Hit a pad for {}", verb.label()),
            subtitle: Some("Esc cancel".to_string()),
            arrived: None,
            owners_caption: refused.map(|ch| {
                format!(
                    "already bound to the {} lane — pick another",
                    channel_name(ch)
                )
            }),
            choice: None,
        }),
```

- [ ] Show the live velocity line during a system pad capture too — in `sync_capture_modal` (L253) replace:

```rust
    let listening_midi = matches!(
        *capture,
        CaptureState::Midi(_) | CaptureState::SystemMidi { .. }
    );
```

- [ ] Make `capture_footer_text` exhaustive in `crates/gameplay-drums/src/editor/footer.rs` — add three arms **before** the `KeyArrived`/`MidiArrived` arms (the refusal arm must come first so its guard wins), i.e. insert after the `CaptureState::Midi(channel)` arm (L132):

```rust
        CaptureState::SystemKey {
            refused: Some(owner),
            ..
        }
        | CaptureState::SystemMidi {
            refused: Some(owner),
            ..
        } => Some(format!(
            "{} already drives that input — pick another",
            owner.short_name().unwrap_or("a lane")
        )),
        CaptureState::SystemKey { verb, .. } => Some(format!(
            "Press a key for {} — Esc cancels",
            verb.label()
        )),
        CaptureState::SystemMidi { verb, .. } => Some(format!(
            "Hit a pad for {} — Esc cancels",
            verb.label()
        )),
```

- [ ] Run `cargo test -p gameplay-drums --lib` — expect all tests to pass, including the four new ones.

- [ ] Commit:

```sh
git add crates/gameplay-drums/src/editor/bindings_capture.rs crates/gameplay-drums/src/editor/capture_modal.rs crates/gameplay-drums/src/editor/footer.rs
git commit -m "feat(customize): capture system-verb binds, refusing lane-owned sources in place"
```

---

## Task 11 — Controls-tab **System** section

**Files:** `crates/gameplay-drums/src/editor/controls_panel.rs` (`RowStep`/`step_channel` L143-177, `controls_nav_consumer` L191-289, tests), `crates/gameplay-drums/src/editor/bindings_panel.rs` (components L23-53, `last_segment_source_index` L209-218, `segment_rows` L223-246, `spawn_bindings_block` L248-269, handlers, plugin L109-141, tests), `crates/gameplay-drums/src/editor/bindings_capture.rs` (`highlight_selected_system_row`, plugin), `crates/gameplay-drums/src/editor/mod.rs` (L125-126).

**Do not touch `pad_excluded` or the `pad_exclusion_matches_controls_contract` test.** Pads stay excluded from Controls-tab navigation: a stray pad hit while testing bindings must not move focus. The System rows are keyboard-driven like every other Controls row.

- [ ] Add the failing tests to the `tests` module in `crates/gameplay-drums/src/editor/controls_panel.rs`:

```rust
    #[test]
    fn controls_rows_put_system_verbs_after_the_lane_rows() {
        use dtx_input::{SystemVerb, BINDABLE_CHANNELS};
        let rows = controls_rows(&dtx_layout::classic());
        assert_eq!(rows.len(), BINDABLE_CHANNELS.len() + 2);
        assert_eq!(
            rows[BINDABLE_CHANNELS.len()],
            ControlsRow::System(SystemVerb::Pause)
        );
        assert_eq!(
            rows[BINDABLE_CHANNELS.len() + 1],
            ControlsRow::System(SystemVerb::Restart)
        );
        assert!(matches!(rows[0], ControlsRow::Channel(_)));
    }

    #[test]
    fn row_cursor_walks_from_the_last_lane_into_the_system_rows() {
        use dtx_core::EChannel;
        use dtx_input::SystemVerb;
        let rows = [
            ControlsRow::Channel(EChannel::Snare),
            ControlsRow::System(SystemVerb::Pause),
            ControlsRow::System(SystemVerb::Restart),
        ];
        assert_eq!(
            step_row(&rows, Some(ControlsRow::Channel(EChannel::Snare)), 1),
            RowStep::Select(ControlsRow::System(SystemVerb::Pause))
        );
        assert_eq!(
            step_row(&rows, Some(ControlsRow::System(SystemVerb::Restart)), 1),
            RowStep::Select(ControlsRow::System(SystemVerb::Restart)),
            "clamps at the bottom"
        );
        assert_eq!(
            step_row(&rows, Some(ControlsRow::System(SystemVerb::Pause)), -1),
            RowStep::Select(ControlsRow::Channel(EChannel::Snare)),
            "Up returns to the lanes"
        );
    }

    #[test]
    fn current_row_prefers_the_system_cursor() {
        use dtx_core::EChannel;
        use dtx_input::SystemVerb;
        assert_eq!(
            current_row(Some(EChannel::Snare), Some(SystemVerb::Pause)),
            Some(ControlsRow::System(SystemVerb::Pause))
        );
        assert_eq!(
            current_row(Some(EChannel::Snare), None),
            Some(ControlsRow::Channel(EChannel::Snare))
        );
        assert_eq!(current_row(None, None), None);
    }
```

- [ ] Run `cargo test -p gameplay-drums --lib controls_panel::` — expect compile failure: `cannot find function controls_rows`.

- [ ] Generalize the row cursor in `crates/gameplay-drums/src/editor/controls_panel.rs`. Replace `RowStep` + `step_channel` (L143-177) with:

```rust
/// One focusable row in the Controls tab: the lane rows in display order,
/// then one row per system verb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlsRow {
    Channel(EChannel),
    System(dtx_input::SystemVerb),
}

/// The Controls row cursor when it sits on a System row. `None` = the cursor is
/// on a lane row (`SelectedChannel` owns it, and the spatial lane display and
/// the capture sync keep reading that resource untouched).
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct SelectedSystem(pub Option<dtx_input::SystemVerb>);

/// Every Controls row, in focus order: lanes (display order), then verbs.
pub fn controls_rows(arrangement: &LaneArrangement) -> Vec<ControlsRow> {
    super::bindings_panel::bindable_channels_in_order(arrangement)
        .into_iter()
        .map(ControlsRow::Channel)
        .chain(dtx_input::SYSTEM_VERBS.into_iter().map(ControlsRow::System))
        .collect()
}

/// The row the cursor is on. The system cursor wins when set; clearing it
/// (`SelectedSystem(None)`) hands the cursor back to `SelectedChannel`.
pub fn current_row(
    selected: Option<EChannel>,
    system: Option<dtx_input::SystemVerb>,
) -> Option<ControlsRow> {
    system
        .map(ControlsRow::System)
        .or(selected.map(ControlsRow::Channel))
}

/// Outcome of one Up/Down step through the Controls rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowStep<T> {
    /// Up from the first row: focus returns to the segment selector.
    ToSegmentSelector,
    /// The row cursor lands on this row.
    Select(T),
    /// Nothing to do (Down on an empty row list).
    None,
}

/// Step the row cursor through `rows` (the panel's display order). A missing or
/// stale `current` clamps to the first row; Up from the first row hands focus
/// back to the segment selector (the reducer's Rows+Up arm).
pub fn step_row<T: Copy + PartialEq>(rows: &[T], current: Option<T>, dir: i32) -> RowStep<T> {
    if rows.is_empty() {
        return if dir < 0 {
            RowStep::ToSegmentSelector
        } else {
            RowStep::None
        };
    }
    let Some(index) = current.and_then(|row| rows.iter().position(|r| *r == row)) else {
        return RowStep::Select(rows[0]);
    };
    if dir < 0 {
        if index == 0 {
            RowStep::ToSegmentSelector
        } else {
            RowStep::Select(rows[index - 1])
        }
    } else {
        RowStep::Select(rows[(index + 1).min(rows.len() - 1)])
    }
}
```

- [ ] Update the existing `row_step_walks_display_order_and_hands_off_at_top` test (L443-477) — rename every `step_channel(` call to `step_row(`. Its assertions are unchanged (`RowStep<EChannel>` infers).

- [ ] Rewrite `controls_nav_consumer` (L190-289) to walk both row kinds. Replace the whole function with:

```rust
#[allow(clippy::too_many_arguments)]
pub(super) fn controls_nav_consumer(
    mut actions: MessageReader<NavAction>,
    keys: Res<ButtonInput<KeyCode>>,
    active: Res<super::tabs::ActiveTab>,
    lanes: Res<Lanes>,
    mut capture: ResMut<CaptureState>,
    mut focus: ResMut<ControlsFocus>,
    mut segment: ResMut<ControlsSegment>,
    mut selected: ResMut<SelectedChannel>,
    mut system: ResMut<SelectedSystem>,
    mut live: ResMut<LiveBindings>,
    mut rev: ResMut<BindingsRev>,
) {
    if active.0 != game_shell::CustomizeTab::Controls {
        return; // own reader: unread messages just expire
    }
    if active.is_changed() && *focus != ControlsFocus::TabBar {
        // Fresh visit (tab switched here): keyboard focus restarts at the bar.
        *focus = ControlsFocus::TabBar;
    }
    // Only the state matters, never `capture.is_changed()`: `capture_binding`
    // does a `mem::take` on `CaptureState` every frame, so the change tick is
    // always set and testing it would dead-lock this consumer forever. The
    // two self-capture hazards are already closed elsewhere — we run
    // `.after(capture_binding)`, and `keyboard_emit_nav` is gated on
    // `not(capture_active)` so a capture keypress emits no verb at all.
    if !matches!(*capture, CaptureState::Idle) {
        actions.clear();
        return;
    }
    let rows = controls_rows(&lanes.0);
    let cursor = current_row(selected.0, system.0);
    // Backspace is not a NavVerb — read it directly, Rows level only.
    if *focus == ControlsFocus::Rows && keys.just_pressed(KeyCode::Backspace) {
        match cursor {
            Some(ControlsRow::Channel(channel)) => {
                if let Some(index) =
                    super::bindings_panel::last_segment_source_index(&live.0, channel, *segment)
                {
                    if let Some(sources) = live.0.map.get_mut(&channel) {
                        sources.remove(index);
                        rev.0 = rev.0.wrapping_add(1);
                    }
                }
            }
            Some(ControlsRow::System(verb)) => {
                if let Some(index) =
                    super::bindings_panel::last_system_source_index(&live.0, verb, *segment)
                {
                    if let Some(sources) = live.0.system.get_mut(&verb) {
                        sources.remove(index);
                        rev.0 = rev.0.wrapping_add(1);
                    }
                }
            }
            None => {}
        }
    }
    for action in actions.read() {
        if action.source != NavSource::Keyboard {
            continue;
        }
        match (*focus, action.verb) {
            (ControlsFocus::Rows, NavVerb::Up) | (ControlsFocus::Rows, NavVerb::Down) => {
                let dir = if action.verb == NavVerb::Up { -1 } else { 1 };
                match step_row(&rows, current_row(selected.0, system.0), dir) {
                    RowStep::ToSegmentSelector => {
                        let (next_focus, next_segment) =
                            reduce_controls_nav(*focus, *segment, NavVerb::Up);
                        if *focus != next_focus {
                            *focus = next_focus;
                        }
                        if *segment != next_segment {
                            *segment = next_segment;
                        }
                    }
                    RowStep::Select(ControlsRow::Channel(channel)) => {
                        if selected.0 != Some(channel) {
                            selected.0 = Some(channel);
                        }
                        if system.0.is_some() {
                            system.0 = None;
                        }
                    }
                    RowStep::Select(ControlsRow::System(verb)) => {
                        if system.0 != Some(verb) {
                            system.0 = Some(verb);
                        }
                    }
                    RowStep::None => {}
                }
            }
            (ControlsFocus::Rows, NavVerb::Confirm) => {
                match current_row(selected.0, system.0).filter(|row| rows.contains(row)) {
                    Some(ControlsRow::Channel(channel)) => {
                        *capture = match *segment {
                            ControlsSegment::Keyboard => CaptureState::Keyboard(channel),
                            ControlsSegment::Midi => CaptureState::Midi(channel),
                        };
                        actions.clear();
                        return; // the capture flow owns input from here
                    }
                    Some(ControlsRow::System(verb)) => {
                        *capture = match *segment {
                            ControlsSegment::Keyboard => CaptureState::SystemKey {
                                verb,
                                refused: None,
                            },
                            ControlsSegment::Midi => CaptureState::SystemMidi {
                                verb,
                                refused: None,
                            },
                        };
                        actions.clear();
                        return;
                    }
                    None => {}
                }
            }
            _ => {
                let (next_focus, next_segment) = reduce_controls_nav(*focus, *segment, action.verb);
                let entered_rows =
                    *focus != ControlsFocus::Rows && next_focus == ControlsFocus::Rows;
                if *focus != next_focus {
                    *focus = next_focus;
                }
                if *segment != next_segment {
                    *segment = next_segment;
                }
                let stale = !current_row(selected.0, system.0)
                    .is_some_and(|row| rows.contains(&row));
                if entered_rows && stale {
                    // Seed the row cursor so Enter/Backspace always target a row.
                    if let Some(ControlsRow::Channel(first)) = rows.first().copied() {
                        selected.0 = Some(first);
                        system.0 = None;
                    }
                }
            }
        }
    }
}
```

- [ ] Import the new capture state in `controls_panel.rs` — the `use super::bindings_capture::{CaptureState, SelectedChannel};` line (L14) is unchanged; `CaptureState::SystemKey` resolves through it. No import edit needed.

- [ ] Register the resource in `crates/gameplay-drums/src/editor/mod.rs` — add one line after `.init_resource::<controls_panel::ControlsFocus>()` (L126):

```rust
        .init_resource::<controls_panel::SelectedSystem>()
```

- [ ] Add the System row plumbing to `crates/gameplay-drums/src/editor/bindings_panel.rs`. First, the components — add after `BindCaptureStart` (L53):

```rust
/// One system-verb row in the System card.
#[derive(Component, Clone, Copy)]
pub struct BindSystemRow(pub dtx_input::SystemVerb);

/// The `×` remove button on a system chip: removes `source[index]` from `verb`.
#[derive(Component, Clone, Copy)]
pub struct BindSystemChipRemove {
    pub verb: dtx_input::SystemVerb,
    pub index: usize,
}

/// `+` on a system row: starts capturing a new source for `verb`.
#[derive(Component, Clone, Copy)]
pub struct BindSystemCaptureStart(pub dtx_input::SystemVerb);
```

- [ ] Add the row model + index helper after `segment_rows` (L246):

```rust
/// One system-verb row for the active segment.
pub struct SystemSegmentRow {
    pub verb: dtx_input::SystemVerb,
    pub chips: Vec<SegmentChip>,
    pub unbound: bool,
}

/// Index (into the verb's FULL, unfiltered source list) of the LAST source
/// belonging to `segment` — the target of a keyboard Backspace on a System row.
/// `None` = nothing to delete (Backspace no-ops).
pub(super) fn last_system_source_index(
    b: &InputBindings,
    verb: dtx_input::SystemVerb,
    segment: ControlsSegment,
) -> Option<usize> {
    b.system
        .get(&verb)?
        .iter()
        .rposition(|source| segment_matches(segment, source))
}

/// Rows for the System card in the active segment. `shared` is always false:
/// a source a lane owns can never be bound to a verb (`lane_owner` refuses it),
/// so a system chip is never shared with a lane.
pub fn system_segment_rows(b: &InputBindings, segment: ControlsSegment) -> Vec<SystemSegmentRow> {
    dtx_input::SYSTEM_VERBS
        .into_iter()
        .map(|verb| {
            let chips: Vec<SegmentChip> = b
                .system_sources(verb)
                .iter()
                .enumerate()
                .filter(|(_, source)| segment_matches(segment, source))
                .map(|(index, source)| SegmentChip {
                    source: *source,
                    label: source_label(source),
                    index,
                    shared: false,
                })
                .collect();
            let unbound = chips.is_empty();
            SystemSegmentRow {
                verb,
                chips,
                unbound,
            }
        })
        .collect()
}
```

- [ ] Spawn the System card. Add the call at the end of `spawn_bindings_block`'s closure (L262-268) — the signature does not change (selection tint is applied live by `highlight_selected_system_row`):

```rust
    commands.entity(root).with_children(|p| {
        spawn_segment_selector(p, t, segment, focus, reset);
        if segment == ControlsSegment::Midi {
            spawn_device_card(p, t, live, ports);
        }
        spawn_pads_card(p, t, live, lanes, segment, selected);
        spawn_system_card(p, t, live, segment);
    });
```

- [ ] Add `spawn_system_card` after `spawn_pads_card` (L696):

```rust
/// System card: one row per bindable system verb (Pause, Restart), with the
/// same segment-filtered chips / `×` / `+` grammar as a lane row. Unbound by
/// default — Escape keeps working, and note maps vary by brand.
fn spawn_system_card(
    p: &mut ChildSpawnerCommands,
    t: &dtx_ui::theme::Theme,
    live: &LiveBindings,
    segment: ControlsSegment,
) {
    let body = panel_kit::spawn_card(p, "System");
    let rows = system_segment_rows(&live.0, segment);
    p.commands_mut().entity(body).with_children(|card| {
        for row in rows {
            let verb = row.verb;
            let unbound = row.unbound;
            let mut row_cmds = card.spawn((
                BindSystemRow(verb),
                Button,
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    padding: UiRect::axes(Val::Px(4.0), Val::Px(3.0)),
                    border: UiRect::left(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                },
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
                // Zero-width baseline; `highlight_selected_system_row` widens it
                // to the FOCUS_RING while keyboard focus sits on the rows.
                Outline::new(Val::Px(0.0), Val::Px(1.0), Color::NONE),
            ));
            if unbound {
                row_cmds.insert(UnboundRow);
            }
            row_cmds.with_children(|r| {
                r.spawn((
                    Text::new(verb.label()),
                    dtx_ui::theme::Theme::font(11.0),
                    TextColor(t.text_primary),
                    Node {
                        min_width: Val::Px(60.0),
                        ..default()
                    },
                ));
                for chip in &row.chips {
                    let chip_id = panel_kit::spawn_chip(r, &chip.label, false, ());
                    r.commands_mut().entity(chip_id).with_children(|cc| {
                        cc.spawn((
                            BindSystemChipRemove {
                                verb,
                                index: chip.index,
                            },
                            Button,
                            Pickable {
                                should_block_lower: false,
                                is_hoverable: true,
                            },
                            Node {
                                padding: UiRect::axes(Val::Px(3.0), Val::Px(0.0)),
                                margin: UiRect::left(Val::Px(2.0)),
                                ..default()
                            },
                            children![(
                                Text::new("\u{00d7}"),
                                dtx_ui::theme::Theme::font(11.0),
                                TextColor(chrome::TEXT_MUTED),
                            )],
                        ));
                    });
                }
                if unbound {
                    r.spawn((
                        Text::new("unbound"),
                        dtx_ui::theme::Theme::font(10.0),
                        TextColor(chrome::TEXT_MUTED),
                    ));
                }
                r.spawn((
                    BindSystemCaptureStart(verb),
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(5.0), Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(chrome::CHIP_BG),
                    children![(
                        Text::new("+"),
                        dtx_ui::theme::Theme::font(11.0),
                        TextColor(t.text_primary),
                    )],
                ));
            });
        }
    });
}
```

- [ ] Add the two mouse handlers after `handle_capture_start` (L825):

```rust
/// The `×` button on a system chip: drop that source from the verb's list.
fn handle_system_chip_remove(
    q: Query<(&Interaction, &BindSystemChipRemove), Changed<Interaction>>,
    mut live: ResMut<LiveBindings>,
    mut rev: ResMut<BindingsRev>,
) {
    for (interaction, chip) in &q {
        if *interaction == Interaction::Pressed {
            if let Some(sources) = live.0.system.get_mut(&chip.verb) {
                if chip.index < sources.len() {
                    sources.remove(chip.index);
                    rev.0 = rev.0.wrapping_add(1);
                }
            }
        }
    }
}

/// `+` on a system row: arm segment-specific capture for that verb.
fn handle_system_capture_start(
    q: Query<(&Interaction, &BindSystemCaptureStart), Changed<Interaction>>,
    segment: Res<ControlsSegment>,
    mut capture: ResMut<CaptureState>,
) {
    for (interaction, start) in &q {
        if *interaction == Interaction::Pressed {
            *capture = match *segment {
                ControlsSegment::Keyboard => CaptureState::SystemKey {
                    verb: start.0,
                    refused: None,
                },
                ControlsSegment::Midi => CaptureState::SystemMidi {
                    verb: start.0,
                    refused: None,
                },
            };
        }
    }
}
```

- [ ] Register them in `bindings_panel::plugin` — add both to the existing tuple (L121-133), which becomes:

```rust
        .add_systems(
            Update,
            (
                handle_velocity_adjust,
                handle_bind_chip_remove,
                handle_capture_start,
                handle_system_chip_remove,
                handle_system_capture_start,
                handle_port_cycle,
                handle_rescan,
                handle_bindings_reset,
                update_velocity_meter,
                update_chip_hover_highlight,
            )
                .run_if(in_state(game_shell::AppState::Performance))
                .run_if(super::editor_open),
        )
```

- [ ] Add the system-row highlight in `crates/gameplay-drums/src/editor/bindings_capture.rs`, after `highlight_selected_row` (L579):

```rust
/// Tint the selected System row and, while keyboard focus sits on the rows,
/// ring it — the mirror of `highlight_selected_row` for the System card. Runs
/// every frame, so no repaint plumbing is needed.
fn highlight_selected_system_row(
    system: Res<super::controls_panel::SelectedSystem>,
    focus: Res<super::controls_panel::ControlsFocus>,
    mut rows: Query<(
        &super::bindings_panel::BindSystemRow,
        Has<super::bindings_panel::UnboundRow>,
        &mut BackgroundColor,
        &mut BorderColor,
        &mut Outline,
    )>,
) {
    let rows_focused = *focus == super::controls_panel::ControlsFocus::Rows;
    for (row, unbound, mut bg, mut border, mut outline) in &mut rows {
        let on = system.0 == Some(row.0);
        *bg = BackgroundColor(if on {
            super::chrome::ROW_SELECTED_BG
        } else if unbound {
            super::chrome::WARN_TINT
        } else {
            Color::NONE
        });
        *border = BorderColor::all(if on { super::chrome::ACCENT } else { Color::NONE });
        if on && rows_focused {
            outline.width = Val::Px(2.0);
            outline.color = super::panel::FOCUS_RING;
        } else {
            outline.width = Val::Px(0.0);
            outline.color = Color::NONE;
        }
    }
}
```

- [ ] Register it in `bindings_capture::plugin` — add `highlight_selected_system_row` to the tuple (L131-140), after `highlight_selected_row`.

- [ ] Add the panel-level tests to the `tests` module in `crates/gameplay-drums/src/editor/bindings_panel.rs`:

```rust
    #[test]
    fn system_rows_are_segment_filtered_and_flag_unbound() {
        use dtx_input::{BindSource, InputBindings, SystemVerb};

        let mut b = InputBindings::default();
        b.bind_system(SystemVerb::Pause, BindSource::Midi { note: 37 });
        b.bind_system(SystemVerb::Pause, BindSource::Key(KeyCode::F9));

        let midi_rows = system_segment_rows(&b, ControlsSegment::Midi);
        let pause = midi_rows
            .iter()
            .find(|r| r.verb == SystemVerb::Pause)
            .expect("Pause row exists");
        assert_eq!(pause.chips.len(), 1);
        assert_eq!(pause.chips[0].label, "N37");
        assert_eq!(pause.chips[0].index, 0, "index into the FULL source list");
        assert!(!pause.chips[0].shared, "a verb source is never a lane's");

        let restart = midi_rows
            .iter()
            .find(|r| r.verb == SystemVerb::Restart)
            .expect("Restart row exists");
        assert!(restart.unbound, "unbound by default");

        let kb_rows = system_segment_rows(&b, ControlsSegment::Keyboard);
        let pause = kb_rows
            .iter()
            .find(|r| r.verb == SystemVerb::Pause)
            .expect("Pause row exists");
        assert_eq!(pause.chips.len(), 1);
        assert_eq!(pause.chips[0].index, 1, "full-list index, not filtered");
    }

    #[test]
    fn last_system_source_index_picks_last_in_segment() {
        use dtx_input::{BindSource, InputBindings, SystemVerb};

        let mut b = InputBindings::default();
        b.bind_system(SystemVerb::Pause, BindSource::Key(KeyCode::F9));
        b.bind_system(SystemVerb::Pause, BindSource::Midi { note: 37 });
        b.bind_system(SystemVerb::Pause, BindSource::Midi { note: 53 });

        assert_eq!(
            last_system_source_index(&b, SystemVerb::Pause, ControlsSegment::Midi),
            Some(2)
        );
        assert_eq!(
            last_system_source_index(&b, SystemVerb::Pause, ControlsSegment::Keyboard),
            Some(0)
        );
        assert_eq!(
            last_system_source_index(&b, SystemVerb::Restart, ControlsSegment::Midi),
            None,
            "unbound verb: Backspace no-ops"
        );
    }
```

- [ ] Add the consumer test to the `tests` module in `crates/gameplay-drums/src/editor/controls_panel.rs`:

```rust
    #[test]
    fn enter_on_a_system_row_arms_system_capture() {
        use crate::editor::bindings_capture::{CaptureState, SelectedChannel};
        use crate::editor::bindings_panel::BindingsRev;
        use bevy::prelude::*;
        use dtx_input::SystemVerb;
        use game_shell::{NavAction, NavSource};

        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ControlsFocus>()
            .init_resource::<ControlsSegment>()
            .init_resource::<CaptureState>()
            .init_resource::<SelectedChannel>()
            .init_resource::<SelectedSystem>()
            .init_resource::<BindingsRev>()
            .init_resource::<crate::bindings::LiveBindings>()
            .init_resource::<crate::lanes::Lanes>()
            .insert_resource(crate::editor::tabs::ActiveTab(
                game_shell::CustomizeTab::Controls,
            ))
            .add_message::<NavAction>()
            .add_systems(Update, controls_nav_consumer);
        app.update();

        fn nav(app: &mut App, verb: NavVerb) {
            app.world_mut()
                .resource_mut::<Messages<NavAction>>()
                .write(NavAction {
                    verb,
                    source: NavSource::Keyboard,
                    coarse: false,
                });
            app.update();
        }

        // TabBar → SegmentSelector → Rows (cursor seeds on the first lane row).
        nav(&mut app, NavVerb::Down);
        nav(&mut app, NavVerb::Down);
        // Walk past the twelve lane rows onto the Pause row.
        for _ in 0..dtx_input::BINDABLE_CHANNELS.len() {
            nav(&mut app, NavVerb::Down);
        }
        assert_eq!(
            app.world().resource::<SelectedSystem>().0,
            Some(SystemVerb::Pause),
            "the cursor walks off the lanes into the System rows"
        );

        nav(&mut app, NavVerb::Confirm);
        assert!(matches!(
            *app.world().resource::<CaptureState>(),
            CaptureState::SystemKey {
                verb: SystemVerb::Pause,
                refused: None
            }
        ));
    }
```

- [ ] Run `cargo test -p gameplay-drums --lib` — expect all tests to pass, including `pad_exclusion_matches_controls_contract` (untouched).

- [ ] Commit:

```sh
git add crates/gameplay-drums/src/editor/controls_panel.rs crates/gameplay-drums/src/editor/bindings_panel.rs crates/gameplay-drums/src/editor/bindings_capture.rs crates/gameplay-drums/src/editor/mod.rs
git commit -m "feat(customize): add the Controls-tab System section for Pause and Restart"
```

---

## Task 12 — Full-workspace gate + runtime smoke

**Files:** none (verification only).

- [ ] Run `cargo check --workspace` — expect `Finished` with no errors.

- [ ] Run `cargo clippy --workspace --all-targets -- -D warnings` — expect no warnings. If clippy flags `too_many_arguments` on `controls_nav_consumer`, the `#[allow(clippy::too_many_arguments)]` attribute from Task 11 already covers it; `gameplay-drums` also allows it crate-wide (`lib.rs:18`).

- [ ] Run the package tests:

```sh
cargo test -p dtx-input
cargo test -p gameplay-drums --lib
cargo test -p game-menu --lib
```
Expect all green.

- [ ] Grep for the forbidden call: `rg '\.unwrap\(\)' crates/dtx-input/src crates/gameplay-drums/src crates/game-menu/src --glob '!*test*'` — any hit in code you added is a bug; only pre-existing `#[cfg(test)]` blocks may match.

- [ ] **BRP runtime smoke is mandatory, not optional** (spec §Testing — the last three streams each shipped a bug the whole unit suite passed straight through). Launch the game, open Customize → Controls → MIDI, focus the **System / Pause** row, press Enter, hit a spare zone note (xstick 37 / ride bell 53), Ctrl+S to save. Then:
  1. Start a song and hit the bound pad **mid-play** → the pause overlay opens.
  2. From the pads alone, navigate to **Quit to Song Select** (HH/CY up-down, BD confirm) and quit.
  3. Re-enter the song; during the loading screen hit **SD** → the load cancels back to song select.
  4. Re-open Customize → Controls → MIDI → System / Pause → Enter → hit the **snare** → the bind is refused in place, naming SD; nothing is bound.

- [ ] Commit nothing (verification task). If any gate failed, fix it in the owning task's file and amend that task's commit.

---

## Self-review

### Spec coverage

| Spec requirement | Task |
|---|---|
| `SystemVerb { Pause, Restart }` enum | 1 |
| `InputBindings.system` parallel map, `BindSource` reused unchanged | 1 |
| `BindingsFile` `[system]` table, `serde(default)`, **no version bump, no migration** | 1 |
| Round-trip: old `bindings.toml` with no `[system]` loads with an empty map | 1 (`old_file_without_system_table_loads_empty_system_map`) |
| `lane_owner` — one pure function, single source of truth for the rule | 1 (impl) + 2 (tests) |
| Default binding is **unbound**; Escape keeps working | 1 (`Default` leaves `system` empty); `toggle_pause` untouched (7) |
| `BindResolver.note_to_system` / `key_to_system`, built in **both** `from_bindings` and `from_profiles` | 4 |
| Resolver **skips** a colliding system source and `warn!`s — footgun closed at the resolver, not just the UI | 4 |
| Lane binds never refused; lanes win ties; rule is one-directional | 2 (`lane_owner_ignores_system_binds_lanes_win_ties`) + 4 |
| `SystemVerbHit { verb }` message registered in the drums plugin, emitted from `DrumsSets::Input` | 5 |
| Emitted **before** the `gameplay_ready` gate, mirroring `PadNavHit` | 5 (`system_verb_fires_while_gameplay_is_not_ready`) |
| Velocity threshold applies to system notes | 5 (`sub_threshold_system_note_emits_nothing`) |
| Keyboard-bound verbs emitted on the same message | 6 |
| `Pause` consumer: toggles both directions, `PracticePauseSurface::Overlay` before pausing, `in_state(Performance)` + `editor_closed` | 7 |
| F3 (quit from the kit) falls out of F1 — no separate binding | 7 (the overlay already carries pad grammar + Quit) |
| `Restart` consumer: `request_transition(SongLoading)` like the Retry row; fires running **or** paused | 8 |
| F4: `NavContext::Loading` + `AppState::SongLoading` arm | 9 |
| F4: `watch_cancel_key` gains a `MessageReader<NavAction>`, cancels on `NavVerb::Back` (SD) | 9 |
| Controls tab **System** section: row per verb, keyboard + MIDI segments, Enter to capture, Backspace to remove last source | 11 |
| Capture refuses a colliding note **in place**, naming the owning lane | 10 |
| Pads stay excluded from Controls-tab nav; `pad_excluded` + its test untouched | 11 (explicit non-goal, restated in the task) |
| No gesture detection, no `NoteOff` consumption, no CC plumbing | Nothing in this plan touches `midi.rs` |
| BRP runtime smoke, mandatory | 12 |

### Gaps and deviations

1. **Persistence (Task 3) is not in the spec.** The spec's data model predates the profile-registry split; without `KeyboardProfile.system` / `MidiProfile.system`, a system bind dies at the next `reload_profiles`. Called out at the top of this plan. No behavior in the spec is changed by it — it is the plumbing the spec assumed already existed.
2. **`menu_nav`'s existing `no_context_during_live_play_or_capture` test asserts `SongLoading → None`.** Task 9 updates that assertion. This is intentional and is the *only* existing assertion this plan inverts.
3. **The `Restart` mis-hit risk is accepted, not mitigated** (spec's named risk, restated in Task 8). If it proves annoying, gate it to `PauseState::Paused`.
4. **No default bind ships.** A user with no MIDI kit sees two unbound System rows. That is the spec's decision ("note maps vary by brand — we do not guess").
5. **The system-verb keyboard path is not gated on `EGameMode::Drums`** (unlike the lane path). Guitar mode has no system binds, so the gate would be dead code; a `Pause` bind working in either mode is the desirable behavior anyway.

### Type consistency check

`SystemVerb`, `SYSTEM_VERBS`, `lane_owner`, `InputBindings::bind_system`, `InputBindings::system_sources`, `SystemVerb::key`/`from_key`/`label` are defined in Task 1 and used under exactly those names in Tasks 2, 3, 4, 5, 10, 11.
`BindResolver::system_for_note` / `system_for_key` — defined Task 4, used Tasks 5 and 6.
`crate::events::SystemVerbHit { verb }` — defined Task 5, read in Tasks 6, 7, 8.
`KeyboardProfile::add_system_key` / `MidiProfile::bind_system_note` — defined Task 3, used in Tasks 3 (`split_bindings`) and 4 (tests).
`CaptureState::SystemKey { verb, refused }` / `SystemMidi { verb, refused }` and `system_capture_step` / `SystemCaptureStep` — defined Task 10, used in Tasks 10 and 11.
`ControlsRow`, `SelectedSystem`, `controls_rows`, `current_row`, `step_row`, `RowStep<T>` — defined Task 11, used only in Task 11.
`bindings_panel::last_system_source_index` / `system_segment_rows` / `BindSystemRow` — defined Task 11, used in Task 11 (`controls_nav_consumer`, `highlight_selected_system_row`).
No name appears under two spellings.
