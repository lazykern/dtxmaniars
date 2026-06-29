//! Cached HUD text updates — skip rewriting when values unchanged.

use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct HudDisplayCache {
    pub score_text: Option<String>,
    pub combo_text: Option<String>,
    pub counters_text: Option<String>,
}

pub fn set_text_if_changed(text: &mut Text, cache: &mut Option<String>, new_text: String) {
    if cache.as_ref() != Some(&new_text) {
        *text = Text::new(new_text.clone());
        *cache = Some(new_text);
    }
}
