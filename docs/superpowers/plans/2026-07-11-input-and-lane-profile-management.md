# Input and Lane Profile Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add independent named keyboard, MIDI, and lane profiles with checked migration, transactional persistence, source-specific Controls editing, lane-profile editing, and dirty-close protection while preserving logical drum judgment routing.

**Architecture:** Add pure `dtx-persistence` wrapper over `atomicwrites` for path-plus-bytes replacement. Keep profile schemas in owning pure crates: keyboard/MIDI in `dtx-config`, lanes in `dtx-layout`. Persist `EChannel` names, compose active keyboard/MIDI profiles through existing `lane_of(EChannel) -> LaneId`, and use lane profiles only for display columns. Gameplay editor owns generic draft/transaction reducers and UI state; disk success gates runtime mutation.

**Tech Stack:** Rust 2021/2024 workspace, serde/TOML, `atomicwrites`, Bevy 0.19 ECS/UI/messages, Cargo tests.

## Global Constraints

- Keyboard, MIDI, and lane profiles remain independent.
- Replace Bindings with one Controls tab containing `Keyboard | MIDI`; keep seven top-level Customize tabs.
- Add pure `dtx-persistence` crate wrapping `atomicwrites`; project crates contain no unsafe code.
- `dtx-persistence` owns safe byte replacement plus shared `ProfileName` rules; it contains no TOML schemas, registries, migrations, config-directory logic, or domain built-ins.
- Do not add `dtx-layout -> dtx-config`; sibling profile crates do not depend on each other.
- Do not duplicate profile-name, transaction, lane-order, or safe-write helpers.
- Keep `EChannel` as persistent/editor boundary and existing logical `LaneId` judgment API; lane profiles never change judgment routing.
- Keep built-ins immutable in code: `DTXMania default`, `General MIDI drums`, `Classic`, `NX Type-B`, and `NX Type-D`.
- Validate and serialize a cloned complete registry before writing; mutate runtime state and mark drafts clean only after successful replacement.
- Missing files are not errors. Malformed files never fall back and get overwritten; recovery requires confirmed `Back up and reset`.
- Legacy migration is checked, leaves `bindings.toml` and `layout.toml` untouched, and uses registry existence as its only completion marker.
- Preserve shared keyboard keys and exclusive MIDI notes; MIDI conflicts require explicit steal confirmation.
- Preserve widget data in `layout.toml`; `lane-profiles.toml` is authoritative for lanes once present.
- Do not add bundles, automatic profile switching, import/export, cloud sync, crash-recovery drafts, per-song profiles, guitar/bass profile UI, or a dedicated profile-manager screen.
- No `unwrap()` in `crates/*` production code. `references/` remains read-only.
- Use red-green TDD for every behavior change. One commit per task. Tasks execute sequentially with one writer.
- Run final Cargo gates with `CARGO_BUILD_JOBS=1`; default parallel linking OOMs in this environment.
- Do not modify or stage unrelated user changes.

## File Structure

### New files

- `crates/dtx-persistence/Cargo.toml`: pure crate manifest; `atomicwrites` and `thiserror` only.
- `crates/dtx-persistence/src/lib.rs`: `replace_bytes(path, bytes)` plus shared profile-name validation/comparison/suggestion and focused tests.
- `crates/dtx-config/src/profiles.rs`: keyboard/MIDI profile values, registries, built-ins, validation, reducers, checked load/save, legacy migration/reset.
- `crates/dtx-layout/src/profiles.rs`: lane profile registry, built-ins, checked load/save, legacy migration/reset and compatibility precedence.
- `crates/gameplay-drums/src/editor/profile_state.rs`: generic draft/dirty/action reducers and multi-profile close decisions.
- `crates/gameplay-drums/src/editor/profile_bar.rs`: shared profile selector/Save/Save As/overflow UI.
- `crates/gameplay-drums/src/editor/controls_panel.rs`: Controls segment state and Keyboard/MIDI panel rendering.
- `crates/gameplay-drums/src/editor/profile_dialog.rs`: dirty, name, delete, corrupt-reset, and persistence-error dialogs.
- `crates/gameplay-drums/tests/input_lane_profiles.rs`: headless composition, migration transaction, rollback, and lane-invariance integration tests.
- `docs/migrations/input-and-lane-profiles-v1.md`: exact legacy/new file behavior and manual verification.

### Modified files

- `Cargo.toml`, `Cargo.lock`: workspace deps for `dtx-persistence` and `atomicwrites`.
- `crates/dtx-config/Cargo.toml`, `src/lib.rs`, `src/bindings.rs`: depend on persistence, export profile API, retain legacy checked parser/DTO only.
- `crates/dtx-layout/Cargo.toml`, `src/lib.rs`, `src/file.rs`: depend on persistence, export lane registry API, expose checked legacy parser while keeping scene persistence.
- `crates/game-shell/src/states.rs`: rename `CustomizeTab::Bindings` to `Controls`, preserving order/count.
- `crates/gameplay-drums/src/bindings.rs`: active profile resources and fixed logical resolver composition; remove close auto-save.
- `crates/gameplay-drums/src/editor/mod.rs`, `tabs.rs`, `ui.rs`, `panel.rs`, `bindings_capture.rs`, `bindings_panel.rs`, `bindings_spatial.rs`, `keyboard_nav.rs`, `footer.rs`, `save.rs`, `session.rs`: register profile state/UI, split Controls, route close/exit confirmation, retain widget saving, remove mixed bindings behavior.
- `crates/gameplay-drums/src/lanes.rs`, `layout.rs`: initialize/render active lane profile and live draft without altering judgment IDs.
- `crates/gameplay-drums/tests/editor.rs`, `editor_lanes.rs`, `lane_arrangement.rs`: update tab/profile and compatibility assertions.

---

### Task 1: Add safe byte replacement boundary

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/dtx-persistence/Cargo.toml`
- Create: `crates/dtx-persistence/src/lib.rs`
- Modify: `Cargo.lock`

**Interfaces:**

```rust
pub fn replace_bytes(path: &Path, bytes: &[u8]) -> Result<(), PersistenceError>;

