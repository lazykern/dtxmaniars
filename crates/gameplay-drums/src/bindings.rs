//! Bind-resolution re-exports + schedule wiring.
//!
//! The resolver moved to `dtx_input::resolver` (menu-nav extraction,
//! 2026-07-15 spec). This module keeps the `crate::bindings::…` paths alive
//! and owns the *when*: dtx-input cannot reference `game_shell::AppState`.

use bevy::prelude::*;

pub use dtx_input::resolver::{
    active_keyboard_profile, active_midi_profile, apply_live_bindings, compose_bindings,
    keyboard_registry_path, midi_registry_path, reload_profiles, ActiveInputProfiles, BindResolver,
    LiveBindings,
};

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<BindResolver>()
        .init_resource::<LiveBindings>()
        .init_resource::<ActiveInputProfiles>()
        // Seeded at boot too: pads navigate menus before any Performance enter.
        .add_systems(Startup, reload_profiles)
        .add_systems(OnEnter(game_shell::AppState::Performance), reload_profiles)
        .add_systems(
            Update,
            apply_live_bindings
                .run_if(resource_changed::<LiveBindings>)
                .run_if(in_state(game_shell::AppState::Performance)),
        );
}
