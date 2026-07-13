use bevy::prelude::Component;

pub use super::action_button::DialogAction;

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct ModalDialog {
    actions: Vec<DialogAction>,
    focused: usize,
}

impl ModalDialog {
    pub fn new(actions: Vec<DialogAction>) -> Self {
        let focused = actions
            .iter()
            .rposition(|action| !matches!(action, DialogAction::Destructive))
            .unwrap_or_default();
        Self { actions, focused }
    }

    pub fn actions(&self) -> &[DialogAction] {
        &self.actions
    }

    pub fn focused_action(&self) -> Option<DialogAction> {
        self.actions.get(self.focused).copied()
    }

    pub fn step_focus(mut self, delta: i32) -> Self {
        if !self.actions.is_empty() {
            self.focused =
                (self.focused as i32 + delta).rem_euclid(self.actions.len() as i32) as usize;
        }
        self
    }

    pub fn cancel_action(&self) -> Option<DialogAction> {
        self.actions
            .iter()
            .copied()
            .find(|action| *action == DialogAction::Cancel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_focus_skips_destructive_actions_and_wraps_inside_dialog() {
        let dialog = ModalDialog::new(vec![
            DialogAction::Cancel,
            DialogAction::Destructive,
            DialogAction::Confirm,
        ]);
        assert_eq!(dialog.focused_action(), Some(DialogAction::Confirm));
        assert_eq!(
            dialog.step_focus(1).focused_action(),
            Some(DialogAction::Cancel)
        );
    }
}