#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    #[error("cannot create parent directory for {path}: {source}")]
    CreateParent { path: PathBuf, source: io::Error },
    #[error("cannot write replacement for {path}: {source}")]
    Write { path: PathBuf, source: io::Error },
    #[error("cannot commit replacement for {path}: {source}")]
    Commit { path: PathBuf, source: io::Error },
}
```

Implementation creates parent directories, then uses `AtomicFile::new(path, OverwriteBehavior::AllowOverwrite).write(|file| file.write_all(bytes))`. Map `atomicwrites::Error::User` to `Write` and `Error::Internal` to `Commit`; never delete destination first. Context7 returned only the unrelated Python package after three Rust-specific lookups, so verify the Rust 0.4.4 API against `https://docs.rs/atomicwrites/0.4.4/atomicwrites/` and its linked source before coding.

- [ ] **Step 1: Write failing tests** in `crates/dtx-persistence/src/lib.rs`: `replace_bytes_creates_parent_and_file`, `replace_bytes_overwrites_complete_contents`, and `directory_target_reports_commit_error_without_deletion`.
- [ ] **Step 2: Verify RED**

Run: `CARGO_BUILD_JOBS=1 cargo test -p dtx-persistence`

Expected: FAIL because package/API does not exist.

- [ ] **Step 3: Add crate and implementation**. Add workspace dependency `dtx-persistence = { path = "crates/dtx-persistence" }` and third-party `atomicwrites = "0.4.4"`. Keep crate pure and `unsafe_code = "forbid"`.
- [ ] **Step 4: Verify GREEN**

Run: `CARGO_BUILD_JOBS=1 cargo test -p dtx-persistence`

