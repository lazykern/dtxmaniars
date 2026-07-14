use bevy::prelude::{Commands, Component, Entity, Or, Query, Res, Resource, With, World};
use bevy::text::{FontSize, TextFont};
use dtx_config::TextScale;

use crate::{AccessibilityPolicy, Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypographyRole {
    Display,
    Title,
    Heading,
    Body,
    Label,
    Hint,
    Hud,
}

/// Marks player-facing text with a semantic size that follows live settings.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticText(pub TypographyRole);

/// Player-facing text whose authored size should scale proportionally.
/// Prefer [`SemanticText`] for ordinary labels; this marker is for deliberate
/// display sizes such as a result rank or compact reference-space HUD text.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AccessibleText;

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct AccessibleTextBaseline(f32);

type AccessibleTextQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut TextFont,
        Option<&'static SemanticText>,
        Option<&'static AccessibleTextBaseline>,
    ),
    Or<(With<SemanticText>, With<AccessibleText>)>,
>;

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Typography;

impl Typography {
    pub const fn base_px(self, role: TypographyRole) -> f32 {
        match role {
            TypographyRole::Display => 48.0,
            TypographyRole::Title => 36.0,
            TypographyRole::Heading => 24.0,
            TypographyRole::Body => 16.0,
            TypographyRole::Label => 16.0,
            TypographyRole::Hint => 14.0,
            TypographyRole::Hud => 32.0,
        }
    }

    pub fn px(self, role: TypographyRole, scale: TextScale) -> f32 {
        (self.base_px(role) * scale.multiplier()).max(14.0)
    }

    pub fn font(self, role: TypographyRole, policy: AccessibilityPolicy) -> TextFont {
        Theme::font(self.px(role, policy.text_scale()))
    }
}

pub(crate) fn apply_semantic_typography(
    mut commands: Commands,
    typography: Res<Typography>,
    policy: Res<AccessibilityPolicy>,
    mut text: AccessibleTextQuery,
) {
    for (entity, mut font, semantic, baseline) in &mut text {
        let current_px = match font.font_size {
            FontSize::Px(px) => px,
            _ => continue,
        };
        let base_px = baseline.map_or(current_px, |baseline| baseline.0);
        if baseline.is_none() {
            commands.queue(move |world: &mut World| {
                if let Ok(mut entity) = world.get_entity_mut(entity) {
                    entity.insert(AccessibleTextBaseline(base_px));
                }
            });
        }
        let px = semantic.map_or(
            (base_px * policy.text_multiplier()).max(base_px),
            |semantic| typography.px(semantic.0, policy.text_scale()),
        );
        font.font_size = FontSize::Px(px);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpacingRole {
    Xs,
    Sm,
    Md,
    Lg,
    Xl,
}

impl SpacingRole {
    pub const fn px(self) -> f32 {
        match self {
            Self::Xs => 4.0,
            Self::Sm => 8.0,
            Self::Md => 16.0,
            Self::Lg => 24.0,
            Self::Xl => 32.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateMarker {
    Focus,
    Selected,
    Error,
    Destructive,
    Success,
    Disabled,
}

impl StateMarker {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Focus => ">",
            Self::Selected => "✓",
            Self::Error => "!",
            Self::Destructive => "DELETE",
            Self::Success => "OK",
            Self::Disabled => "—",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionTone {
    Focus,
    Selected,
    Error,
    Destructive,
    Success,
    Disabled,
}

impl InteractionTone {
    pub const ALL: [Self; 6] = [
        Self::Focus,
        Self::Selected,
        Self::Error,
        Self::Destructive,
        Self::Success,
        Self::Disabled,
    ];

    pub const fn marker(self) -> StateMarker {
        match self {
            Self::Focus => StateMarker::Focus,
            Self::Selected => StateMarker::Selected,
            Self::Error => StateMarker::Error,
            Self::Destructive => StateMarker::Destructive,
            Self::Success => StateMarker::Success,
            Self::Disabled => StateMarker::Disabled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;

    #[test]
    fn semantic_text_scales_and_never_drops_below_minimum() {
        let typography = Typography;
        assert_eq!(
            typography.px(TypographyRole::Body, dtx_config::TextScale::Large),
            20.0
        );
        assert!(typography.px(TypographyRole::Hint, dtx_config::TextScale::Standard) >= 14.0);
        assert_eq!(
            typography.px(TypographyRole::Hud, dtx_config::TextScale::XLarge),
            48.0
        );
    }

    #[test]
    fn interaction_tones_always_have_shape_or_text_markers() {
        for tone in InteractionTone::ALL {
            assert!(!tone.marker().label().is_empty());
        }
    }

    #[test]
    fn semantic_text_reacts_to_live_policy_changes() {
        let mut app = App::new();
        app.init_resource::<Typography>()
            .init_resource::<AccessibilityPolicy>()
            .add_systems(Update, apply_semantic_typography);
        let entity = app
            .world_mut()
            .spawn((TextFont::default(), SemanticText(TypographyRole::Body)))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<TextFont>(entity).unwrap().font_size,
            FontSize::Px(16.0)
        );

        app.world_mut().insert_resource(AccessibilityPolicy::from(
            &dtx_config::AccessibilityConfig {
                text_scale: dtx_config::TextScale::XLarge,
                ..Default::default()
            },
        ));
        app.update();
        assert_eq!(
            app.world().get::<TextFont>(entity).unwrap().font_size,
            FontSize::Px(24.0)
        );
    }

    #[test]
    fn deliberate_display_size_scales_proportionally() {
        let mut app = App::new();
        app.init_resource::<Typography>()
            .insert_resource(AccessibilityPolicy::from(
                &dtx_config::AccessibilityConfig {
                    text_scale: dtx_config::TextScale::XLarge,
                    ..Default::default()
                },
            ))
            .add_systems(Update, apply_semantic_typography);
        let entity = app
            .world_mut()
            .spawn((Theme::font(160.0), AccessibleText))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<TextFont>(entity).unwrap().font_size,
            FontSize::Px(240.0)
        );
    }

    #[test]
    fn baseline_insert_tolerates_entity_despawn_before_commands_apply() {
        let mut app = App::new();
        app.init_resource::<Typography>()
            .init_resource::<AccessibilityPolicy>();
        let entity = app
            .world_mut()
            .spawn((TextFont::default(), SemanticText(TypographyRole::Body)))
            .id();

        let mut system = IntoSystem::into_system(apply_semantic_typography);
        system.initialize(app.world_mut());
        assert!(system.run((), app.world_mut()).is_ok());
        app.world_mut().despawn(entity);
        system.apply_deferred(app.world_mut());

        assert!(app.world().get_entity(entity).is_err());
    }
}
