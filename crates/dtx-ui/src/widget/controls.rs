//! Minimal form controls for editor/settings UIs: Slider, Stepper, Toggle,
//! AnchorGrid. Pattern: caller spawns via the helpers, tags rows with its own
//! marker, then watches Changed<ControlValue>/<ControlBool>/<AnchorChoice>.
//! `ControlsPlugin` drives interaction → value updates + visuals.

use bevy::prelude::*;

use crate::theme::Theme;

/// Continuous value carried by Slider and Stepper entities.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ControlValue(pub f32);

/// Boolean carried by Toggle entities.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ControlBool(pub bool);

#[derive(Component, Debug, Clone, Copy)]
pub struct Slider {
    pub min: f32,
    pub max: f32,
}

/// Slider child: filled track portion.
#[derive(Component)]
pub struct SliderFill;

#[derive(Component, Debug, Clone, Copy)]
pub struct Stepper {
    pub step: f32,
    pub min: f32,
    pub max: f32,
    /// Decimal places shown on the label.
    pub decimals: usize,
}

/// Stepper child button: -1 or +1.
#[derive(Component, Debug, Clone, Copy)]
pub struct StepperBtn(pub i8);

/// Stepper child: the numeric label.
#[derive(Component)]
pub struct StepperLabel;

#[derive(Component, Debug, Clone, Copy)]
pub struct Toggle;

/// Toggle child: the knob square.
#[derive(Component)]
pub struct ToggleKnob;

/// Currently dragged slider (one at a time).
#[derive(Resource, Debug, Default)]
pub struct ActiveSlider(pub Option<Entity>);

pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveSlider>().add_systems(
            Update,
            (
                drive_sliders,
                drive_steppers,
                drive_toggles,
                paint_slider_fill,
                paint_stepper_labels,
                paint_toggles,
            ),
        );
    }
}

pub const SLIDER_WIDTH: f32 = 110.0;
pub const SLIDER_HEIGHT: f32 = 14.0;

/// Spawn a slider (track + fill). Returns the slider entity (carries
/// `Slider` + `ControlValue` + `Button` for Interaction).
pub fn spawn_slider(
    p: &mut ChildSpawnerCommands,
    theme: &Theme,
    spec: Slider,
    value: f32,
) -> Entity {
    p.spawn((
        spec,
        ControlValue(value),
        Button,
        Node {
            width: Val::Px(SLIDER_WIDTH),
            height: Val::Px(SLIDER_HEIGHT),
            padding: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
        children![(
            SliderFill,
            Node {
                width: Val::Percent(50.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(theme.accent),
        )],
    ))
    .id()
}

/// Spawn a stepper row: `[-] value [+]`. Returns the stepper entity.
pub fn spawn_stepper(
    p: &mut ChildSpawnerCommands,
    theme: &Theme,
    spec: Stepper,
    value: f32,
) -> Entity {
    p.spawn((
        spec,
        ControlValue(value),
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            ..default()
        },
        children![
            (
                StepperBtn(-1),
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.14, 0.14, 0.18)),
                children![(
                    Text::new("-"),
                    Theme::font(12.0),
                    TextColor(theme.text_primary)
                )],
            ),
            (
                StepperLabel,
                Text::new(""),
                Theme::font(12.0),
                TextColor(theme.text_primary),
                Node {
                    min_width: Val::Px(44.0),
                    ..default()
                },
            ),
            (
                StepperBtn(1),
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.14, 0.14, 0.18)),
                children![(
                    Text::new("+"),
                    Theme::font(12.0),
                    TextColor(theme.text_primary)
                )],
            ),
        ],
    ))
    .id()
}