Expected: all three tests pass; replacement errors return `Commit`, and project code never deletes the destination first.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/dtx-persistence
git commit -m "feat(persistence): add atomic byte replacement"
```

### Task 2: Add shared profile-name rules once

**Files:**
- Modify: `crates/dtx-persistence/src/lib.rs`

**Interfaces:**

```rust
pub struct ProfileName(String);
pub enum ProfileNameError { Blank, TooLong, ControlCharacter, Reserved, Duplicate }
pub fn comparison_key(name: &str) -> String;
pub fn validate_profile_name(
    raw: &str,
    reserved: impl IntoIterator<Item = &'_ str>,
    existing: impl IntoIterator<Item = &'_ str>,
    current: Option<&str>,
) -> Result<ProfileName, ProfileNameError>;
pub fn suggest_copy_name(base: &str, existing: impl IntoIterator<Item = &'_ str>) -> String;
```

`comparison_key` trims then applies `char::to_lowercase`, without Unicode normalization. Validation counts Unicode scalar values, allows 1..=48, rejects controls/reserved/duplicate comparison keys. Suggestion strips trailing ` space + integer`, starts at 2, returns first unused key.

- [ ] **Step 1: Add failing tests**: `profile_name_trims_and_accepts_48_scalars`, `profile_name_rejects_blank_control_and_49_scalars`, `profile_name_rejects_reserved_case_insensitively`, `profile_name_rejects_duplicate_case_insensitively`, `comparison_key_does_not_normalize_unicode`, `copy_name_increments_numeric_suffix`.
- [ ] **Step 2: Verify RED**

Run: `CARGO_BUILD_JOBS=1 cargo test -p dtx-persistence profile_name`

Expected: compile failure because profile-name API is absent.

- [ ] **Step 3: Implement minimal shared value/helper**. Do not add Unicode or profile dependencies.
- [ ] **Step 4: Verify GREEN**

Run: `CARGO_BUILD_JOBS=1 cargo test -p dtx-persistence profile_name`

Expected: six tests pass, including `Studio kit`, occupied 2/3 → `Studio kit 4`.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-persistence/src/lib.rs
git commit -m "feat(persistence): define shared profile name rules"
```

### Task 3: Define keyboard and MIDI registries plus pure transactions

**Files:**
- Modify: `crates/dtx-config/Cargo.toml`
- Create: `crates/dtx-config/src/profiles.rs`
- Modify: `crates/dtx-config/src/lib.rs`

**Interfaces:**

```rust
pub const KEYBOARD_DEFAULT_NAME: &str = "DTXMania default";
pub const MIDI_DEFAULT_NAME: &str = "General MIDI drums";
pub struct KeyboardProfile { pub map: HashMap<EChannel, Vec<KeyCode>> }
pub struct MidiProfile {
    pub port: Option<String>,
    pub velocity_threshold: u8,
    pub map: HashMap<EChannel, Vec<u8>>,
}
pub struct ProfileRegistry<T> {
    pub version: u32,
    pub active: String,
    pub profiles: BTreeMap<String, T>, // user profiles only
}
pub enum RegistryAction<T> {
    Select(String), Save(T), SaveAs { name: ProfileName, value: T },
    Rename(ProfileName), Delete, Revert,
}
pub fn reduce_registry<T: Clone + PartialEq>(
    registry: &ProfileRegistry<T>, builtins: &BTreeMap<String, T>, action: RegistryAction<T>,
) -> Result<ProfileRegistry<T>, RegistryError>;
```

Private serde DTOs emit keyboard channel arrays directly under each profile and MIDI `port`, `velocity_threshold`, and nested `map`. Normalize empty MIDI port strings to `None`. Built-ins never enter `profiles` and cannot be saved/renamed/deleted. Delete active user profile selects type fallback in same cloned value.

- [ ] **Step 1: Add failing tests**: `keyboard_registry_round_trips_spec_shape`, `midi_registry_round_trips_spec_shape`, `missing_and_newer_registry_versions_are_distinct`, `keyboard_key_can_exist_under_multiple_channels`, `midi_note_conflict_reports_owner`, `save_builtin_is_rejected`, `rename_moves_key_and_active_together`, `delete_active_selects_builtin_fallback`, `revert_restores_active_value`.
- [ ] **Step 2: Verify RED**

Run: `CARGO_BUILD_JOBS=1 cargo test -p dtx-config profiles::tests`

Expected: compile failure because profile types are absent.

- [ ] **Step 3: Implement schemas, built-ins, validation, and cloned reducers** by partitioning existing `InputBindings::default()` once into exact keyboard/MIDI defaults. MIDI insertion removes same note from other channels only after caller confirms.
- [ ] **Step 4: Verify GREEN**

Run: `CARGO_BUILD_JOBS=1 cargo test -p dtx-config profiles::tests`

Expected: all listed tests pass; serialized TOML matches approved shapes and no mixed `BindSource` appears.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-config/Cargo.toml crates/dtx-config/src/lib.rs crates/dtx-config/src/profiles.rs
git commit -m "feat(config): add keyboard and MIDI profile registries"
```

### Task 4: Add checked config registry I/O, migration, and reset

**Files:**
- Modify: `crates/dtx-config/src/profiles.rs`
- Modify: `crates/dtx-config/src/bindings.rs`
- Modify: `crates/dtx-config/src/lib.rs`

**Interfaces:**

```rust
pub enum CheckedLoad<T> { Missing, Loaded(T), Malformed(RegistryLoadError) }
pub enum RegistryStartup<T> { Ready(T), LegacySession { registry: T, write_error: RegistryIoError }, ReadOnlyBuiltins(RegistryLoadError) }
pub fn load_keyboard_registry(path: &Path, legacy: &Path) -> RegistryStartup<ProfileRegistry<KeyboardProfile>>;
pub fn load_midi_registry(path: &Path, legacy: &Path) -> RegistryStartup<ProfileRegistry<MidiProfile>>;
pub fn save_keyboard_registry(path: &Path, registry: &ProfileRegistry<KeyboardProfile>) -> Result<(), RegistryIoError>;
pub fn save_midi_registry(path: &Path, registry: &ProfileRegistry<MidiProfile>) -> Result<(), RegistryIoError>;
pub fn backup_and_reset_keyboard_registry(path: &Path, confirmed: bool, now: SystemTime) -> Result<ProfileRegistry<KeyboardProfile>, RegistryIoError>;
pub fn backup_and_reset_midi_registry(path: &Path, confirmed: bool, now: SystemTime) -> Result<ProfileRegistry<MidiProfile>, RegistryIoError>;
```

Add `parse_bindings_checked(raw) -> Result<BindingsFile, ConfigError>`; keep fallback parser only for legacy callers until removed. New paths: `keyboard-profiles.toml`, `midi-profiles.toml`. Migration partitions every valid v1 `BindSource`: matching halves activate built-ins; changed halves become `Migrated keyboard`/`Migrated MIDI`. Each output is independent and atomic. Failed write returns valid migrated session state without creating completion marker; malformed legacy enters read-only mode. Backup uses checked rename to timestamped sibling before default atomic write.

- [ ] **Step 1: Add failing tests**: `checked_legacy_load_distinguishes_missing_and_malformed`, `mixed_v1_bindings_partition_migration_preserves_device_fields`, `matching_legacy_halves_activate_builtins`, `changed_halves_get_migrated_names`, `migration_write_failure_retries_when_registry_remains_missing`, `existing_registry_skips_legacy`, `corrupt_registry_cannot_be_saved`, `reset_requires_confirmation_and_preserves_timestamped_backup`.
- [ ] **Step 2: Verify RED**

Run: `CARGO_BUILD_JOBS=1 cargo test -p dtx-config migration`

Expected: compile/test failure because checked startup APIs are absent.

- [ ] **Step 3: Implement checked reads and atomic writes**. Every persistence fn clones, validates, serializes, calls `dtx_persistence::replace_bytes`, then returns new value. Never call old fallback parser from migration.
- [ ] **Step 4: Verify GREEN and crate**

Run: `CARGO_BUILD_JOBS=1 cargo test -p dtx-config`

Expected: all config tests pass; malformed legacy creates neither registry; simulated failed write is idempotently retried.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-config/src/bindings.rs crates/dtx-config/src/lib.rs crates/dtx-config/src/profiles.rs
git commit -m "feat(config): migrate bindings into checked profile registries"
```

### Task 5: Add lane registry, checked migration, and authority rules

**Files:**
- Modify: `crates/dtx-layout/Cargo.toml`
- Create: `crates/dtx-layout/src/profiles.rs`
- Modify: `crates/dtx-layout/src/file.rs`
- Modify: `crates/dtx-layout/src/lib.rs`

**Interfaces:**

```rust
pub const LANE_DEFAULT_NAME: &str = "Classic";
pub struct LaneProfile { pub arrangement: LaneArrangement }
pub struct LaneProfileRegistry {
    pub version: u32,
    pub active: String,
    pub profiles: BTreeMap<String, LaneProfile>,
}
pub fn load_lane_registry(path: &Path, legacy_layout: &Path) -> LaneRegistryStartup;
pub fn save_lane_registry(path: &Path, registry: &LaneProfileRegistry) -> Result<(), LaneRegistryError>;
pub fn active_lane_arrangement(registry: &LaneProfileRegistry) -> &LaneArrangement;
pub fn load_layout_with_lane_authority(layout: &Path, lane_registry: &Path) -> Result<(LayoutFile, LaneRegistryStartup), LayoutError>;
```

Reuse `dtx_persistence::ProfileName`; do not copy config name helpers or depend on `dtx-config`. Reuse `LanesSection::resolve/from_arrangement` for stable short names and repair rules. Built-ins resolve exact preset tables. Named built-in legacy activates it; custom becomes `Migrated lanes`. Once registry exists, ignore legacy `[lanes]` at startup while loading `[scene]` normally. Lane migration writes only `lane-profiles.toml`.

- [ ] **Step 1: Add failing tests**: `lane_registry_round_trips_custom_order_widths_and_map`, `lane_builtins_resolve_exact_presets`, `named_legacy_layout_activates_builtin`, `custom_legacy_layout_becomes_migrated_lanes`, `malformed_legacy_layout_blocks_migration`, `lane_registry_takes_precedence_over_compatibility_snapshot`, `lane_migration_does_not_rewrite_layout_scene`, `lane_migration_retry_is_idempotent`, `corrupt_lane_registry_requires_confirmed_backup_reset`.
- [ ] **Step 2: Verify RED**

Run: `CARGO_BUILD_JOBS=1 cargo test -p dtx-layout profiles::tests`

Expected: compile failure because lane registry API is absent.

- [ ] **Step 3: Implement registry and checked parser**. Preserve width clamps, unknown-ID drops, map repairs, and all `DRUM_CHANNELS` coverage. Store deterministic DTO maps with `BTreeMap`.
- [ ] **Step 4: Verify GREEN and crate**

Run: `CARGO_BUILD_JOBS=1 cargo test -p dtx-layout`

Expected: all tests pass; scene survives migration unchanged; registry wins over stale `[lanes]`.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-layout/Cargo.toml crates/dtx-layout/src/file.rs crates/dtx-layout/src/lib.rs crates/dtx-layout/src/profiles.rs
git commit -m "feat(layout): add authoritative lane profile registry"
```

### Task 6: Compose active input profiles into fixed logical lanes

**Files:**
- Modify: `crates/gameplay-drums/src/bindings.rs`
- Modify: `crates/gameplay-drums/src/editor/mod.rs`

**Interfaces:**

```rust
#[derive(Resource)] pub struct ActiveInputProfiles {
    pub keyboard: KeyboardProfile,
    pub midi: MidiProfile,
}
impl BindResolver {
    pub fn from_profiles(keyboard: &KeyboardProfile, midi: &MidiProfile) -> Self;
}
```

Replace `LiveBindings`; startup loads/migrates registries and resolves active values. `from_profiles` iterates `BINDABLE_CHANNELS`, calls existing `lane_of`, keeps `KeyCode -> Vec<LaneId>`, `note -> LaneId`, and MIDI threshold. Remove `save_bindings_on_close`; editor preview may rebuild resolver from drafts, but committed active resources change only after registry write succeeds.

- [ ] **Step 1: Replace/add failing tests**: `active_profiles_compose_keyboard_and_midi`, `shared_key_emits_all_fixed_logical_lanes`, `exclusive_note_emits_one_fixed_logical_lane`, `changing_lane_arrangement_does_not_change_resolver_lane_id`, `failed_registry_selection_keeps_active_resolver`.
- [ ] **Step 2: Verify RED**

Run: `CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums bindings::tests`

Expected: compile failure until resolver accepts separate profiles.

- [ ] **Step 3: Implement composition and load resources**, retaining public `lane_for_key`, `lanes_for_key`, `lane_for_note`, and `velocity_threshold` judgment consumers.
- [ ] **Step 4: Verify GREEN**

Run: `CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums bindings::tests`

Expected: tests pass; Classic/NX/custom display arrangements produce identical logical `LaneId` for same `EChannel`.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/bindings.rs crates/gameplay-drums/src/editor/mod.rs
git commit -m "feat(gameplay): compose independent input profiles"
```

