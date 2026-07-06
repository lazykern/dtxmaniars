//! Runtime HUD widget placement: per-widget container nodes driven by a
//! `WidgetLayouts` resource (code defaults ⊕ layout.toml `[scene.gameplay]`).
//!
//! Each HUD widget's children are parented to a `WidgetContainer` node placed
//! absolutely at ref-origin (0,0), full-size. The container's `left/top` is the
//! widget's resolved offset·scale, so moving it translates the whole widget as
//! one unit; at the default offset (0,0) every widget lands where it did before
//! this system existed (parity). Anchor/origin are modeled for the editor
//! (plan 3); with the current uniform scale the applied position reduces to
//! `offset` (screen-space) — see `apply_widget_layout`.

use std::collections::HashMap;

use bevy::prelude::*;
use dtx_layout::{WidgetInstance, WidgetKind};
use game_shell::AppState;

use crate::layout::PlayfieldLayout;

/// Marks a per-widget container node (parent of one widget's children).
#[derive(Component, Debug, Clone, Copy)]
pub struct WidgetContainer(pub WidgetKind);

/// Resolved placement for every widget kind (defaults ⊕ file).
#[derive(Resource, Debug, Clone)]
pub struct WidgetLayouts(pub HashMap<WidgetKind, WidgetInstance>);

impl Default for WidgetLayouts {
    fn default() -> Self {
        Self(dtx_layout::SceneSection::default().resolve())
    }
}

impl WidgetLayouts {
    pub fn get(&self, kind: WidgetKind) -> &WidgetInstance {
        self.0.get(&kind).expect("WidgetLayouts missing a kind")
    }
}

/// Whether a widget is visible in the current mode (practice vs play).
pub fn widget_visible(inst: &WidgetInstance, practice: bool) -> bool {
    if practice {
        inst.visible_practice
    } else {
        inst.visible_play
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<WidgetLayouts>()
        .add_systems(Startup, load_widget_layouts)
        .add_systems(
            Update,
            apply_widget_layout
                .run_if(in_state(AppState::Performance))
                .run_if(
                    resource_changed::<WidgetLayouts>.or_else(resource_changed::<PlayfieldLayout>),
                ),
        );
}

/// Load `[scene.gameplay]` from layout.toml at startup (defaults on absence).
fn load_widget_layouts(mut layouts: ResMut<WidgetLayouts>) {
    let file = dtx_layout::load(&dtx_layout::default_path());
    layouts.0 = file.scene.resolve();
}

/// Position + z-order + visibility for every widget container. Runs on layout
/// or arrangement change. Position = offset·scale (screen-space, uniform scale);
/// full anchor-aware resolution is a plan-3 concern where variable scale matters.
fn apply_widget_layout(
    layouts: Res<WidgetLayouts>,
    practice: Option<Res<crate::practice::PracticeSession>>,
    pfl: Res<PlayfieldLayout>,
    mut containers: Query<(
        &WidgetContainer,
        &mut Node,
        Option<&mut ZIndex>,
        &mut Visibility,
    )>,
) {
    let is_practice = practice.is_some();
    let scale = pfl.scale;
    for (container, mut node, z, mut vis) in &mut containers {
        let inst = layouts.get(container.0);
        node.left = Val::Px(inst.offset.0 * scale);
        node.top = Val::Px(inst.offset.1 * scale);
        if let Some(mut z) = z {
            *z = ZIndex(inst.z);
        }
        *vis = if widget_visible(inst, is_practice) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_layout::default_instance;

    #[test]
    fn default_layouts_cover_all_kinds() {
        let l = WidgetLayouts::default();
        for k in WidgetKind::ALL {
            assert_eq!(*l.get(k), default_instance(k));
        }
    }

    #[test]
    fn visibility_respects_mode() {
        let transport = default_instance(WidgetKind::PracticeTransport);
        assert!(!widget_visible(&transport, false));
        assert!(widget_visible(&transport, true));
        let combo = default_instance(WidgetKind::Combo);
        assert!(widget_visible(&combo, false));
        assert!(widget_visible(&combo, true));
    }
}
