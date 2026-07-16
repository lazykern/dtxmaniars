//! Application navigation contexts. The top of the stack owns semantic input.

use bevy::prelude::*;

/// Which UI surface owns semantic input. Top of the stack wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavContext {
    /// Home (title) menu.
    Home,
    /// Song select, songs column focused.
    SongSelectSongs,
    /// Song select, difficulty column focused.
    SongSelectDifficulty,
    /// Song Ready surface, browsing the five cards.
    SongReadyBrowse,
    /// Song Ready surface, editing a card value.
    SongReadyEdit,
    /// Chart/audio load in progress (Back cancels).
    SongLoading,
    /// Practice Setup, settings list focused (or single-column layout).
    PracticeSetupSettings,
    /// Practice Setup, preview transport focused.
    PracticeSetupPreview,
    /// Pause overlay (normal or practice) during a performance.
    PauseMenu,
    /// Post-play results screen.
    Results,
    /// Settings, category bar focused.
    SettingsTabs,
    /// Settings, row list focused.
    SettingsRows,
    /// Settings, editing a row value.
    SettingsEdit,
    /// A modal dialog exclusively owns navigation.
    ModalDialog,
    /// Binding capture / calibration owns raw input exclusively.
    BindingCapture,
    /// The chart-backed full layout editor (Customize overlay).
    LayoutEditor,
    /// Live judged gameplay: menu verbs are dropped, lanes judge.
    LiveGameplay,
}

impl NavContext {
    /// Edit-type contexts translate NavigateLeft/Right into Decrease/Increase.
    pub fn is_edit(self) -> bool {
        matches!(
            self,
            NavContext::SongReadyEdit
                | NavContext::SettingsEdit
                | NavContext::PracticeSetupSettings
        )
    }

    /// Contexts that own raw input exclusively: no menu routing at all.
    pub fn exclusive(self) -> bool {
        matches!(self, NavContext::BindingCapture)
    }
}

/// Stack of active contexts; screens push in OnEnter/overlay-open and pop in
/// OnExit/overlay-close. `push` moves an already-present context to the top.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct NavContextStack(Vec<NavContext>);

impl NavContextStack {
    /// The context that owns input, if any.
    pub fn top(&self) -> Option<NavContext> {
        self.0.last().copied()
    }
    /// Put `ctx` on top (idempotent: an existing entry moves up).
    pub fn push(&mut self, ctx: NavContext) {
        self.0.retain(|c| *c != ctx);
        self.0.push(ctx);
    }
    /// Remove `ctx` wherever it sits (screens pop in OnExit; overlay order
    /// must not corrupt the stack).
    pub fn pop(&mut self, ctx: NavContext) {
        self.0.retain(|c| *c != ctx);
    }
    /// Drop every context (used on hard resets).
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_of_stack_owns_input() {
        let mut stack = NavContextStack::default();
        assert_eq!(stack.top(), None);
        stack.push(NavContext::Home);
        stack.push(NavContext::ModalDialog);
        assert_eq!(stack.top(), Some(NavContext::ModalDialog));
        stack.pop(NavContext::ModalDialog);
        assert_eq!(stack.top(), Some(NavContext::Home));
    }

    #[test]
    fn pop_removes_the_named_context_even_if_not_top() {
        let mut stack = NavContextStack::default();
        stack.push(NavContext::SongSelectSongs);
        stack.push(NavContext::ModalDialog);
        stack.pop(NavContext::SongSelectSongs);
        assert_eq!(stack.top(), Some(NavContext::ModalDialog));
    }

    #[test]
    fn push_is_idempotent_per_context() {
        let mut stack = NavContextStack::default();
        stack.push(NavContext::Home);
        stack.push(NavContext::Home);
        stack.pop(NavContext::Home);
        assert_eq!(stack.top(), None);
    }
}