### Task 7: Add UI-independent draft and transaction reducers

**Files:**
- Create: `crates/gameplay-drums/src/editor/profile_state.rs`
- Modify: `crates/gameplay-drums/src/editor/mod.rs`

**Interfaces:**

```rust
pub struct ProfileDraft<T> { pub selected: String, pub saved: T, pub value: T }
pub enum DirtyDecision { Save, SaveAs(ProfileName), Discard, Cancel }
pub enum PendingProfileAction { Select(String), Revert, CloseCustomize, ExitApp }
pub struct ProfileSession { pub keyboard: ..., pub midi: ..., pub lanes: ... }
pub enum TransactionResult<R, T> { Committed { registry: R, draft: ProfileDraft<T> }, Unchanged, Failed(ProfileError) }
pub fn reduce_dirty_action<T: Clone + PartialEq>(...) -> DraftEffect<T>;
pub fn dirty_profile_kinds(session: &ProfileSession) -> Vec<ProfileKind>;
```

Reducers build complete next registries for clean select, Save, Save As, rename, delete, dirty Save+select, and dirty Discard+select. Cancel yields no mutation/write. Multi-save returns per-kind success so successful drafts clean and failed drafts remain dirty.

- [ ] **Step 1: Add failing tests**: `clean_select_requests_active_transaction`, `dirty_select_save_combines_save_and_selection`, `dirty_select_discard_does_not_persist_draft`, `dirty_select_cancel_is_noop`, `builtin_save_requires_save_as`, `save_all_cleans_only_successful_drafts`, `tab_and_controls_segment_changes_keep_all_drafts`.
- [ ] **Step 2: Verify RED**

Run: `CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums profile_state::tests`

Expected: compile failure because reducer module is absent.

- [ ] **Step 3: Implement generic reducers**. Keep Bevy/disk out of reducer fns; effects describe one registry write and post-success state.
- [ ] **Step 4: Verify GREEN**

Run: `CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums profile_state::tests`

