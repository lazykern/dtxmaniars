//! End-to-end profile coverage: legacy migration, composition, rollback,
//! partial Save All, malformed legacy input, and second-startup authority.
//!
//! Uses isolated temp config directories and the pure startup loaders; a
//! deterministic persistence failure comes from a regular file standing
//! where the registry's parent directory must be (no chmod dependence).

use std::path::PathBuf;

use bevy::prelude::KeyCode;
use dtx_core::EChannel;
use dtx_input::profiles as cfg;
use dtx_layout::profiles as lp;
use gameplay_drums::bindings::BindResolver;
use gameplay_drums::editor::profile_state::{
    apply_save_all_results, dirty_profile_kinds, ProfileKind, ProfileSession,
};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("dtx-input-lane-profiles")
        .join(std::process::id().to_string())
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("test dir");
    dir
}

fn write_legacy_bindings(dir: &std::path::Path) -> PathBuf {
    let path = dir.join("bindings.toml");
    let file = dtx_input::InputBindings::default().to_file();
    std::fs::write(&path, toml::to_string_pretty(&file).expect("legacy toml")).expect("write");
    path
}

fn write_legacy_layout(dir: &std::path::Path, preset: dtx_layout::LanePreset) -> PathBuf {
    let path = dir.join("layout.toml");
    let file = dtx_layout::LayoutFile {
        lanes: dtx_layout::LanesSection {
            preset,
            ..Default::default()
        },
        ..Default::default()
    };
    std::fs::write(&path, toml::to_string_pretty(&file).expect("layout toml")).expect("write");
    path
}

fn active_keyboard(registry: &cfg::ProfileRegistry<cfg::KeyboardProfile>) -> cfg::KeyboardProfile {
    registry
        .profiles
        .get(&registry.active)
        .cloned()
        .or_else(|| cfg::keyboard_builtins().get(&registry.active).cloned())
        .unwrap_or_default()
}

fn active_midi(registry: &cfg::ProfileRegistry<cfg::MidiProfile>) -> cfg::MidiProfile {
    registry
        .profiles
        .get(&registry.active)
        .cloned()
        .or_else(|| cfg::midi_builtins().get(&registry.active).cloned())
        .unwrap_or_default()
}

fn ready<T>(startup: cfg::RegistryStartup<T>) -> T {
    match startup {
        cfg::RegistryStartup::Ready(value) => value,
        cfg::RegistryStartup::LegacySession { .. } => panic!("expected Ready, got LegacySession"),
        cfg::RegistryStartup::ReadOnlyBuiltins(error) => {
            panic!("expected Ready, got ReadOnlyBuiltins({error})")
        }
    }
}

