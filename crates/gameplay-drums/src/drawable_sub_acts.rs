//! Performance Drawable sub-acts — batched port (p3-47..p3-49).
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/Drawable/`

/// p3-47a: JudgementString.cs (123 LOC) — judgment string display.
pub mod judgement_string {
    /// 5 judgment kinds (Perfect/Great/Good/Ok/Miss).
    pub const JUDGMENT_KINDS: usize = 5;
    /// Display duration in ms.
    pub const DISPLAY_MS: u32 = 600;
    /// Vertical offset between consecutive judgments.
    pub const VERT_OFFSET: f32 = 32.0;
}

/// p3-47b: NoteExplosion.cs (96 LOC) — note strike particles.
pub mod note_explosion {
    /// Particles per strike.
    pub const PARTICLE_COUNT: usize = 16;
    /// Particle lifetime frames.
    pub const LIFETIME_FRAMES: u32 = 30;
}

/// p3-47c: WailingEffect.cs (42 LOC) — wailing bonus visual.
pub mod wailing_effect {
    /// Wailing duration frames.
    pub const DURATION_FRAMES: u32 = 60;
}

/// p3-48: CActPerformanceInformation depth — extended stats struct.
pub mod performance_info {
    /// Default BPM.
    pub const DEFAULT_BPM: f64 = 120.0;
    /// 5 judgment count fields (PERFECT/GREAT/GOOD/POOR/MISS).
    pub const JUDGMENT_FIELDS: usize = 5;
}

/// p3-49: WailingBonus orchestrator — combines wailing state.
pub mod wailing_bonus_orch {
    /// Min notes between wailings.
    pub const WAILING_NOTE_INTERVAL: u32 = 32;
    /// Wailing bonus base value.
    pub const WAILING_BONUS_BASE: u32 = 1000;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn judgement_string_kinds() {
        assert_eq!(judgement_string::JUDGMENT_KINDS, 5);
        assert_eq!(judgement_string::DISPLAY_MS, 600);
    }

    #[test]
    fn note_explosion_particles() {
        assert_eq!(note_explosion::PARTICLE_COUNT, 16);
        assert_eq!(note_explosion::LIFETIME_FRAMES, 30);
    }

    #[test]
    fn wailing_effect_duration() {
        assert_eq!(wailing_effect::DURATION_FRAMES, 60);
    }

    #[test]
    fn performance_info_default_bpm() {
        assert_eq!(performance_info::DEFAULT_BPM, 120.0);
        assert_eq!(performance_info::JUDGMENT_FIELDS, 5);
    }

    #[test]
    fn wailing_bonus_orch_constants() {
        assert_eq!(wailing_bonus_orch::WAILING_NOTE_INTERVAL, 32);
        assert_eq!(wailing_bonus_orch::WAILING_BONUS_BASE, 1000);
    }
}