Expected: all transition tests pass, including partial Save All.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/editor/mod.rs crates/gameplay-drums/src/editor/profile_state.rs
git commit -m "feat(editor): add profile draft transaction reducers"
```

### Task 8: Rename Bindings to Controls and add segment navigation

**Files:**
- Modify: `crates/game-shell/src/states.rs`
- Modify: `crates/gameplay-drums/src/editor/tabs.rs`
- Modify: `crates/gameplay-drums/src/editor/panel.rs`
- Modify: `crates/gameplay-drums/src/editor/keyboard_nav.rs`
- Modify: `crates/gameplay-drums/src/editor/bindings_spatial.rs`
- Create: `crates/gameplay-drums/src/editor/controls_panel.rs`
- Modify: `crates/gameplay-drums/src/editor/mod.rs`

**Interfaces:**

```rust
pub enum CustomizeTab { Gameplay, Audio, Drums, System, Controls, Lanes, Widgets }
#[derive(Resource, Default)] pub enum ControlsSegment { #[default] Keyboard, Midi }
```

Controls focus contract: Down/Enter enters segment selector; Left/Right switches segment; Down enters profile/mapping rows; Up returns one level. Top-level switches preserve both drafts.

- [ ] **Step 1: Update/add failing tests**: `controls_is_a_kit_tab`, `customize_still_has_seven_tabs`, `controls_segment_left_right_switches`, `controls_down_enters_segment_then_rows`, `controls_up_returns_one_level`, `pad_exclusion_matches_controls_contract`.
- [ ] **Step 2: Verify RED**

Run: `CARGO_BUILD_JOBS=1 cargo test -p game-shell customize_tab && CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums controls_segment`

Expected: compile failures at old `Bindings` variant and missing segment state.

- [ ] **Step 3: Rename all `CustomizeTab::Bindings` callers and add segment state/navigation**. Do not add top-level variant.
- [ ] **Step 4: Verify GREEN**

Run: `CARGO_BUILD_JOBS=1 cargo test -p game-shell && CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums controls_segment`

Expected: shell tests pass with seven tabs; segment focus tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/game-shell/src/states.rs crates/gameplay-drums/src/editor/{mod.rs,tabs.rs,panel.rs,keyboard_nav.rs,bindings_spatial.rs,controls_panel.rs}
git commit -m "feat(customize): replace Bindings with Controls segments"
```

### Task 9: Add shared profile bar and transactional actions

**Files:**
- Create: `crates/gameplay-drums/src/editor/profile_bar.rs`
- Create: `crates/gameplay-drums/src/editor/profile_dialog.rs`
- Modify: `crates/gameplay-drums/src/editor/mod.rs`
- Modify: `crates/gameplay-drums/src/editor/panel.rs`

**Interfaces:**

```rust
pub enum ProfileBarAction { Select(String), Save, SaveAs, Rename, Revert, Delete }
pub struct ProfileUiError { pub kind: ProfileKind, pub path: PathBuf, pub message: String }
pub enum ProfileDialogState { Closed, Name { action: NameAction, value: String, error: Option<ProfileNameError> }, ConfirmDelete, Dirty(...), CorruptReset(...) }
```

Selector lists built-ins first, then user insertion/key order, marks selected. Save disabled for built-ins/clean drafts. Overflow: user Rename/Revert/Delete; built-in Save As only. Name dialog preselects shared suggestion and retains inline errors. Every action executes reducer → atomic registry write → post-success runtime/draft update. On failure keep prior runtime/selection/draft, show path+cause, and re-read canonical registry before next write.

- [ ] **Step 1: Add failing headless UI/state tests**: `profile_bar_groups_builtins_before_users`, `builtin_overflow_only_offers_save_as`, `user_overflow_offers_rename_revert_delete`, `invalid_save_as_keeps_name_dialog_open`, `transaction_failure_keeps_draft_and_active_selection`, `next_write_reloads_after_failure`.
- [ ] **Step 2: Verify RED**

Run: `CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums profile_bar`

Expected: compile failure because shared bar/dialog do not exist.

- [ ] **Step 3: Implement one reusable bar/action path** for all profile kinds; no three copied action handlers.
- [ ] **Step 4: Verify GREEN**

Run: `CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums profile_bar`

Expected: all action/rollback tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/editor/{mod.rs,panel.rs,profile_bar.rs,profile_dialog.rs}
git commit -m "feat(editor): add transactional profile controls"
```

### Task 10: Split keyboard Controls and keyboard-only capture

**Files:**
- Modify: `crates/gameplay-drums/src/editor/controls_panel.rs`
- Modify: `crates/gameplay-drums/src/editor/bindings_capture.rs`
- Modify: `crates/gameplay-drums/src/editor/bindings_panel.rs`
- Modify: `crates/gameplay-drums/src/editor/footer.rs`
- Modify: `crates/gameplay-drums/src/editor/panel.rs`

**Interfaces:**

```rust
pub enum CaptureState {
    Idle,
    Keyboard(EChannel),
    Midi(EChannel),
    ConfirmMidiSteal { channel: EChannel, note: u8, from: EChannel },
}
pub fn channels_in_display_order(arrangement: &LaneArrangement) -> Vec<EChannel>;
```

`channels_in_display_order` iterates display lanes left-to-right, primary first, then remaining mapped channels in canonical `DRUM_CHANNELS` order, once each. Keyboard rows render keys only. Capture rejects Escape, Tab, F1..F12, and any key pressed with Ctrl/Alt/Super; Escape cancels. Binding uses shared semantics.

- [ ] **Step 1: Add failing tests**: `display_order_uses_primary_then_canonical_secondaries_once`, `keyboard_capture_ignores_new_midi_hit`, `keyboard_capture_rejects_reserved_and_modified_keys`, `keyboard_capture_adds_shared_key_without_steal`, `escape_cancels_keyboard_capture`, `footer_describes_keyboard_capture`.
- [ ] **Step 2: Verify RED**

Run: `CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums keyboard_capture`

Expected: current mixed capture consumes MIDI and fails source-isolation tests.

- [ ] **Step 3: Render Keyboard segment and split state machine**. Mutate `ProfileDraft<KeyboardProfile>` only; rebuild preview resolver without committing active registry.
- [ ] **Step 4: Verify GREEN**

Run: `CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums keyboard_capture`

Expected: all keyboard isolation/order tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/editor/{controls_panel.rs,bindings_capture.rs,bindings_panel.rs,footer.rs,panel.rs}
git commit -m "feat(controls): add keyboard profile editor"
```

### Task 11: Add MIDI Controls, port contract, and MIDI-only learning

