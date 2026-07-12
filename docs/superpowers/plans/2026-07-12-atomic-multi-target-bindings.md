# Atomic Multi-Target Bindings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace shared-source fan-out with ordered multi-target bindings that emit one physical input event and judge at most one chip.

**Architecture:** `dtx-input` stores each keyboard key or MIDI note once with a non-empty ordered target list. `gameplay-drums` resolves the physical source to one `InputHit`; the judge sends single-target hits through the existing BocuD group resolver and multi-target hits through a one-result explicit resolver. Lane profiles remain display-only.

**Tech Stack:** Rust 2024, Bevy 0.19 messages and schedules, Serde/TOML, `dtx-persistence::replace_bytes`, Cargo tests.

## Global Constraints

- Read `crates/dtx-input/AGENTS.md`, `crates/gameplay-drums/AGENTS.md`, and the reference excerpts before implementation.
- Preserve ordinary BocuD drum-group behavior. The one-chip limit applies only to explicit multi-target bindings.
- Cite `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/CStagePerfDrumsScreen.cs:L972` and the exact tie-handling lines used when committing judgment changes.
- Keep `references/` read-only.
- Do not add `unwrap()` under `crates/*`.
- Use one plugin function per Bevy module.
- Do not change scoring windows, score formulas, global drum-group settings, or lane-layout semantics.
- Bevy 0.19 requires Rust 1.95 or newer.
- Do not include AI co-author trailers, secrets, tokens, or local configuration in commits.

---

## File map

| File | Responsibility after this change |
|---|---|
| `crates/dtx-input/src/targets.rs` | Validated non-empty ordered channel targets and source-centric binding record. |
| `crates/dtx-input/src/profiles.rs` | Keyboard/MIDI source maps, profile editing, v1 registry conversion, v2 persistence. |
| `crates/dtx-input/src/bindings.rs` | Legacy `bindings.toml` reader used only for migration; conversion into source-centric profiles. |
| `crates/dtx-input/src/lib.rs` | Public exports for target types. |
| `crates/gameplay-drums/src/events.rs` | Atomic `InputHit` message plus existing chart-derived `LaneHit` for autoplay if retained. |
| `crates/gameplay-drums/src/bindings.rs` | Key/note to ordered targets lookup and profile composition for the editor. |
| `crates/gameplay-drums/src/input.rs` | Keyboard capture and one-event emission. |
| `crates/gameplay-drums/src/lib.rs` | MIDI consumer and message registration. |
| `crates/gameplay-drums/src/drum_groups.rs` | Pure explicit-target resolver that returns zero or one candidate. |
| `crates/gameplay-drums/src/judge.rs` | Route atomic input by target count; filter wait-mode candidates before consumption. |
| `crates/gameplay-drums/src/hit_feedback.rs` | One primary flash for physical input; judged-channel feedback after success. |
| `crates/gameplay-drums/src/editor/bindings_capture.rs` | Set-primary, add-alternate, remove-target, and move-only reducers. |
| `crates/gameplay-drums/src/editor/bindings_panel.rs` | Primary/alternate labels, `1x` badge, channel-row inverted view. |
| `crates/gameplay-drums/src/editor/lanes_panel.rs` | “Displayed channels” copy only; no binding mutation. |
| `crates/gameplay-drums/tests/atomic_multi_target_bindings.rs` | End-to-end input, judgment, and layout-independence regression tests. |

---

### Task 1: Add validated ordered targets and source-centric profile APIs

**Files:**
- Create: `crates/dtx-input/src/targets.rs`
- Modify: `crates/dtx-input/src/lib.rs`
- Modify: `crates/dtx-input/src/profiles.rs`

**Interfaces:**
- Produces: `TargetList::new(primary, alternates) -> Result<TargetList, TargetListError>`
- Produces: `TargetList::{primary, as_slice, add_alternate, set_primary, remove}`
- Produces: `KeyboardProfile::targets_for(KeyCode) -> Option<&TargetList>`
- Produces: `MidiProfile::targets_for(u8) -> Option<&TargetList>`
- Produces: source editing methods used by the Customize reducers.

- [ ] **Step 1: Write failing `TargetList` tests**