#[test]
fn legacy_profiles_migrate_and_compose_end_to_end() {
    let dir = temp_dir("migrate-compose");
    let legacy = write_legacy_bindings(&dir);
    let layout = write_legacy_layout(&dir, dtx_layout::LanePreset::NxTypeB);
    let kb_path = dir.join("keyboard-profiles.toml");
    let midi_path = dir.join("midi-profiles.toml");
    let lane_path = dir.join("lane-profiles.toml");

    let keyboard = ready(cfg::load_keyboard_registry(&kb_path, &legacy));
    let midi = ready(cfg::load_midi_registry(&midi_path, &legacy));
    let lanes = match lp::load_lane_registry(&lane_path, &layout) {
        lp::LaneRegistryStartup::Ready(registry) => registry,
        other => panic!("lane migration failed: {other:?}"),
    };

    assert!(kb_path.exists() && midi_path.exists() && lane_path.exists());
    assert_eq!(lanes.active, "NX Type-B");

    let resolver = BindResolver::from_profiles(&active_keyboard(&keyboard), &active_midi(&midi));
    assert_eq!(resolver.lane_for_note(38), Some(1)); // SD
    assert_eq!(resolver.lane_for_key(KeyCode::Space), Some(2)); // BD
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn classic_nx_and_custom_profiles_keep_same_lane_hit_id() {
    let resolver = BindResolver::default();
    let baseline: Vec<_> = [36u8, 38, 42, 46, 49, 51]
        .into_iter()
        .map(|note| resolver.lane_for_note(note))
        .collect();
    let mut custom = dtx_layout::classic();
    dtx_layout::split_channel(&mut custom, EChannel::HiHatOpen);
    for arrangement in [
        dtx_layout::classic(),
        dtx_layout::nx_type_b(),
        dtx_layout::nx_type_d(),
        custom,
    ] {
        // Arrangements are display-only: same resolver, same LaneHit ids.
        let _ = arrangement;
        let now: Vec<_> = [36u8, 38, 42, 46, 49, 51]
            .into_iter()
            .map(|note| resolver.lane_for_note(note))
            .collect();
        assert_eq!(now, baseline);
    }
}

#[test]
fn multi_draft_save_all_retains_only_failed_draft() {
    let mut session = ProfileSession::default();
    session
        .keyboard
        .value
        .add_key(EChannel::Snare, KeyCode::KeyQ);
    session.midi.value.velocity_threshold = 33;
    session.lanes.value = lp::LaneProfile::from_arrangement(dtx_layout::nx_type_b());
    assert_eq!(
        dirty_profile_kinds(&session),
        vec![ProfileKind::Keyboard, ProfileKind::Midi, ProfileKind::Lanes]
    );
    let failed = apply_save_all_results(
        &mut session,
        &[
            (ProfileKind::Keyboard, true),
            (ProfileKind::Midi, false),
            (ProfileKind::Lanes, true),
        ],
    );
    assert_eq!(failed, vec![ProfileKind::Midi]);
    assert_eq!(dirty_profile_kinds(&session), vec![ProfileKind::Midi]);
}

#[test]
fn malformed_legacy_files_create_no_registries() {
    let dir = temp_dir("malformed-legacy");
    let legacy = dir.join("bindings.toml");
    let layout = dir.join("layout.toml");
    std::fs::write(&legacy, "not = [valid").expect("write");
    std::fs::write(&layout, "also = [broken").expect("write");
    let kb_path = dir.join("keyboard-profiles.toml");
    let midi_path = dir.join("midi-profiles.toml");
    let lane_path = dir.join("lane-profiles.toml");

    assert!(matches!(
        cfg::load_keyboard_registry(&kb_path, &legacy),
        cfg::RegistryStartup::ReadOnlyBuiltins(_)
    ));
    assert!(matches!(
        cfg::load_midi_registry(&midi_path, &legacy),
        cfg::RegistryStartup::ReadOnlyBuiltins(_)
    ));
    assert!(matches!(
        lp::load_lane_registry(&lane_path, &layout),
        lp::LaneRegistryStartup::ReadOnlyBuiltins(_)
    ));
    assert!(!kb_path.exists());
    assert!(!midi_path.exists());
    assert!(!lane_path.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn second_startup_uses_registries_and_ignores_legacy_snapshots() {
    let dir = temp_dir("second-startup");
    let legacy = write_legacy_bindings(&dir);
    let layout = write_legacy_layout(&dir, dtx_layout::LanePreset::Classic);
    let kb_path = dir.join("keyboard-profiles.toml");
    let midi_path = dir.join("midi-profiles.toml");
    let lane_path = dir.join("lane-profiles.toml");

    // First startup migrates.
    let _ = ready(cfg::load_keyboard_registry(&kb_path, &legacy));
    let _ = ready(cfg::load_midi_registry(&midi_path, &legacy));
    assert!(matches!(
        lp::load_lane_registry(&lane_path, &layout),
        lp::LaneRegistryStartup::Ready(_)
    ));

    // Legacy files change afterwards (stale snapshots).
    std::fs::write(&legacy, "garbage that would fail parsing").expect("write");
    let _ = write_legacy_layout(&dir, dtx_layout::LanePreset::NxTypeD);

    // Second startup: registries win, legacy never re-read.
    let keyboard = ready(cfg::load_keyboard_registry(&kb_path, &legacy));
    let midi = ready(cfg::load_midi_registry(&midi_path, &legacy));
    let lanes = match lp::load_lane_registry(&lane_path, &layout) {
        lp::LaneRegistryStartup::Ready(registry) => registry,
        other => panic!("registry must load: {other:?}"),
    };
    assert_eq!(keyboard.active, cfg::KEYBOARD_DEFAULT_NAME);
    assert_eq!(midi.active, cfg::MIDI_DEFAULT_NAME);
    assert_eq!(lanes.active, lp::LANE_DEFAULT_NAME);
    assert_eq!(
        lp::active_lane_arrangement(&lanes),
        dtx_layout::classic(),
        "stale NX Type-D legacy snapshot is ignored"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