**Files:**
- Modify: `crates/gameplay-drums/src/editor/controls_panel.rs`
- Modify: `crates/gameplay-drums/src/editor/bindings_capture.rs`
- Modify: `crates/gameplay-drums/src/editor/bindings_panel.rs`
- Modify: `crates/gameplay-drums/src/editor/footer.rs`

**Interfaces:**

```rust
pub enum PortMatch { FirstAvailable, Exact(usize), Substring { index: usize, ambiguous: bool }, Disconnected }
pub fn match_midi_port(filter: Option<&str>, enumerated: &[String]) -> PortMatch;
```

Empty/whitespace filter normalizes to `None`. `None` selects first available. Otherwise exact case-sensitive full name wins; absent exact uses first case-sensitive substring in enumeration order and flags multiple matches; no match is Disconnected. Selection stores full enumerated name. Never auto-switch profile. MIDI hits update meter/lane-test only and never navigation. Learn consumes strictly newer positive-velocity NoteOn only; keyboard ignored. Existing note opens explicit steal confirmation.

- [ ] **Step 1: Add failing tests**: `empty_port_filter_normalizes_to_none`, `port_match_prefers_exact_case_sensitive_name`, `port_match_uses_first_case_sensitive_substring_and_warns`, `missing_port_is_disconnected_without_profile_switch`, `midi_capture_ignores_keyboard`, `stale_midi_hit_is_not_learned`, `midi_conflict_requires_confirmed_steal`, `physical_hit_does_not_change_editor_navigation`.
- [ ] **Step 2: Verify RED**

Run: `CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums midi_capture port_match`

Expected: mixed capture/old port label behavior fails isolation and ambiguity tests.

- [ ] **Step 3: Implement MIDI segment** with status, rescan, threshold, live meter, note-only chips, Learn pad, and confirmation. Keep selected profile active when disconnected.
- [ ] **Step 4: Verify GREEN**

Run: `CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums midi_capture && CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums port_match`

Expected: all port and learning tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/editor/{controls_panel.rs,bindings_capture.rs,bindings_panel.rs,footer.rs}
git commit -m "feat(controls): add MIDI profile editor and port matching"
```

### Task 12: Wire lane profiles to live preview and shared actions

**Files:**
- Modify: `crates/gameplay-drums/src/lanes.rs`
- Modify: `crates/gameplay-drums/src/layout.rs`
- Modify: `crates/gameplay-drums/src/editor/panel.rs`
- Modify: `crates/gameplay-drums/src/editor/profile_state.rs`
- Modify: `crates/gameplay-drums/tests/editor_lanes.rs`
- Modify: `crates/gameplay-drums/tests/lane_arrangement.rs`

**Interfaces:**

```rust
#[derive(Resource)] pub struct LaneProfileDraft(pub ProfileDraft<LaneProfile>);
fn apply_lane_draft_preview(draft: Res<LaneProfileDraft>, lanes: ResMut<Lanes>);
```

Add shared profile bar above current reorder/resize/split/merge controls. Manual edits update draft arrangement and `Lanes` preview but retain selected profile name. Built-in Save disabled; Save As creates user profile. Clean selection resolves exact built-in/user arrangement after successful active-registry write.

- [ ] **Step 1: Add failing tests**: `lane_edit_keeps_user_profile_name`, `builtin_lane_edit_requires_save_as`, `lane_draft_updates_playfield_preview`, `cancelled_lane_switch_keeps_preview`, `successful_lane_selection_updates_display_only`, `all_lane_profiles_preserve_logical_judgment_id`.
- [ ] **Step 2: Verify RED**

Run: `CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums --test editor_lanes`

Expected: current edits change preset marker to generic Custom and lack named draft behavior.

- [ ] **Step 3: Route existing lane edit handlers through `LaneProfileDraft`**, then mirror its arrangement to `Lanes`. Keep `LanePreset::Custom` internal payload marker; display profile name from draft metadata.
- [ ] **Step 4: Verify GREEN**

Run: `CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums --test editor_lanes && CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums --test lane_arrangement`

Expected: profile/live preview tests pass; fixed logical IDs unchanged.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/{lanes.rs,layout.rs} crates/gameplay-drums/src/editor/{panel.rs,profile_state.rs} crates/gameplay-drums/tests/{editor_lanes.rs,lane_arrangement.rs}
git commit -m "feat(lanes): edit named profiles with live preview"
```

### Task 13: Add corrupt-registry and transaction error UI

**Files:**
- Modify: `crates/gameplay-drums/src/editor/profile_dialog.rs`
- Modify: `crates/gameplay-drums/src/editor/profile_state.rs`
- Modify: `crates/gameplay-drums/src/editor/panel.rs`

**Interfaces:**

Read-only startup state disables profile mutation while showing built-ins. `Back up and reset` opens explicit confirmation; confirm calls owning crate checked backup/reset. Transaction errors show profile kind, canonical path, and full cause. Failure sets `reload_before_write`; next persistence action re-reads canonical registry first and aborts if it is malformed.

- [ ] **Step 1: Add failing tests**: `corrupt_registry_shows_read_only_builtins`, `reset_button_requires_second_confirmation`, `failed_backup_does_not_create_default_registry`, `transaction_error_contains_path_and_cause`, `canonical_reread_blocks_write_after_external_corruption`.
- [ ] **Step 2: Verify RED**

Run: `CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums corrupt_registry`

Expected: no recovery/error dialog behavior exists.

- [ ] **Step 3: Implement shared recovery/error states** using owning crate APIs; never delete corrupt canonical file or infer success from temp files.
- [ ] **Step 4: Verify GREEN**

Run: `CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums corrupt_registry`

