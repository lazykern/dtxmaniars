//! Verifies all 8 stages are defined and unique.
//!
//! CStage.cs (BocuD) has 8 EStage values: Startup, Title, Config,
//! SongSelect, SongLoading, Performance, Result, ChangeSkin, End.
//! End is included for the boot exit path.

use game_shell::AppState;

#[test]
fn all_stages_present_and_distinct() {
    let all: [AppState; 9] = [
        AppState::Startup,
        AppState::Title,
        AppState::Config,
        AppState::SongSelect,
        AppState::SongLoading,
        AppState::Performance,
        AppState::Result,
        AppState::ChangeSkin,
        AppState::End,
    ];
    let unique: std::collections::HashSet<_> = all.iter().collect();
    assert_eq!(unique.len(), 9, "AppState must have 9 distinct variants");
}

#[test]
fn default_is_startup() {
    // ADR-0001: Drums-first MVP. AppState::Startup is the default.
    let s = AppState::default();
    assert!(matches!(s, AppState::Startup));
}

#[test]
fn egamemode_default_is_drums() {
    use game_shell::EGameMode;
    let m = EGameMode::default();
    assert!(matches!(m, EGameMode::Drums));
}

#[test]
fn egamemode_cycles_drums_guitar() {
    use game_shell::EGameMode;
    assert_eq!(EGameMode::Drums.next(), EGameMode::Guitar);
    assert_eq!(EGameMode::Guitar.next(), EGameMode::Drums);
}