Add `targets.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::EChannel::{BassDrum, LeftBassDrum, Snare};

    #[test]
    fn rejects_empty_and_duplicate_targets() {
        assert_eq!(TargetList::try_from(Vec::new()), Err(TargetListError::Empty));
        assert_eq!(
            TargetList::try_from(vec![BassDrum, BassDrum]),
            Err(TargetListError::Duplicate(BassDrum)),
        );
    }

    #[test]
    fn primary_is_first_and_promotes_first_alternate() {
        let mut targets = TargetList::new(BassDrum, [LeftBassDrum, Snare]).expect("valid targets");
        assert_eq!(targets.primary(), BassDrum);
        assert!(targets.remove(BassDrum));
        assert_eq!(targets.as_slice(), &[LeftBassDrum, Snare]);
    }

    #[test]
    fn set_primary_moves_existing_target_without_duplication() {
        let mut targets = TargetList::new(BassDrum, [LeftBassDrum]).expect("valid targets");
        targets.set_primary(LeftBassDrum);
        assert_eq!(targets.as_slice(), &[LeftBassDrum, BassDrum]);
    }
}
```

- [ ] **Step 2: Run the focused test and verify failure**

Run: `cargo test -p dtx-input targets::tests --lib`

Expected: compile failure because `TargetList` and `TargetListError` do not exist.

- [ ] **Step 3: Implement the validated target type**

Add above the tests in `targets.rs`:

```rust
use dtx_core::EChannel;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::bindings::BINDABLE_CHANNELS;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TargetListError {
    #[error("a binding must contain at least one target")]
    Empty,
    #[error("duplicate binding target: {0:?}")]
    Duplicate(EChannel),
    #[error("channel cannot be bound: {0:?}")]
    Unbindable(EChannel),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetList(Vec<EChannel>);

impl TargetList {
    pub fn new(
        primary: EChannel,
        alternates: impl IntoIterator<Item = EChannel>,
    ) -> Result<Self, TargetListError> {
        let mut values = vec![primary];
        values.extend(alternates);
        Self::try_from(values)
    }

    pub fn primary(&self) -> EChannel { self.0[0] }
    pub fn as_slice(&self) -> &[EChannel] { &self.0 }
    pub fn is_multi_target(&self) -> bool { self.0.len() > 1 }

    pub fn add_alternate(&mut self, channel: EChannel) -> Result<bool, TargetListError> {
        if !BINDABLE_CHANNELS.contains(&channel) {
            return Err(TargetListError::Unbindable(channel));
        }
        if self.0.contains(&channel) { return Ok(false); }
        self.0.push(channel);
        Ok(true)
    }

    pub fn set_primary(&mut self, channel: EChannel) {
        self.0.retain(|target| *target != channel);
        self.0.insert(0, channel);
    }

    pub fn remove(&mut self, channel: EChannel) -> bool {
        if self.0.len() == 1 && self.0[0] == channel { return false; }
        let before = self.0.len();
        self.0.retain(|target| *target != channel);
        self.0.len() != before
    }
}

impl TryFrom<Vec<EChannel>> for TargetList {
    type Error = TargetListError;

    fn try_from(values: Vec<EChannel>) -> Result<Self, Self::Error> {
        if values.is_empty() { return Err(TargetListError::Empty); }
        let mut seen = Vec::new();
        for channel in &values {
            if !BINDABLE_CHANNELS.contains(channel) {
                return Err(TargetListError::Unbindable(*channel));
            }
            if seen.contains(channel) { return Err(TargetListError::Duplicate(*channel)); }
            seen.push(*channel);
        }
        Ok(Self(values))
    }
}

impl Serialize for TargetList {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        let names: Vec<&str> = self.0.iter().filter_map(|channel| channel.short_name()).collect();
        names.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TargetList {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de> {
        let names = Vec::<String>::deserialize(deserializer)?;
        let channels = names.into_iter().map(|name| {
            EChannel::from_short_name(&name)
                .ok_or_else(|| serde::de::Error::custom(format!("unknown binding target {name:?}")))
        }).collect::<Result<Vec<_>, _>>()?;
        Self::try_from(channels).map_err(serde::de::Error::custom)
    }
}
```