Expected: all read-only/reset/reread tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/editor/{profile_dialog.rs,profile_state.rs,panel.rs}
git commit -m "feat(editor): surface profile persistence recovery errors"
```

### Task 14: Protect dirty profile replacement, close, and graceful exit

**Files:**
- Modify: `crates/gameplay-drums/src/editor/ui.rs`
- Modify: `crates/gameplay-drums/src/editor/mod.rs`
- Modify: `crates/gameplay-drums/src/editor/session.rs`
- Modify: `crates/gameplay-drums/src/editor/profile_dialog.rs`
- Modify: `crates/gameplay-drums/src/editor/profile_state.rs`
- Modify: `crates/gameplay-drums/src/editor/tabs.rs`
- Modify: `crates/gameplay-drums/src/bindings.rs`
- Modify: `crates/gameplay-drums/tests/editor_session.rs`

**Interfaces:**

```rust
pub enum CloseIntent { Customize, GracefulAppExit }
pub struct PendingClose { pub intent: CloseIntent, pub dirty: Vec<ProfileKind> }
pub enum CloseDecision { Cancel, DiscardAll, SaveAll }
```

Intercept Esc/`EditorCloseRequest` before setting `EditorOpen(false)`. One dirty user: `Cancel | Discard changes | Save changes`; built-in primary `Save as new profile`. Multiple: list kinds, `Cancel | Discard all | Save all`. Save is default focus/Enter; Escape cancels; destructive action never default. Save All writes sequentially and independently; successful drafts clean, failures remain dirty with dialog open/errors. Graceful exit uses same flow; forced process death cannot prompt and stays out of scope. Non-profile config/widget behavior remains existing policy.

- [ ] **Step 1: Add failing tests**: `dirty_close_does_not_flip_editor_open`, `single_user_dialog_orders_cancel_discard_save`, `builtin_dialog_uses_save_as_primary`, `multiple_dirty_dialog_lists_kinds`, `enter_saves_and_escape_cancels`, `discard_never_has_default_focus`, `partial_save_all_closes_only_successful_drafts`, `graceful_exit_waits_for_dirty_decision`.
- [ ] **Step 2: Verify RED**

Run: `CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums dirty_close`

Expected: current close immediately flips `EditorOpen` and auto-saves.

- [ ] **Step 3: Route close/exit through pending intent**. Remove profile auto-save from `close_editor_on_exit`, `save_bindings_on_close`, and close-trigger systems. Only finalize close after Discard or all required saves succeed.
- [ ] **Step 4: Verify GREEN**

Run: `CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums dirty_close && CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums --test editor_session`

Expected: all dirty and session lifecycle tests pass; partial failures retain dialog/drafts.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/bindings.rs crates/gameplay-drums/src/editor/{ui.rs,mod.rs,session.rs,profile_dialog.rs,profile_state.rs,tabs.rs} crates/gameplay-drums/tests/editor_session.rs
git commit -m "feat(editor): protect dirty profiles on close and exit"
```

### Task 15: Keep widget saves separate and write compatibility lane snapshot

**Files:**
- Modify: `crates/gameplay-drums/src/editor/save.rs`
- Modify: `crates/gameplay-drums/src/editor/mod.rs`
- Modify: `crates/gameplay-drums/tests/editor.rs`
- Modify: `crates/gameplay-drums/tests/editor_lanes.rs`

**Interfaces:**

```rust
pub fn layout_file_from(
    layouts: &WidgetLayouts,
    active_lanes: &LaneArrangement,
) -> LayoutFile;
```

`Ctrl+S` and existing widget close-save write `layout.toml [scene]` plus non-authoritative active lane snapshot. They never commit profile drafts. Startup ignores snapshot whenever `lane-profiles.toml` exists. Remove old lane preset cycle as profile selection owns built-ins.

- [ ] **Step 1: Add failing tests**: `widget_save_keeps_scene_and_active_lane_snapshot`, `ctrl_s_does_not_clean_lane_profile_draft`, `layout_close_save_does_not_commit_profile_drafts`, `registry_remains_authoritative_after_compatibility_snapshot_write`.
- [ ] **Step 2: Verify RED**

Run: `CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums compatibility_snapshot`

Expected: old save path treats live lane edits as authoritative and lacks dirty assertion.

- [ ] **Step 3: Separate widget/layout persistence from profile transactions**. Snapshot only last successfully active lane profile, not unsaved preview draft.
- [ ] **Step 4: Verify GREEN**

Run: `CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums compatibility_snapshot`

Expected: scene and snapshot persist; profile dirty state remains unchanged.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/editor/{save.rs,mod.rs} crates/gameplay-drums/tests/{editor.rs,editor_lanes.rs}
git commit -m "fix(layout): keep profile authority outside widget saves"
```

### Task 16: Add end-to-end migration, rollback, and lane-invariance coverage

**Files:**
- Create: `crates/gameplay-drums/tests/input_lane_profiles.rs`

**Test fixture:** Use isolated temp config directory with legacy `bindings.toml` and `layout.toml`; call pure startup loaders directly; construct `BindResolver`; apply Classic, NX Type-B, NX Type-D, and custom lane registries. Inject deterministic persistence failure by placing a regular file where the registry parent directory must exist; do not rely on chmod behavior or production-only hooks.

- [ ] **Step 1: Write failing tests**: `legacy_profiles_migrate_and_compose_end_to_end`, `classic_nx_and_custom_profiles_keep_same_lane_hit_id`, `dirty_save_and_select_rolls_back_on_write_failure`, `multi_draft_save_all_retains_only_failed_draft`, `malformed_legacy_files_create_no_registries`, `second_startup_uses_registries_and_ignores_legacy_snapshots`.
- [ ] **Step 2: Verify RED**

Run: `CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums --test input_lane_profiles`

Expected: at least one integration assertion fails until cross-crate startup/transaction wiring is complete.

- [ ] **Step 3: Complete the integration fixture and assertions** using only public APIs from Tasks 1-15. If a failing assertion exposes a production gap, return the fix to its owning module and amend that owning task's commit; do not add alternate profile abstractions or duplicate reducers in the integration test.
- [ ] **Step 4: Verify GREEN**

Run: `CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums --test input_lane_profiles`

Expected: six tests pass; emitted logical lane IDs match across all display profiles; failed transactions preserve runtime/draft state and reconcile the canonical file before retry.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/tests/input_lane_profiles.rs
git commit -m "test(gameplay): cover input and lane profiles end to end"
```