/// Spawn a toggle. Returns the toggle entity.
pub fn spawn_toggle(p: &mut ChildSpawnerCommands, theme: &Theme, value: bool) -> Entity {
    p.spawn((
        Toggle,
        ControlBool(value),
        Button,
        Node {
            width: Val::Px(30.0),
            height: Val::Px(16.0),
            padding: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
        children![(
            ToggleKnob,
            Node {
                width: Val::Px(12.0),
                height: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(theme.accent),
        )],
    ))
    .id()
}

/// Pure: slider value from a cursor x within the track rect.
pub fn slider_value_at(
    min: f32,
    max: f32,
    track_left: f32,
    track_width: f32,
    cursor_x: f32,
) -> f32 {
    if track_width <= f32::EPSILON {
        return min;
    }
    let frac = ((cursor_x - track_left) / track_width).clamp(0.0, 1.0);
    min + frac * (max - min)
}

/// Pure: stepper arithmetic (shift = ×10 step).
pub fn stepper_next(value: f32, dir: i8, step: f32, big: bool, min: f32, max: f32) -> f32 {
    let s = if big { step * 10.0 } else { step };
    (value + s * dir as f32).clamp(min, max)
}

fn drive_sliders(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut active: ResMut<ActiveSlider>,
    mut sliders: Query<(
        Entity,
        &Slider,
        &mut ControlValue,
        &Interaction,
        &ComputedNode,
        &bevy::ui::UiGlobalTransform,
    )>,
) {
    if !buttons.pressed(MouseButton::Left) {
        active.0 = None;
    } else if buttons.just_pressed(MouseButton::Left) {
        for (e, _, _, interaction, _, _) in &sliders {
            if *interaction == Interaction::Pressed {
                active.0 = Some(e);
                break;
            }
        }
    }
    let Some(active_e) = active.0 else { return };
    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    if let Ok((_, spec, mut value, _, cn, gt)) = sliders.get_mut(active_e) {
        let inv = cn.inverse_scale_factor();
        let center = gt.translation * inv;
        let size = cn.size() * inv;
        let left = center.x - size.x / 2.0;
        let next = slider_value_at(spec.min, spec.max, left, size.x, cursor.x);
        if (next - value.0).abs() > f32::EPSILON {
            value.0 = next;
        }
    }
}

fn drive_steppers(
    keys: Res<ButtonInput<KeyCode>>,
    btns: Query<(&StepperBtn, &Interaction, &ChildOf), Changed<Interaction>>,
    mut steppers: Query<(&Stepper, &mut ControlValue)>,
) {
    let big = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    for (btn, interaction, child_of) in &btns {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Ok((spec, mut value)) = steppers.get_mut(child_of.parent()) {
            value.0 = stepper_next(value.0, btn.0, spec.step, big, spec.min, spec.max);
        }
    }
}

#[allow(clippy::type_complexity)]
fn drive_toggles(
    mut toggles: Query<(&Interaction, &mut ControlBool), (With<Toggle>, Changed<Interaction>)>,
) {
    for (interaction, mut v) in &mut toggles {
        if *interaction == Interaction::Pressed {
            v.0 = !v.0;
        }
    }
}

fn paint_slider_fill(
    sliders: Query<(&Slider, &ControlValue, &Children), Changed<ControlValue>>,
    mut fills: Query<&mut Node, With<SliderFill>>,
) {
    for (spec, value, children) in &sliders {
        let frac = ((value.0 - spec.min) / (spec.max - spec.min)).clamp(0.0, 1.0);
        for child in children.iter() {
            if let Ok(mut node) = fills.get_mut(child) {
                node.width = Val::Percent(frac * 100.0);
            }
        }
    }
}

fn paint_stepper_labels(
    steppers: Query<(&Stepper, &ControlValue, &Children), Changed<ControlValue>>,
    mut labels: Query<&mut Text, With<StepperLabel>>,
) {
    for (spec, value, children) in &steppers {
        for child in children.iter() {
            if let Ok(mut text) = labels.get_mut(child) {
                text.0 = format!("{:.*}", spec.decimals, value.0);
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn paint_toggles(
    toggles: Query<(&ControlBool, &Children), (With<Toggle>, Changed<ControlBool>)>,
    mut knobs: Query<(&mut Node, &mut BackgroundColor), With<ToggleKnob>>,
) {
    for (v, children) in &toggles {
        for child in children.iter() {
            if let Ok((mut node, mut bg)) = knobs.get_mut(child) {
                node.margin = if v.0 {
                    UiRect::left(Val::Px(14.0))
                } else {
                    UiRect::left(Val::Px(0.0))
                };
                bg.0 = if v.0 {
                    Color::srgb(0.0, 0.831, 0.667)
                } else {
                    Color::srgba(1.0, 1.0, 1.0, 0.3)
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_value_maps_cursor_to_range() {
        assert_eq!(slider_value_at(0.0, 10.0, 100.0, 200.0, 100.0), 0.0);
        assert_eq!(slider_value_at(0.0, 10.0, 100.0, 200.0, 300.0), 10.0);
        assert_eq!(slider_value_at(0.0, 10.0, 100.0, 200.0, 200.0), 5.0);
        assert_eq!(slider_value_at(0.0, 10.0, 100.0, 200.0, 0.0), 0.0); // clamped
    }

    #[test]
    fn stepper_steps_and_clamps() {
        assert_eq!(stepper_next(5.0, 1, 1.0, false, 0.0, 10.0), 6.0);
        assert_eq!(stepper_next(5.0, 1, 1.0, true, 0.0, 10.0), 10.0); // 5+10 clamped
        assert_eq!(stepper_next(0.5, -1, 1.0, false, 0.0, 10.0), 0.0);
    }
}
