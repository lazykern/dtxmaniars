use std::convert::Infallible;

use bevy::prelude::Resource;
use dtx_config::{
    PracticePrerollPreset, PracticePresetConfig, PracticeSnapPreset, PracticeTrainerPreset,
    RampPreset,
};
use game_shell::{PracticePreRoll, PracticeRequest, PracticeSeed};

use super::session::{LoopRegion, PracticeSession, PrerollSetting, RampConfig, RATE_MAX, RATE_MIN};
use crate::timeline::{ChipTimeline, SnapDivisor};

const RAMP_TEMPO_GAP: f32 = 0.05;
const RAMP_STEP_MIN: f32 = 0.05;
const RAMP_STEP_MAX: f32 = 0.25;
const RAMP_THRESHOLD_MIN: f32 = 50.0;
const RAMP_THRESHOLD_MAX: f32 = 100.0;
const RAMP_SUCCESSES_MIN: u8 = 1;
const RAMP_SUCCESSES_MAX: u8 = 3;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PracticeTrainerMode {
    #[default]
    Off,
    Wait,
    Ramp,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PracticeDraftSource {
    #[default]
    WholeSong,
    LastUsed,
    Recommended,
    Saved(u64),
    Custom,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PracticeTrainerDraft {
    pub mode: PracticeTrainerMode,
    pub ramp_config: RampConfig,
}

#[derive(Resource, Debug, Clone, PartialEq)]
pub struct PracticeDraft {
    pub source: PracticeDraftSource,
    pub loop_region: Option<LoopRegion>,
    pub user_tempo: f32,
    pub snap: SnapDivisor,
    pub preroll: PrerollSetting,
    pub count_in: bool,
    pub trainer: PracticeTrainerDraft,
}

impl Default for PracticeDraft {
    fn default() -> Self {
        let session = PracticeSession::default();
        Self {
            source: PracticeDraftSource::WholeSong,
            loop_region: session.transport.loop_region,
            user_tempo: session.transport.user_tempo,
            snap: session.transport.snap,
            preroll: session.transport.preroll,
            count_in: session.transport.metronome,
            trainer: PracticeTrainerDraft::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValidatedDraft {
    pub draft: PracticeDraft,
    pub warning: Option<String>,
}

impl PracticeDraft {
    pub fn trainer_mode(&self) -> PracticeTrainerMode {
        self.trainer.mode
    }

    pub fn set_trainer_mode(&mut self, mode: PracticeTrainerMode) {
        self.trainer.mode = mode;
    }

    pub fn from_request(request: &PracticeRequest) -> Self {
        match request.seed {
            PracticeSeed::Manual => Self::default(),
            PracticeSeed::Recommended(recommendation) => {
                if !recommendation.has_valid_loop() {
                    return Self::default();
                }
                let mut draft = Self {
                    source: PracticeDraftSource::Recommended,
                    loop_region: Some(LoopRegion {
                        start_ms: recommendation.loop_start_ms,
                        end_ms: recommendation.loop_end_ms,
                    }),
                    user_tempo: recommendation.initial_tempo,
                    ..Default::default()
                };
                draft.preroll = match recommendation.pre_roll {
                    PracticePreRoll::OneBar => PrerollSetting::OneBar,
                };
                draft
            }
        }
    }

    pub fn from_preset(id: u64, config: &PracticePresetConfig) -> Self {
        Self {
            source: PracticeDraftSource::Saved(id),
            ..Self::from(config)
        }
    }

    pub fn validate(&self, timeline: &ChipTimeline) -> Result<ValidatedDraft, Infallible> {
        let mut draft = self.clone();
        let mut warning = None;

        draft.user_tempo = finite_clamp(draft.user_tempo, RATE_MIN, RATE_MAX, 1.0);
        let ramp = &mut draft.trainer.ramp_config;
        ramp.start_tempo = finite_clamp(
            ramp.start_tempo,
            RATE_MIN,
            RATE_MAX - RAMP_TEMPO_GAP,
            super::session::RAMP_START_DEFAULT,
        );
        let minimum_target = ramp.start_tempo + RAMP_TEMPO_GAP;
        let target = finite_clamp(
            ramp.target_tempo,
            RATE_MIN,
            RATE_MAX,
            super::session::RAMP_TARGET_DEFAULT.max(minimum_target),
        );
        ramp.target_tempo = if target + f32::EPSILON < minimum_target {
            minimum_target
        } else {
            target
        };
        ramp.step = finite_clamp(
            ramp.step,
            RAMP_STEP_MIN,
            RAMP_STEP_MAX,
            super::session::RAMP_STEP_DEFAULT,
        );
        ramp.threshold_pct = finite_clamp(
            ramp.threshold_pct,
            RAMP_THRESHOLD_MIN,
            RAMP_THRESHOLD_MAX,
            super::session::RAMP_THRESHOLD_DEFAULT,
        );
        ramp.required_successes = ramp
            .required_successes
            .clamp(RAMP_SUCCESSES_MIN, RAMP_SUCCESSES_MAX);

        if let Some(region) = draft.loop_region {
            let chart_end = timeline.end_ms.max(0);
            let mut start_ms = region.start_ms.clamp(0, chart_end);
            let mut end_ms = region.end_ms.clamp(0, chart_end);
            if start_ms > end_ms {
                std::mem::swap(&mut start_ms, &mut end_ms);
            }
            if start_ms == end_ms {
                draft.loop_region = None;
                draft.source = PracticeDraftSource::WholeSong;
                warning = Some(
                    "Loop bounds could not form a positive region; using whole song".to_owned(),
                );
            } else {
                draft.loop_region = Some(LoopRegion { start_ms, end_ms });
            }
        }

        Ok(ValidatedDraft { draft, warning })
    }

    pub fn apply_to_session(&self, session: &mut PracticeSession) {
        if session.transport.loop_region != self.loop_region {
            session.current_attempt_lane_diag.clear();
            session.lane_diag.clear();
        }
        session.transport.loop_region = self.loop_region;
        session.transport.user_tempo = self.user_tempo;
        session.transport.snap = self.snap;
        session.transport.preroll = self.preroll;
        session.transport.metronome = self.count_in;
        session.transport.scrub_cursor_ms = None;
        session.trainer.ramp_config = self.trainer.ramp_config;
        match self.trainer.mode {
            PracticeTrainerMode::Off => session.trainer.disable(),
            PracticeTrainerMode::Wait => session.trainer.enable_wait(true),
            PracticeTrainerMode::Ramp => session.trainer.arm_ramp(),
        }
    }
}

impl From<&PracticeSession> for PracticeDraft {
    fn from(session: &PracticeSession) -> Self {
        Self {
            source: PracticeDraftSource::Custom,
            loop_region: session.transport.loop_region,
            user_tempo: session.transport.user_tempo,
            snap: session.transport.snap,
            preroll: session.transport.preroll,
            count_in: session.transport.metronome,
            trainer: PracticeTrainerDraft {
                mode: session.trainer.mode,
                ramp_config: session.trainer.ramp_config,
            },
        }
    }
}

impl From<&PracticePresetConfig> for PracticeDraft {
    fn from(config: &PracticePresetConfig) -> Self {
        let loop_region = match (config.loop_start_ms, config.loop_end_ms) {
            (Some(start_ms), Some(end_ms)) => Some(LoopRegion { start_ms, end_ms }),
            _ => None,
        };
        let snap = match config.snap {
            PracticeSnapPreset::Bar => SnapDivisor::Bar,
            PracticeSnapPreset::Beat => SnapDivisor::Beat,
            PracticeSnapPreset::HalfBeat => SnapDivisor::Quarter,
        };
        let preroll = match config.preroll {
            PracticePrerollPreset::OneBar => PrerollSetting::OneBar,
            PracticePrerollPreset::TwoSeconds => PrerollSetting::Seconds(2.0),
            PracticePrerollPreset::Off => PrerollSetting::Off,
        };
        let (mode, ramp_config) = match config.trainer {
            PracticeTrainerPreset::Off => (PracticeTrainerMode::Off, RampConfig::default()),
            PracticeTrainerPreset::Wait => (PracticeTrainerMode::Wait, RampConfig::default()),
            PracticeTrainerPreset::Ramp(ramp) => (
                PracticeTrainerMode::Ramp,
                RampConfig {
                    start_tempo: ramp.start_tempo,
                    target_tempo: ramp.target_tempo,
                    step: ramp.step,
                    threshold_pct: ramp.threshold_pct,
                    required_successes: ramp.required_successes,
                },
            ),
        };
        Self {
            source: PracticeDraftSource::Custom,
            loop_region,
            user_tempo: config.tempo,
            snap,
            preroll,
            count_in: config.count_in,
            trainer: PracticeTrainerDraft { mode, ramp_config },
        }
    }
}

impl From<&PracticeDraft> for PracticePresetConfig {
    fn from(draft: &PracticeDraft) -> Self {
        let (loop_start_ms, loop_end_ms) = draft.loop_region.map_or((None, None), |region| {
            (Some(region.start_ms), Some(region.end_ms))
        });
        let snap = match draft.snap {
            SnapDivisor::Bar => PracticeSnapPreset::Bar,
            SnapDivisor::Beat => PracticeSnapPreset::Beat,
            SnapDivisor::Quarter => PracticeSnapPreset::HalfBeat,
        };
        let preroll = match draft.preroll {
            PrerollSetting::OneBar => PracticePrerollPreset::OneBar,
            PrerollSetting::Seconds(_) => PracticePrerollPreset::TwoSeconds,
            PrerollSetting::Off => PracticePrerollPreset::Off,
        };
        let trainer = match draft.trainer.mode {
            PracticeTrainerMode::Off => PracticeTrainerPreset::Off,
            PracticeTrainerMode::Wait => PracticeTrainerPreset::Wait,
            PracticeTrainerMode::Ramp => {
                let ramp = draft.trainer.ramp_config;
                PracticeTrainerPreset::Ramp(RampPreset {
                    start_tempo: ramp.start_tempo,
                    target_tempo: ramp.target_tempo,
                    step: ramp.step,
                    threshold_pct: ramp.threshold_pct,
                    required_successes: ramp.required_successes,
                })
            }
        };
        Self {
            loop_start_ms,
            loop_end_ms,
            snap,
            tempo: draft.user_tempo,
            preroll,
            count_in: draft.count_in,
            trainer,
        }
    }
}

fn finite_clamp(value: f32, min: f32, max: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback.clamp(min, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::practice::session::{LoopRegion, PracticeSession, PrerollSetting, RampConfig};
    use crate::timeline::{ChipTimeline, SnapDivisor};
    use dtx_config::{
        PracticePrerollPreset, PracticePresetConfig, PracticeSnapPreset, PracticeTrainerPreset,
        RampPreset,
    };
    use game_shell::{PracticeOrigin, PracticeRecommendation, PracticeRequest, PracticeSeed};

    fn timeline() -> ChipTimeline {
        ChipTimeline {
            end_ms: 10_000,
            ..Default::default()
        }
    }

    #[test]
    fn wait_and_ramp_are_one_mode() {
        let mut draft = PracticeDraft::default();
        draft.set_trainer_mode(PracticeTrainerMode::Wait);
        assert_eq!(draft.trainer_mode(), PracticeTrainerMode::Wait);
        draft.set_trainer_mode(PracticeTrainerMode::Ramp);
        assert_eq!(draft.trainer_mode(), PracticeTrainerMode::Ramp);
    }

    #[test]
    fn invalid_bounds_fall_back_to_whole_song() {
        let draft = PracticeDraft {
            loop_region: Some(LoopRegion {
                start_ms: 4_000,
                end_ms: 4_000,
            }),
            ..Default::default()
        };

        let validated = draft.validate(&timeline()).expect("recoverable draft");

        assert_eq!(validated.draft.loop_region, None);
        assert!(validated.warning.is_some());
    }

    #[test]
    fn out_of_chart_saved_loop_falls_back_to_whole_song_source() {
        let config = PracticePresetConfig {
            loop_start_ms: Some(20_000),
            loop_end_ms: Some(30_000),
            snap: PracticeSnapPreset::Bar,
            tempo: 1.0,
            preroll: PracticePrerollPreset::OneBar,
            count_in: true,
            trainer: PracticeTrainerPreset::Off,
        };
        let draft = PracticeDraft::from_preset(17, &config);

        let validated = draft.validate(&timeline()).expect("recoverable draft");

        assert_eq!(validated.draft.loop_region, None);
        assert_eq!(validated.draft.source, PracticeDraftSource::WholeSong);
        assert!(validated.warning.is_some());
    }

    #[test]
    fn out_of_chart_custom_loop_falls_back_to_whole_song_source() {
        let mut session = PracticeSession::default();
        session.transport.loop_region = Some(LoopRegion {
            start_ms: 20_000,
            end_ms: 30_000,
        });
        let draft = PracticeDraft::from(&session);

        let validated = draft.validate(&timeline()).expect("recoverable draft");

        assert_eq!(validated.draft.loop_region, None);
        assert_eq!(validated.draft.source, PracticeDraftSource::WholeSong);
        assert!(validated.warning.is_some());
    }

    #[test]
    fn out_of_chart_recommended_loop_falls_back_to_whole_song_source() {
        let request = PracticeRequest {
            origin: PracticeOrigin::Results,
            seed: PracticeSeed::Recommended(PracticeRecommendation::weak_section(
                20_000, 30_000, None,
            )),
        };
        let draft = PracticeDraft::from_request(&request);

        let validated = draft.validate(&timeline()).expect("recoverable draft");

        assert_eq!(validated.draft.loop_region, None);
        assert_eq!(validated.draft.source, PracticeDraftSource::WholeSong);
        assert!(validated.warning.is_some());
    }

    #[test]
    fn validation_normalizes_bounds_and_clamps_every_numeric_field() {
        let mut draft = PracticeDraft {
            loop_region: Some(LoopRegion {
                start_ms: 12_000,
                end_ms: -500,
            }),
            user_tempo: 9.0,
            ..Default::default()
        };
        draft.trainer.ramp_config = RampConfig {
            start_tempo: 1.8,
            target_tempo: 0.1,
            step: 9.0,
            threshold_pct: 20.0,
            required_successes: 9,
        };

        let validated = draft.validate(&timeline()).expect("recoverable draft");

        assert_eq!(
            validated.draft.loop_region,
            Some(LoopRegion {
                start_ms: 0,
                end_ms: 10_000,
            })
        );
        assert_eq!(validated.draft.user_tempo, 1.5);
        assert_eq!(validated.draft.trainer.ramp_config.start_tempo, 1.45);
        assert_eq!(validated.draft.trainer.ramp_config.target_tempo, 1.5);
        assert_eq!(validated.draft.trainer.ramp_config.step, 0.25);
        assert_eq!(validated.draft.trainer.ramp_config.threshold_pct, 50.0);
        assert_eq!(validated.draft.trainer.ramp_config.required_successes, 3);
    }

    #[test]
    fn validation_preserves_the_exact_supported_ramp_gap() {
        let mut draft = PracticeDraft::default();
        draft.trainer.ramp_config.start_tempo = 0.60;
        draft.trainer.ramp_config.target_tempo = 0.65;

        let validated = draft.validate(&timeline()).expect("validated").draft;

        assert_eq!(validated.trainer.ramp_config.start_tempo, 0.60);
        assert_eq!(validated.trainer.ramp_config.target_tempo, 0.65);
    }

    #[test]
    fn session_conversion_maps_all_committed_fields() {
        let mut session = PracticeSession::default();
        session.transport.loop_region = Some(LoopRegion {
            start_ms: 1_000,
            end_ms: 8_000,
        });
        session.transport.user_tempo = 0.85;
        session.transport.snap = SnapDivisor::Quarter;
        session.transport.preroll = PrerollSetting::Seconds(2.0);
        session.transport.metronome = false;
        session.trainer.ramp_config = RampConfig {
            start_tempo: 0.65,
            target_tempo: 1.1,
            step: 0.1,
            threshold_pct: 92.0,
            required_successes: 2,
        };
        session.trainer.arm_ramp();

        let draft = PracticeDraft::from(&session);
        let mut restored = PracticeSession::default();
        draft.apply_to_session(&mut restored);

        assert_eq!(
            restored.transport.loop_region,
            session.transport.loop_region
        );
        assert_eq!(restored.transport.user_tempo, session.transport.user_tempo);
        assert_eq!(restored.transport.snap, session.transport.snap);
        assert_eq!(restored.transport.preroll, session.transport.preroll);
        assert_eq!(restored.transport.metronome, session.transport.metronome);
        assert_eq!(restored.trainer.mode, PracticeTrainerMode::Ramp);
        assert_eq!(restored.trainer.ramp_config, session.trainer.ramp_config);
        assert!(restored.trainer.ramp_armed());
    }

    #[test]
    fn commit_keeps_diagnosis_only_for_the_same_span() {
        use dtx_scoring::JudgmentKind;

        let mut session = PracticeSession::default();
        session.transport.loop_region = Some(LoopRegion {
            start_ms: 1_000,
            end_ms: 5_000,
        });
        session
            .lane_diag
            .apply_judgment(0, JudgmentKind::Perfect, 0);
        let same = PracticeDraft {
            loop_region: session.transport.loop_region,
            ..Default::default()
        };
        same.apply_to_session(&mut session);
        assert!(!session.lane_diag.lanes.is_empty());

        let changed = PracticeDraft {
            loop_region: Some(LoopRegion {
                start_ms: 2_000,
                end_ms: 6_000,
            }),
            ..same
        };
        changed.apply_to_session(&mut session);
        assert!(session.lane_diag.lanes.is_empty());
    }

    #[test]
    fn preset_conversion_round_trips_all_fields() {
        let config = PracticePresetConfig {
            loop_start_ms: Some(2_000),
            loop_end_ms: Some(6_000),
            snap: PracticeSnapPreset::HalfBeat,
            tempo: 0.8,
            preroll: PracticePrerollPreset::TwoSeconds,
            count_in: false,
            trainer: PracticeTrainerPreset::Ramp(RampPreset {
                start_tempo: 0.6,
                target_tempo: 1.0,
                step: 0.1,
                threshold_pct: 95.0,
                required_successes: 3,
            }),
        };

        let draft = PracticeDraft::from_preset(17, &config);

        assert_eq!(draft.source, PracticeDraftSource::Saved(17));
        assert_eq!(PracticePresetConfig::from(&draft), config);
    }

    #[test]
    fn recommended_request_seeds_source_loop_and_tempo() {
        let request = PracticeRequest {
            origin: PracticeOrigin::Results,
            seed: PracticeSeed::Recommended(PracticeRecommendation::weak_section(
                2_000,
                6_000,
                Some(3),
            )),
        };

        let draft = PracticeDraft::from_request(&request);

        assert_eq!(draft.source, PracticeDraftSource::Recommended);
        assert_eq!(
            draft.loop_region,
            Some(LoopRegion {
                start_ms: 2_000,
                end_ms: 6_000,
            })
        );
        assert_eq!(draft.user_tempo, 1.0);
    }

    #[test]
    fn invalid_recommendation_falls_back_to_whole_song_source() {
        let request = PracticeRequest {
            origin: PracticeOrigin::Results,
            seed: PracticeSeed::Recommended(PracticeRecommendation::weak_section(
                4_000, 4_000, None,
            )),
        };

        let draft = PracticeDraft::from_request(&request);

        assert_eq!(draft.source, PracticeDraftSource::WholeSong);
        assert_eq!(draft.loop_region, None);
    }
}