### Task 17: Document migration and perform manual UI verification

**Files:**
- Create: `docs/migrations/input-and-lane-profiles-v1.md`
- Modify: `docs/ROADMAP.md`

**Documentation content:** Exact three registry paths/schemas, built-ins, name comparison rule, checked first-run migration matrix, untouched legacy files, lane snapshot precedence, corrupt backup/reset behavior, failed migration retry, `atomicwrites` limits, and transaction/reread behavior. Include no claim of Windows backup recovery beyond `atomicwrites`.

- [ ] **Step 1: Write migration doc and update roadmap status** without changing unrelated milestones.
- [ ] **Step 2: Run doc/schema verification**

Run: `grep -R "bindings.toml\|keyboard-profiles.toml\|midi-profiles.toml\|lane-profiles.toml" docs/migrations/input-and-lane-profiles-v1.md`

Expected: all four filenames appear with authority/migration roles.

- [ ] **Step 3: Run manual checks**

Run: `CARGO_BUILD_JOBS=1 cargo run -p dtxmaniars-desktop`

Expected manual results:
- Controls focus/pointer switches Keyboard/MIDI without losing drafts.
- Keyboard capture cannot consume MIDI; MIDI learn cannot consume keyboard.
- Save, Save As, rename, revert, delete, dirty switch, one/many dirty close work with specified button order/default focus.
- MIDI exact/substring/ambiguous/disconnected/rescan/threshold/meter behavior matches contract; physical hit does not navigate.
- Lanes preview updates live; built-in edit requires Save As; user name remains visible.
- Corrupt registry is read-only until confirmed backup/reset; write error shows path/cause.
- Supported resolutions keep profile lists/dialog text readable.

- [ ] **Step 4: Commit**

```bash
git add docs/ROADMAP.md docs/migrations/input-and-lane-profiles-v1.md
git commit -m "docs: describe input and lane profile migration"
```

### Task 18: Run final gates and review scope

**Files:**
- Modify only files required by failures; amend owning task commit rather than create unrelated cleanup.

- [ ] **Step 1: Format check**

Run: `cargo fmt --all -- --check`

Expected: exit 0. If it fails, run `cargo fmt --all`, inspect diff, and amend affected task commit.

- [ ] **Step 2: Run required tests serially**

```bash
CARGO_BUILD_JOBS=1 cargo test -p dtx-persistence
CARGO_BUILD_JOBS=1 cargo test -p dtx-config
CARGO_BUILD_JOBS=1 cargo test -p dtx-layout
CARGO_BUILD_JOBS=1 cargo test -p gameplay-drums
```

Expected: all tests pass.

- [ ] **Step 3: Check workspace serially**

Run: `CARGO_BUILD_JOBS=1 cargo check --workspace`

Expected: exit 0 with no compile errors.

- [ ] **Step 4: Lint changed crates serially**

Run: `CARGO_BUILD_JOBS=1 cargo clippy -p dtx-persistence -p dtx-config -p dtx-layout -p game-shell -p gameplay-drums --all-targets -- -D warnings`

Expected: exit 0.

- [ ] **Step 5: Review invariants and scope**

Run:

```bash
git diff --check
git grep -n "CustomizeTab::Bindings\|save_bindings_on_close"
git grep -n "unsafe" -- crates/dtx-persistence crates/dtx-config crates/dtx-layout
git status --short
git diff --cached --name-only
```

Expected: `git diff --check` clean; removed-symbol greps empty; no project unsafe code; only planned files changed; no staged files after implementation review unless worker intentionally staged final commit.

- [ ] **Step 6: Final commit only if gate fixes were required**

Amend owning commit. Do not create a broad cleanup commit.

## Dependencies and Single-Writer Order

1. Task 1 blocks all registry writes.
2. Task 2 blocks Tasks 3 and 5 name behavior.
3. Tasks 3-4 block runtime/editor input work.
4. Task 5 blocks lane startup/editor work.
5. Task 6 blocks preview and end-to-end tests.
6. Task 7 blocks all profile UI actions and dirty flow.
7. Tasks 8-11 build Controls sequentially; same editor files require one writer.
8. Task 12 follows shared profile UI and lane registry.
9. Tasks 13-15 follow all three profile drafts and remain sequential due shared dialog/close/save files.
10. Task 16 integrates completed behavior only.
11. Task 17 documents verified behavior.
12. Task 18 runs after all commits.

No tasks run concurrently. Each task owns one reviewable commit before next starts.

## Self-Review

- **Spec coverage:** Includes independent registries, immutable built-ins, exact name rules/suggestions, checked missing-vs-malformed migration, idempotent retries, atomic writes, all transaction actions, rollback, Controls split, source-specific capture, MIDI port matching/status, lane live preview, compatibility snapshot authority, dirty switch/close/exit, partial Save All, corrupt reset/error UI, docs, integration/manual/final gates.
- **Type consistency:** Persistent/editor keys remain `EChannel`; only `BindResolver::from_profiles` calls existing `lane_of` and emits existing `LaneId`. `LaneArrangement` remains display-only. `dtx-layout` and `dtx-config` share `dtx_persistence::ProfileName` and safe replacement, never depend on each other.
- **Persistence consistency:** Owning crates serialize cloned complete registries; `dtx-persistence` accepts bytes only. Successful helper return is sole trigger for committed runtime/draft changes. Legacy sources remain untouched.
- **Scope check:** No bundles, auto-switching, import/export, profile manager, crash drafts, per-song or guitar/bass UI, Windows unsafe wrapper, or speculative recovery artifacts.
- **Completeness check:** No incomplete markers remain. Every task names files, symbols, RED/GREEN commands, expected results, and one commit.
- **Environment check:** Build/test/check/clippy commands use `CARGO_BUILD_JOBS=1` where linking/build pressure matters.