Export it from `lib.rs`:

```rust
pub mod targets;
pub use targets::{TargetList, TargetListError};
```

- [ ] **Step 4: Convert profile structs and add edit API tests**

Replace channel-keyed profile maps with:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyboardBinding { pub source: KeyCode, pub targets: TargetList }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MidiBinding { pub source: u8, pub targets: TargetList }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyboardProfile {
    #[serde(default)]
    pub bindings: Vec<KeyboardBinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MidiProfile {
    pub port: Option<String>,
    pub velocity_threshold: u8,
    #[serde(default)]
    pub bindings: Vec<MidiBinding>,
}
```

Add methods and tests for both source types:

```rust
impl KeyboardProfile {
    pub fn targets_for(&self, key: KeyCode) -> Option<&TargetList> {
        self.bindings.iter().find(|binding| binding.source == key).map(|binding| &binding.targets)
    }
    pub fn set_primary(&mut self, key: KeyCode, channel: EChannel) {
        if let Some(binding) = self.bindings.iter_mut().find(|binding| binding.source == key) {
            binding.targets.set_primary(channel);
        } else {
            self.bindings.push(KeyboardBinding {
                source: key,
                targets: TargetList::new(channel, []).expect("bindable channel"),
            });
        }
    }
    pub fn add_alternate(&mut self, key: KeyCode, channel: EChannel) -> Result<bool, TargetListError> {
        match self.bindings.iter_mut().find(|binding| binding.source == key) {
            Some(binding) => binding.targets.add_alternate(channel),
            None => { self.set_primary(key, channel); Ok(true) }
        }
    }
}
```

Implement the same methods for `MidiProfile` with `note: u8`. Test that adding LBD to Space produces `[BD, LBD]`, changing primary produces `[LBD, BD]`, and removing the last target removes the source record through a profile-level `remove_target` method.

- [ ] **Step 5: Run dtx-input library tests**

Run: `cargo test -p dtx-input --lib`

Expected: all tests pass after updating existing profile/default tests to assert source-centric records.

- [ ] **Step 6: Commit the target model**

```bash
git add crates/dtx-input/src/targets.rs crates/dtx-input/src/lib.rs crates/dtx-input/src/profiles.rs
git commit -m "refactor(input): model ordered binding targets"
```

---

### Task 2: Add v1-to-v2 profile migration without load-time writes

**Files:**
- Modify: `crates/dtx-input/src/profiles.rs`
- Modify: `crates/dtx-input/src/bindings.rs`
- Test: `crates/dtx-input/src/profiles.rs`
- Test: `crates/gameplay-drums/tests/input_lane_profiles.rs`

**Interfaces:**
- Consumes: `TargetList`, source-centric `KeyboardProfile` and `MidiProfile` from Task 1.
- Produces: `PROFILE_REGISTRY_VERSION == 2`.
- Produces: `RegistryStartup::MigratedSession { registry }` with no write attempt.
- Produces: `keyboard_from_channel_map` and `midi_from_channel_map` using `BINDABLE_CHANNELS` as priority order.

- [ ] **Step 1: Write failing migration tests**

Add tests that write these v1 registries to a temp directory:

```toml
version = 1
active = "Pedals"

[profiles.Pedals.map]
BD = ["Space"]
LBD = ["Space"]
```

and:

```toml
version = 1
active = "Pedals"

[profiles.Pedals]
velocity_threshold = 10

[profiles.Pedals.map]
BD = [36]
LBD = [36]
```

Assert:

```rust
let RegistryStartup::MigratedSession { registry } = startup else {
    panic!("v1 registry should migrate in memory");
};
let targets = registry.profiles["Pedals"].targets_for(KeyCode::Space).expect("Space migrated");
assert_eq!(targets.as_slice(), &[EChannel::BassDrum, EChannel::LeftBassDrum]);
assert_eq!(std::fs::read_to_string(&path).expect("v1 file remains"), original);
```

- [ ] **Step 2: Run migration tests and verify failure**

Run: `cargo test -p dtx-input profiles::tests::v1 --lib`

Expected: failure because version 1 is unsupported and `MigratedSession` does not exist.

- [ ] **Step 3: Add explicit v1 DTOs and canonical conversion**

Keep the v1 DTO private:

```rust
#[derive(Deserialize)]
struct V1KeyboardProfile { #[serde(default)] map: BTreeMap<String, Vec<KeyCode>> }

#[derive(Deserialize)]
struct V1MidiProfile {
    port: Option<String>,
    #[serde(default)] velocity_threshold: u8,
    #[serde(default)] map: BTreeMap<String, Vec<u8>>,
}

#[derive(Deserialize)]
struct V1Registry<T> {
    version: u32,
    active: String,
    #[serde(default)] profiles: BTreeMap<String, T>,
}
```

Convert by iterating `BINDABLE_CHANNELS`, then each source under that channel. The first canonical channel becomes primary:

```rust
fn keyboard_from_channel_map(map: &BTreeMap<String, Vec<KeyCode>>) -> Result<KeyboardProfile, String> {
    let mut profile = KeyboardProfile { bindings: Vec::new() };
    for channel in BINDABLE_CHANNELS {
        let Some(name) = channel.short_name() else { continue };
        for key in map.get(name).into_iter().flatten() {
            match profile.bindings.iter_mut().find(|binding| binding.source == *key) {
                Some(binding) => { binding.targets.add_alternate(channel).map_err(|e| e.to_string())?; }
                None => { profile.bindings.push(KeyboardBinding {
                    source: *key,
                    targets: TargetList::new(channel, []).map_err(|e| e.to_string())?,
                }); }
            }
        }
    }
    Ok(profile)
}
```

Implement the note equivalent and reject unknown v1 channel names instead of dropping them.

- [ ] **Step 4: Dispatch registry loading by the raw version field**

Parse a header before the typed registry:

```rust
#[derive(Deserialize)]
struct VersionHeader { #[serde(default)] version: u32 }
```

For version 2, deserialize `ProfileRegistry<T>` and validate it. For version 1, deserialize the matching v1 DTO, convert each profile, set `version: 2`, validate, and return `RegistryStartup::MigratedSession`. For any other version, return `UnsupportedVersion`. Remove the current migration write attempt for profile registries.

- [ ] **Step 5: Make writes validate source records and write only v2**

Set `PROFILE_REGISTRY_VERSION` to `2`. Before `replace_bytes`, validate every profile target list, reject duplicate source records, and validate every MIDI note range. Keep `bindings.toml` version 1 readable only as the older combined-profile migration source; do not rewrite it.

- [ ] **Step 6: Run migration and package tests**

Run:

```bash
cargo test -p dtx-input --lib
cargo test -p gameplay-drums --test input_lane_profiles
```

Expected: both commands pass; the integration test confirms migrated files retain their original bytes until Save.

- [ ] **Step 7: Commit migration**

```bash
git add crates/dtx-input/src/profiles.rs crates/dtx-input/src/bindings.rs crates/gameplay-drums/tests/input_lane_profiles.rs
git commit -m "feat(input): migrate profiles to ordered targets"
```

---

### Task 3: Emit one atomic event from keyboard and MIDI

**Files:**
- Modify: `crates/gameplay-drums/src/events.rs`
- Modify: `crates/gameplay-drums/src/bindings.rs`
- Modify: `crates/gameplay-drums/src/input.rs`
- Modify: `crates/gameplay-drums/src/lib.rs`
- Modify: `crates/gameplay-drums/src/autoplay.rs`
- Modify: `crates/gameplay-drums/src/hit_feedback.rs`

**Interfaces:**
- Consumes: `TargetList` and source-centric profiles.
- Produces: `InputHit { targets: TargetList, audio_ms: i64, source: InputSourceKind }`.
- Produces: `BindResolver::{targets_for_key, targets_for_note}` returning cloned `TargetList` values.
- Keeps: chart-derived `LaneHit` for autoplay, or converts autoplay to a one-target `InputHit` without changing its judgment behavior.

- [ ] **Step 1: Write failing resolver and keyboard capture tests**

Add a resolver test:

```rust
#[test]
fn shared_key_resolves_to_one_ordered_target_list() {
    let mut keyboard = KeyboardProfile::default();
    keyboard.set_primary(KeyCode::Space, EChannel::BassDrum);
    keyboard.add_alternate(KeyCode::Space, EChannel::LeftBassDrum).expect("alternate");
    let resolver = BindResolver::from_profiles(&keyboard, &MidiProfile::default());
    assert_eq!(
        resolver.targets_for_key(KeyCode::Space).expect("binding").as_slice(),
        &[EChannel::BassDrum, EChannel::LeftBassDrum],
    );
}
```

Add a pure helper test in `input.rs` that passes one pressed key and asserts one captured record with two ordered targets.

- [ ] **Step 2: Run focused tests and verify failure**

Run: `cargo test -p gameplay-drums bindings::tests::shared_key input::tests --lib`

Expected: compile failure because the target resolver and atomic capture record do not exist.

- [ ] **Step 3: Add the atomic event and resolver lookup**

In `events.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputSourceKind { Keyboard, Midi, Autoplay }

#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct InputHit {
    pub targets: dtx_input::TargetList,
    pub audio_ms: i64,
    pub source: InputSourceKind,
}
```

Change `BindResolver` maps to `HashMap<KeyCode, TargetList>` and `HashMap<u8, TargetList>`. Construct them by cloning each profile’s source-centric record. Replace `lanes_for_key` and `lanes_for_note` with:

```rust
pub fn targets_for_key(&self, key: KeyCode) -> Option<TargetList> {
    self.key_to_targets.get(&key).cloned()
}

pub fn targets_for_note(&self, note: u8) -> Option<TargetList> {
    self.note_to_targets.get(&note).cloned()
}
```

- [ ] **Step 4: Emit one keyboard event**

Store `TargetList` in `CapturedInput`. Keep the existing `Instant` compensation. Replace the lane fan-out loop with one lookup and one queued record:

```rust
for key in keys.get_just_pressed() {
    if let Some(targets) = resolver.targets_for_key(*key) {
        pending.events.push(CapturedInput { targets, captured_at: Instant::now() });
    }
}
```

`emit_pending_inputs` writes one `InputHit` per record with `InputSourceKind::Keyboard`.

- [ ] **Step 5: Emit one MIDI event**

In `midi_consumer`, replace `for lane in resolver.lanes_for_note(note)` with one `targets_for_note(note)` lookup. Preserve velocity threshold, stale NoteOn filtering, timestamp compensation, and the navigation event. Write one `InputHit` with `InputSourceKind::Midi`.

- [ ] **Step 6: Keep autoplay semantics explicit**

Have autoplay build `TargetList::new(chip.channel, [])` and emit `InputHit { source: Autoplay, ... }`. Its target count stays one, so the judge follows the existing group path. Remove `LaneHit` registration only after all consumers use `InputHit`.

- [ ] **Step 7: Update immediate feedback to flash the primary once**

Change `hit_feedback` and keyboard visualization readers to read `InputHit` and derive the primary lane with `lane_of(hit.targets.primary())`. Do not iterate alternates.

- [ ] **Step 8: Run gameplay-drums library tests**

Run: `cargo test -p gameplay-drums --lib`

Expected: all tests pass; resolver tests assert one event with ordered targets for keyboard and MIDI.

- [ ] **Step 9: Commit atomic emission**

```bash
git add crates/gameplay-drums/src/events.rs crates/gameplay-drums/src/bindings.rs crates/gameplay-drums/src/input.rs crates/gameplay-drums/src/lib.rs crates/gameplay-drums/src/autoplay.rs crates/gameplay-drums/src/hit_feedback.rs
git commit -m "refactor(drums): emit atomic physical inputs"
```

---

### Task 4: Resolve explicit targets to at most one chip

**Files:**
- Modify: `crates/gameplay-drums/src/drum_groups.rs`
- Modify: `crates/gameplay-drums/src/judge.rs`
- Modify: `crates/gameplay-drums/src/events.rs`
- Test: `crates/gameplay-drums/src/drum_groups.rs`
- Test: `crates/gameplay-drums/src/judge.rs`

**Interfaces:**
- Consumes: atomic `InputHit` from Task 3.
- Produces: `resolve_explicit_targets(...) -> Option<(usize, i64, EChannel)>`.
- Preserves: `resolve_judgments(DrumPad, ...) -> Vec<(usize, i64)>` for one-target events.

- [ ] **Step 1: Add failing pure resolver tests**

Build a chart with BD and LBD chips and assert:

```rust
let targets = TargetList::new(EChannel::BassDrum, [EChannel::LeftBassDrum]).expect("targets");
let result = resolve_explicit_targets(
    &targets,
    hit_ms,
    &chart,
    &HashSet::new(),
    base_bpm,
    timing,
    None,
);
assert_eq!(result.map(|(_, _, channel)| channel), Some(EChannel::BassDrum));
```

Cover four cases in separate tests: equal-time primary wins, earlier alternate wins, already-judged primary lets alternate win, and halted-set filtering removes a closer disallowed chip before selection.

- [ ] **Step 2: Run the focused resolver tests and verify failure**

Run: `cargo test -p gameplay-drums drum_groups::tests::explicit --lib`

Expected: compile failure because `resolve_explicit_targets` does not exist.

- [ ] **Step 3: Implement explicit resolution**

Reuse `closest_candidate`, then filter before selection:

```rust
pub fn resolve_explicit_targets(
    targets: &dtx_input::TargetList,
    audio_ms: i64,
    chart: &Chart,
    judged: &HashSet<usize>,
    base_bpm: f32,
    timing: ChartTiming<'_>,
    allowed: Option<&[usize]>,
) -> Option<(usize, i64, EChannel)> {
    targets.as_slice().iter().enumerate()
        .filter_map(|(priority, channel)| {
            closest_candidate(*channel, audio_ms, chart, judged, base_bpm, timing)
                .filter(|candidate| allowed.is_none_or(|indices| indices.contains(&candidate.idx)))
                .map(|candidate| (priority, candidate))
        })
        .min_by_key(|(priority, candidate)| (candidate.target_ms, *priority))
        .map(|(_, candidate)| (candidate.idx, candidate.delta, candidate.channel))
}
```

Do not call `pick_earliest`; it returns every exact-time tie and would recreate the bug.

- [ ] **Step 4: Route events in the judge**

For one target, map the primary channel to `DrumPad` and call the existing resolver, then apply the existing halted-set filter. For multiple targets, call `resolve_explicit_targets` with `halted_chips` and convert its `Option` into one result.

Extend `JudgmentEvent` and `EmptyHit` with the actual channel or derive its lane via `lane_of`. Successful feedback must use the consumed channel; empty feedback must use `targets.primary()`.

- [ ] **Step 5: Add the screenshot regression test**

In `judge.rs`, send one `[BD, LBD]` event at a timestamp containing simultaneous BD and LBD chips. Assert that `JudgedChips` contains only the BD index and that exactly one `JudgmentEvent` was written. Send a second physical event whose primary is LBD and assert both indices can then become judged.

- [ ] **Step 6: Run judgment tests**

Run:

```bash
cargo test -p gameplay-drums drum_groups::tests --lib
cargo test -p gameplay-drums judge::tests --lib
```

Expected: both commands pass, including existing group tie tests and new explicit one-result tests.

- [ ] **Step 7: Commit one-chip judgment**

```bash
git add crates/gameplay-drums/src/drum_groups.rs crates/gameplay-drums/src/judge.rs crates/gameplay-drums/src/events.rs
git commit -m "fix(drums): judge one chip per multi-target hit"
```

Commit body:

```text
Keep ordinary grouped-pad tie behavior from
references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/CStagePerfDrumsScreen.cs:L972.

Explicit multi-target bindings are a DTXManiaRS accessibility feature and
use the approved one-press, one-chip rule.
```

Replace `L972` with the exact line range read during implementation.

---

### Task 5: Replace shared-binding editor operations and copy

**Files:**
- Modify: `crates/gameplay-drums/src/editor/bindings_capture.rs`
- Modify: `crates/gameplay-drums/src/editor/capture_modal.rs`
- Modify: `crates/gameplay-drums/src/editor/bindings_panel.rs`
- Modify: `crates/gameplay-drums/src/editor/lanes_panel.rs`
- Modify: `crates/gameplay-drums/src/editor/bindings_spatial.rs`
- Modify: `crates/gameplay-drums/src/editor/profile_state.rs`

**Interfaces:**
- Consumes: profile edit methods from Task 1 and atomic source lookup from Task 3.
- Produces: `BindingEdit::{SetPrimary, AddAlternate, RemoveTarget, MoveHereOnly}` reducer actions.
- Produces: source detail labels and `1x` marker.

- [ ] **Step 1: Write failing pure reducer tests**

Define reducer actions keyed by `BindSource`:

```rust
enum BindingEdit {
    SetPrimary { source: BindSource, channel: EChannel },
    AddAlternate { source: BindSource, channel: EChannel },
    RemoveTarget { source: BindSource, channel: EChannel },
    MoveHereOnly { source: BindSource, channel: EChannel },
}
```

Test that:

- `AddAlternate` changes Space from `[BD]` to `[BD, LBD]`;
- `SetPrimary` changes it to `[LBD, BD]`;
- `RemoveTarget(BD)` leaves `[LBD]`;
- removing the only target removes the source record;
- `MoveHereOnly(BD)` produces `[BD]`.
- removing a primary with alternates first returns a `ConfirmPrimaryRemoval` modal effect and mutates nothing until confirmation.

- [ ] **Step 2: Run reducer tests and verify failure**

Run: `cargo test -p gameplay-drums editor::bindings_capture::tests --lib`

Expected: compile failure because `BindingEdit` and its reducer do not exist.

- [ ] **Step 3: Replace capture choices**

Replace `CommitShared` and `CommitMove` with `AddAlternate` and `MoveHereOnly`. Add `SetPrimary` for editing an existing multi-target source. The reducer updates the active keyboard or MIDI draft first, then recomposes `LiveBindings` only after a successful edit.

Captured test hits emit one `InputHit` with the arrived source’s complete target list. They do not emit one hit for the capture row plus one for each owner.

- [ ] **Step 4: Render source-centric details in channel rows**

Invert each profile’s source records into rows:

```rust
for binding in &profile.bindings {
    for (priority, channel) in binding.targets.as_slice().iter().enumerate() {
        rows.entry(*channel).or_default().push(BindingChip {
            source: binding.source.into(),
            primary: priority == 0,
            target_count: binding.targets.as_slice().len(),
        });
    }
}
```

Show “Primary pad,” “Also accepts,” and `1x`. Use tooltip copy: “One press accepts {targets}; only one note is judged.” Remove “fires both,” “fan-out,” and chord wording from the panel and modal.

- [ ] **Step 5: Keep spatial highlighting without runtime fan-out**

Hovering a multi-target chip highlights every target channel in the preview. Live test input flashes only the primary. Rename `HighlightedChannels` comments so they describe inspection targets, not shared owners.

- [ ] **Step 6: Clarify the Lanes panel**

Change the detail-card label to `Displayed channels`. Verify that add/remove/reorder/resize methods touch only `LaneProfileDraft` and never binding drafts.

- [ ] **Step 7: Mark migrated profile drafts dirty**

Add `needs_save: bool` to `ProfileDraft`. `is_dirty` returns `self.needs_save || self.value != self.saved`; `saved_now` clears the flag. When startup returns `MigratedSession`, seed the draft with `needs_save: true`, so the profile bar shows the dirty dot and Save writes v2. Ordinary v2 and built-in startup paths use `false`. Do not call a save function during editor startup.

- [ ] **Step 8: Run editor tests**

Run:

```bash
cargo test -p gameplay-drums editor::bindings_capture::tests --lib
cargo test -p gameplay-drums editor::bindings_panel::tests --lib
cargo test -p gameplay-drums editor::profile_state::tests --lib
```

Expected: all commands pass; tests assert the new copy and one-event capture behavior.

- [ ] **Step 9: Commit editor changes**

```bash
git add crates/gameplay-drums/src/editor/bindings_capture.rs crates/gameplay-drums/src/editor/capture_modal.rs crates/gameplay-drums/src/editor/bindings_panel.rs crates/gameplay-drums/src/editor/lanes_panel.rs crates/gameplay-drums/src/editor/bindings_spatial.rs crates/gameplay-drums/src/editor/profile_state.rs
git commit -m "feat(customize): edit primary and accepted pads"
```

---

### Task 6: Add end-to-end regressions and run package gates

**Files:**
- Create: `crates/gameplay-drums/tests/atomic_multi_target_bindings.rs`
- Modify: `crates/gameplay-drums/tests/bindings_lane_pipeline.rs`
- Modify: `crates/gameplay-drums/tests/input_lane_profiles.rs`
- Modify: `docs/decisions/README.md`
- Modify: `docs/superpowers/specs/2026-07-11-controls-lanes-redesign-design.md`

**Interfaces:**
- Consumes: all previous tasks.
- Produces: regression coverage for keyboard/MIDI parity, one-chip consumption, two-input chords, migration safety, and layout independence.

- [ ] **Step 1: Write the end-to-end regression fixture**

Create a minimal Bevy `App` with `InputHit`, `JudgmentEvent`, and `EmptyHit` messages, the judge system, a chart containing simultaneous BD/LBD chips, ready clock, zero input offset, separate drum groups, and empty `JudgedChips`.

The first test writes:

```rust
app.world_mut().write_message(InputHit {
    targets: TargetList::new(EChannel::BassDrum, [EChannel::LeftBassDrum]).expect("targets"),
    audio_ms: target_ms,
    source: InputSourceKind::Keyboard,
});
app.update();
assert_eq!(app.world().resource::<JudgedChips>().0.len(), 1);
```

Read the judgment messages and assert one event for the BD chip.

- [ ] **Step 2: Add two-input chord and MIDI parity tests**

Send `[BD]` and `[LBD]` as two events at the same timestamp and assert two judged chips. Repeat the one-event test with `InputSourceKind::Midi` and assert the same chip index and judgment kind as keyboard.

- [ ] **Step 3: Add layout-independence test**

Run the same atomic event against Classic, NX Type-B, and a custom lane arrangement that merges BD/LBD. Assert the consumed chart chip index is identical for all three. The test must change only the `Lanes` resource between runs.

- [ ] **Step 4: Update obsolete fan-out tests and design record**

Replace assertions that expect `lanes_for_key` or several `LaneHit`s with ordered-target and one-`InputHit` assertions. Add a decision entry to `docs/decisions/README.md`:

```markdown
- **Atomic multi-target inputs.** A physical source has one primary channel and ordered accepted alternates. One multi-target press consumes at most one chip; the primary wins exact-time ties. Visual lane merging does not affect input or judgment. Design: `docs/superpowers/specs/2026-07-12-atomic-multi-target-bindings-design.md`.
```

Add a superseding note to the 2026-07-11 Controls/Lanes spec stating that its shared-source fan-out section was replaced by the 2026-07-12 design.

- [ ] **Step 5: Run changed-package tests**

Run:

```bash
cargo test -p dtx-input
cargo test -p gameplay-drums --lib
cargo test -p gameplay-drums --test atomic_multi_target_bindings
cargo test -p gameplay-drums --test bindings_lane_pipeline
cargo test -p gameplay-drums --test input_lane_profiles
```

Expected: every command exits 0 with no failed tests.

- [ ] **Step 6: Run package checks and lint**

Run:

```bash
cargo check -p dtx-input
cargo check -p gameplay-drums
cargo clippy -p dtx-input -p gameplay-drums --all-targets -- -D warnings
```

Expected: every command exits 0 with no warnings.

- [ ] **Step 7: Commit regression coverage and decisions**

```bash
git add crates/gameplay-drums/tests/atomic_multi_target_bindings.rs crates/gameplay-drums/tests/bindings_lane_pipeline.rs crates/gameplay-drums/tests/input_lane_profiles.rs docs/decisions/README.md docs/superpowers/specs/2026-07-11-controls-lanes-redesign-design.md
git commit -m "test(drums): cover atomic multi-target bindings"
```

- [ ] **Step 8: Verify the final diff**

Run:

```bash
git status --short
git log -6 --oneline
git diff HEAD~6 --check
```

Expected: clean worktree, six logical commits for this plan, and no whitespace errors.
