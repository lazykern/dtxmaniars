//! Drum pad grouping, cymbal-free judgment, hit-sound priority.
//!
//! Port of BocuD `CStagePerfDrumsScreen.cs` hit logic + `CStagePerfCommonScreen.cs:r空うちChip`.
//! Reference: `CStagePerfDrumsScreen.cs:697-723` (effective groups),
//! `CStagePerfDrumsScreen.cs:743-1422` (HH/CY/RD/LC), `CStagePerfDrumsScreen.cs:971-1132` (BD/LP/LBD).

use std::collections::HashSet;

use dtx_config::{BdGroup, CyGroup, DrumsConfig, FtGroup, HhGroup, HitSoundPriority};
use dtx_core::{Chart, EChannel};

use crate::judge::chip_target_ms;
use crate::lane_map::{lane_of, LaneId, LANE_COUNT, LANE_ORDER};
use dtx_timing::math::BpmChange;

pub const MAX_JUDGE_WINDOW_MS: i64 = 117;

/// BocuD `EPad` indices 0..11.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DrumPad {
    Hh = 0,
    Sd = 1,
    Bd = 2,
    Ht = 3,
    Lt = 4,
    Ft = 5,
    Cy = 6,
    Hho = 7,
    Rd = 8,
    Lc = 9,
    Lp = 10,
    Lbd = 11,
}

impl DrumPad {
    pub fn from_lane(lane: LaneId) -> Option<Self> {
        match lane {
            0 => Some(Self::Hh),
            1 => Some(Self::Sd),
            2 => Some(Self::Bd),
            3 => Some(Self::Ht),
            4 => Some(Self::Lt),
            5 => Some(Self::Ft),
            6 => Some(Self::Cy),
            7 => Some(Self::Hho),
            8 => Some(Self::Rd),
            9 => Some(Self::Lc),
            10 => Some(Self::Lp),
            11 => Some(Self::Lbd),
            _ => None,
        }
    }

    pub fn lane(self) -> LaneId {
        self as LaneId
    }
}

/// Which drum channels appear in the loaded chart (`bHasChips`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChartChipPresence {
    pub hh_close: bool,
    pub hh_open: bool,
    pub left_cymbal: bool,
    pub cymbal: bool,
    pub ride: bool,
    pub bass_drum: bool,
    pub left_pedal: bool,
    pub left_bass_drum: bool,
}

impl ChartChipPresence {
    pub fn from_chart(chart: &Chart) -> Self {
        let mut p = Self::default();
        for chip in &chart.chips {
            match chip.channel {
                EChannel::HiHatClose => p.hh_close = true,
                EChannel::HiHatOpen => p.hh_open = true,
                EChannel::LeftCymbal => p.left_cymbal = true,
                EChannel::Cymbal => p.cymbal = true,
                EChannel::RideCymbal => p.ride = true,
                EChannel::BassDrum => p.bass_drum = true,
                EChannel::LeftPedal => p.left_pedal = true,
                EChannel::LeftBassDrum => p.left_bass_drum = true,
                _ => {}
            }
        }
        p
    }
}

/// Runtime grouping after chart-aware auto-downgrade.
/// Reference: `CStagePerfDrumsScreen.cs:697-723`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectiveGroups {
    pub cy: CyGroup,
    pub hh: HhGroup,
    pub ft: FtGroup,
    pub bd: BdGroup,
    pub cymbal_free: bool,
}

impl EffectiveGroups {
    pub fn from_config(config: &DrumsConfig, presence: &ChartChipPresence) -> Self {
        let mut cy = config.cy_group;
        let mut hh = config.hh_group;

        if !presence.ride && cy == CyGroup::Separate {
            cy = CyGroup::Common;
        }
        if !presence.hh_open && hh == HhGroup::SeparateAll {
            hh = HhGroup::HhAndHo;
        }
        if !presence.hh_open && hh == HhGroup::HhAndLc {
            hh = HhGroup::CommonAll;
        }
        if !presence.left_cymbal && hh == HhGroup::SeparateAll {
            hh = HhGroup::HhAndLc;
        }
        if !presence.left_cymbal && hh == HhGroup::HhAndHo {
            hh = HhGroup::CommonAll;
        }

        Self {
            cy,
            hh,
            ft: config.ft_group,
            bd: config.bd_group,
            cymbal_free: config.cymbal_free,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Candidate {
    idx: usize,
    target_ms: i64,
    delta: i64,
    channel: EChannel,
}

/// Resolve judgment targets for a pad press. May return multiple hits on tie.
pub fn resolve_judgments(
    pad: DrumPad,
    audio_ms: i64,
    chart: &Chart,
    judged: &HashSet<usize>,
    base_bpm: f32,
    bpm_changes: &[BpmChange],
    groups: &EffectiveGroups,
) -> Vec<(usize, i64)> {
    let hits = match pad {
        DrumPad::Sd | DrumPad::Ht => single_channel_hit(
            pad_channel(pad),
            audio_ms,
            chart,
            judged,
            base_bpm,
            bpm_changes,
        ),
        DrumPad::Lt | DrumPad::Ft => resolve_ft_group(
            pad,
            audio_ms,
            chart,
            judged,
            base_bpm,
            bpm_changes,
            groups.ft,
        ),
        DrumPad::Hh => resolve_hh_pad(
            DrumPad::Hh,
            audio_ms,
            chart,
            judged,
            base_bpm,
            bpm_changes,
            groups.hh,
        ),
        DrumPad::Hho => resolve_hho_pad(audio_ms, chart, judged, base_bpm, bpm_changes, groups.hh),
        DrumPad::Cy => resolve_cy_pad(audio_ms, chart, judged, base_bpm, bpm_changes, groups),
        DrumPad::Rd => resolve_rd_pad(audio_ms, chart, judged, base_bpm, bpm_changes, groups),
        DrumPad::Lc => resolve_lc_pad(audio_ms, chart, judged, base_bpm, bpm_changes, groups),
        DrumPad::Bd => resolve_bd_pad(audio_ms, chart, judged, base_bpm, bpm_changes, groups.bd),
        DrumPad::Lp => resolve_lp_pad(audio_ms, chart, judged, base_bpm, bpm_changes, groups.bd),
        DrumPad::Lbd => resolve_lbd_pad(audio_ms, chart, judged, base_bpm, bpm_changes, groups.bd),
    };
    hits.into_iter().map(|c| (c.idx, c.delta)).collect()
}

fn pad_channel(pad: DrumPad) -> EChannel {
    LANE_ORDER[pad as usize]
}

fn single_channel_hit(
    channel: EChannel,
    audio_ms: i64,
    chart: &Chart,
    judged: &HashSet<usize>,
    base_bpm: f32,
    bpm_changes: &[BpmChange],
) -> Vec<Candidate> {
    closest_candidate(channel, audio_ms, chart, judged, base_bpm, bpm_changes)
        .into_iter()
        .collect()
}

fn closest_candidate(
    channel: EChannel,
    audio_ms: i64,
    chart: &Chart,
    judged: &HashSet<usize>,
    base_bpm: f32,
    bpm_changes: &[BpmChange],
) -> Option<Candidate> {
    let mut best: Option<Candidate> = None;
    for (idx, chip) in chart.chips.iter().enumerate() {
        if chip.channel != channel || judged.contains(&idx) {
            continue;
        }
        let target_ms = chip_target_ms(chip, base_bpm, bpm_changes);
        let delta = audio_ms - target_ms;
        if delta.abs() > MAX_JUDGE_WINDOW_MS {
            continue;
        }
        match best {
            Some(b)
                if delta.abs() < b.delta.abs()
                    || (delta.abs() == b.delta.abs() && target_ms < b.target_ms) =>
            {
                best = Some(Candidate {
                    idx,
                    target_ms,
                    delta,
                    channel,
                });
            }
            None => {
                best = Some(Candidate {
                    idx,
                    target_ms,
                    delta,
                    channel,
                });
            }
            _ => {}
        }
    }
    best
}

fn candidates_for_channels(
    channels: &[EChannel],
    audio_ms: i64,
    chart: &Chart,
    judged: &HashSet<usize>,
    base_bpm: f32,
    bpm_changes: &[BpmChange],
) -> Vec<Candidate> {
    channels
        .iter()
        .filter_map(|&ch| closest_candidate(ch, audio_ms, chart, judged, base_bpm, bpm_changes))
        .collect()
}

/// Pick earliest by playback time; on tie return all at that time.
fn pick_earliest(candidates: Vec<Candidate>) -> Vec<Candidate> {
    if candidates.is_empty() {
        return candidates;
    }
    let min_t = candidates.iter().map(|c| c.target_ms).min().unwrap();
    candidates
        .into_iter()
        .filter(|c| c.target_ms == min_t)
        .collect()
}

fn pick_pair_earliest(a: Option<Candidate>, b: Option<Candidate>) -> Vec<Candidate> {
    match (a, b) {
        (None, None) => vec![],
        (Some(x), None) => vec![x],
        (None, Some(y)) => vec![y],
        (Some(x), Some(y)) if x.target_ms == y.target_ms => vec![x, y],
        (Some(x), Some(y)) if x.target_ms < y.target_ms => vec![x],
        (Some(_x), Some(y)) => vec![y],
    }
}

fn resolve_ft_group(
    pad: DrumPad,
    audio_ms: i64,
    chart: &Chart,
    judged: &HashSet<usize>,
    base_bpm: f32,
    bpm_changes: &[BpmChange],
    ft_group: FtGroup,
) -> Vec<Candidate> {
    let lt = closest_candidate(
        EChannel::LowTom,
        audio_ms,
        chart,
        judged,
        base_bpm,
        bpm_changes,
    );
    let ft = closest_candidate(
        EChannel::FloorTom,
        audio_ms,
        chart,
        judged,
        base_bpm,
        bpm_changes,
    );
    match ft_group {
        FtGroup::Separate => single_channel_hit(
            pad_channel(pad),
            audio_ms,
            chart,
            judged,
            base_bpm,
            bpm_changes,
        ),
        FtGroup::Common => {
            let pool = match pad {
                DrumPad::Lt => pick_pair_earliest(lt, ft),
                DrumPad::Ft => pick_pair_earliest(ft, lt),
                _ => vec![],
            };
            if pool.is_empty() {
                pick_earliest(candidates_for_channels(
                    &[EChannel::LowTom, EChannel::FloorTom],
                    audio_ms,
                    chart,
                    judged,
                    base_bpm,
                    bpm_changes,
                ))
            } else {
                pool
            }
        }
    }
}

fn resolve_hh_pad(
    _pad: DrumPad,
    audio_ms: i64,
    chart: &Chart,
    judged: &HashSet<usize>,
    base_bpm: f32,
    bpm_changes: &[BpmChange],
    hh: HhGroup,
) -> Vec<Candidate> {
    let hc = closest_candidate(
        EChannel::HiHatClose,
        audio_ms,
        chart,
        judged,
        base_bpm,
        bpm_changes,
    );
    let ho = closest_candidate(
        EChannel::HiHatOpen,
        audio_ms,
        chart,
        judged,
        base_bpm,
        bpm_changes,
    );
    let lc = closest_candidate(
        EChannel::LeftCymbal,
        audio_ms,
        chart,
        judged,
        base_bpm,
        bpm_changes,
    );
    match hh {
        HhGroup::SeparateAll => hc.into_iter().collect(),
        HhGroup::HhAndLc => pick_pair_earliest(hc, lc),
        HhGroup::HhAndHo => pick_pair_earliest(hc, ho),
        HhGroup::CommonAll => pick_earliest(candidates_for_channels(
            &[
                EChannel::HiHatClose,
                EChannel::HiHatOpen,
                EChannel::LeftCymbal,
            ],
            audio_ms,
            chart,
            judged,
            base_bpm,
            bpm_changes,
        )),
    }
}

fn resolve_hho_pad(
    audio_ms: i64,
    chart: &Chart,
    judged: &HashSet<usize>,
    base_bpm: f32,
    bpm_changes: &[BpmChange],
    hh: HhGroup,
) -> Vec<Candidate> {
    let hc = closest_candidate(
        EChannel::HiHatClose,
        audio_ms,
        chart,
        judged,
        base_bpm,
        bpm_changes,
    );
    let ho = closest_candidate(
        EChannel::HiHatOpen,
        audio_ms,
        chart,
        judged,
        base_bpm,
        bpm_changes,
    );
    let lc = closest_candidate(
        EChannel::LeftCymbal,
        audio_ms,
        chart,
        judged,
        base_bpm,
        bpm_changes,
    );
    match hh {
        HhGroup::SeparateAll => ho.into_iter().collect(),
        HhGroup::HhAndLc => pick_pair_earliest(ho, lc),
        HhGroup::HhAndHo => pick_pair_earliest(hc, ho),
        HhGroup::CommonAll => pick_earliest(candidates_for_channels(
            &[
                EChannel::HiHatClose,
                EChannel::HiHatOpen,
                EChannel::LeftCymbal,
            ],
            audio_ms,
            chart,
            judged,
            base_bpm,
            bpm_changes,
        )),
    }
}

fn resolve_cy_pad(
    audio_ms: i64,
    chart: &Chart,
    judged: &HashSet<usize>,
    base_bpm: f32,
    bpm_changes: &[BpmChange],
    groups: &EffectiveGroups,
) -> Vec<Candidate> {
    let cy = closest_candidate(
        EChannel::Cymbal,
        audio_ms,
        chart,
        judged,
        base_bpm,
        bpm_changes,
    );
    let rd = closest_candidate(
        EChannel::RideCymbal,
        audio_ms,
        chart,
        judged,
        base_bpm,
        bpm_changes,
    );
    let lc = if groups.cymbal_free {
        closest_candidate(
            EChannel::LeftCymbal,
            audio_ms,
            chart,
            judged,
            base_bpm,
            bpm_changes,
        )
    } else {
        None
    };

    let mut pool = vec![];
    if let Some(c) = cy {
        pool.push(c);
    }
    if let Some(c) = rd {
        pool.push(c);
    }
    if let Some(c) = lc {
        pool.push(c);
    }
    pool.sort_by_key(|c| c.target_ms);

    match (groups.cy, groups.cymbal_free) {
        (CyGroup::Separate, false) => cy.into_iter().collect(),
        (CyGroup::Separate, true) => {
            let filtered: Vec<_> = pool
                .into_iter()
                .filter(|c| matches!(c.channel, EChannel::Cymbal | EChannel::LeftCymbal))
                .collect();
            pick_earliest(filtered)
        }
        (CyGroup::Common, false) => {
            let filtered: Vec<_> = pool
                .into_iter()
                .filter(|c| matches!(c.channel, EChannel::Cymbal | EChannel::RideCymbal))
                .collect();
            pick_earliest(filtered)
        }
        (CyGroup::Common, true) => pick_earliest(pool),
    }
}

fn resolve_rd_pad(
    audio_ms: i64,
    chart: &Chart,
    judged: &HashSet<usize>,
    base_bpm: f32,
    bpm_changes: &[BpmChange],
    groups: &EffectiveGroups,
) -> Vec<Candidate> {
    match (groups.cy, groups.cymbal_free) {
        (CyGroup::Separate, false) => closest_candidate(
            EChannel::RideCymbal,
            audio_ms,
            chart,
            judged,
            base_bpm,
            bpm_changes,
        )
        .into_iter()
        .collect(),
        (CyGroup::Separate, true) => {
            let lc = closest_candidate(
                EChannel::LeftCymbal,
                audio_ms,
                chart,
                judged,
                base_bpm,
                bpm_changes,
            );
            let rd = closest_candidate(
                EChannel::RideCymbal,
                audio_ms,
                chart,
                judged,
                base_bpm,
                bpm_changes,
            );
            pick_pair_earliest(rd, lc)
        }
        (CyGroup::Common, false) => {
            resolve_cy_pad(audio_ms, chart, judged, base_bpm, bpm_changes, groups)
        }
        (CyGroup::Common, true) => {
            resolve_cy_pad(audio_ms, chart, judged, base_bpm, bpm_changes, groups)
        }
    }
}

fn resolve_lc_pad(
    audio_ms: i64,
    chart: &Chart,
    judged: &HashSet<usize>,
    base_bpm: f32,
    bpm_changes: &[BpmChange],
    groups: &EffectiveGroups,
) -> Vec<Candidate> {
    // Reference: `CStagePerfDrumsScreen.cs:1698-1786` (EPad.LC).
    let hc = closest_candidate(
        EChannel::HiHatClose,
        audio_ms,
        chart,
        judged,
        base_bpm,
        bpm_changes,
    );
    let ho = closest_candidate(
        EChannel::HiHatOpen,
        audio_ms,
        chart,
        judged,
        base_bpm,
        bpm_changes,
    );
    let lc = closest_candidate(
        EChannel::LeftCymbal,
        audio_ms,
        chart,
        judged,
        base_bpm,
        bpm_changes,
    );
    let cy = if groups.cymbal_free {
        closest_candidate(
            EChannel::Cymbal,
            audio_ms,
            chart,
            judged,
            base_bpm,
            bpm_changes,
        )
    } else {
        None
    };
    let rd = if groups.cymbal_free {
        closest_candidate(
            EChannel::RideCymbal,
            audio_ms,
            chart,
            judged,
            base_bpm,
            bpm_changes,
        )
    } else {
        None
    };

    if !groups.cymbal_free {
        return match groups.hh {
            HhGroup::SeparateAll | HhGroup::HhAndHo => lc.into_iter().collect(),
            HhGroup::HhAndLc | HhGroup::CommonAll => pick_earliest(candidates_for_channels(
                &[
                    EChannel::LeftCymbal,
                    EChannel::HiHatClose,
                    EChannel::HiHatOpen,
                ],
                audio_ms,
                chart,
                judged,
                base_bpm,
                bpm_changes,
            )),
        };
    }

    let mut pool = vec![];
    for c in [hc, ho, lc, cy, rd].into_iter().flatten() {
        pool.push(c);
    }
    pool.sort_by_key(|c| c.target_ms);

    match groups.hh {
        HhGroup::SeparateAll | HhGroup::HhAndHo => pool
            .into_iter()
            .find(|c| {
                matches!(c.channel, EChannel::LeftCymbal | EChannel::Cymbal)
                    || (c.channel == EChannel::RideCymbal && groups.cy == CyGroup::Common)
            })
            .into_iter()
            .collect(),
        HhGroup::HhAndLc | HhGroup::CommonAll => pool
            .into_iter()
            .find(|c| c.channel != EChannel::RideCymbal || groups.cy == CyGroup::Common)
            .into_iter()
            .collect(),
    }
}

fn resolve_bd_pedal_group(
    audio_ms: i64,
    chart: &Chart,
    judged: &HashSet<usize>,
    base_bpm: f32,
    bpm_changes: &[BpmChange],
    bd: BdGroup,
) -> Vec<Candidate> {
    let chip_bd = closest_candidate(
        EChannel::BassDrum,
        audio_ms,
        chart,
        judged,
        base_bpm,
        bpm_changes,
    );
    let _chip_lp = closest_candidate(
        EChannel::LeftPedal,
        audio_ms,
        chart,
        judged,
        base_bpm,
        bpm_changes,
    );
    let chip_lbd = closest_candidate(
        EChannel::LeftBassDrum,
        audio_ms,
        chart,
        judged,
        base_bpm,
        bpm_changes,
    );

    match bd {
        BdGroup::Separate => vec![],
        BdGroup::BdAndLbd => pick_pair_earliest(chip_bd, chip_lbd),
        BdGroup::PedalsOnly => chip_bd.into_iter().collect(),
        BdGroup::AllBd => pick_earliest(candidates_for_channels(
            &[
                EChannel::BassDrum,
                EChannel::LeftPedal,
                EChannel::LeftBassDrum,
            ],
            audio_ms,
            chart,
            judged,
            base_bpm,
            bpm_changes,
        )),
    }
}

fn resolve_bd_pad(
    audio_ms: i64,
    chart: &Chart,
    judged: &HashSet<usize>,
    base_bpm: f32,
    bpm_changes: &[BpmChange],
    bd: BdGroup,
) -> Vec<Candidate> {
    match bd {
        BdGroup::Separate => single_channel_hit(
            EChannel::BassDrum,
            audio_ms,
            chart,
            judged,
            base_bpm,
            bpm_changes,
        ),
        BdGroup::BdAndLbd => {
            let chip_bd = closest_candidate(
                EChannel::BassDrum,
                audio_ms,
                chart,
                judged,
                base_bpm,
                bpm_changes,
            );
            let chip_lbd = closest_candidate(
                EChannel::LeftBassDrum,
                audio_ms,
                chart,
                judged,
                base_bpm,
                bpm_changes,
            );
            pick_pair_earliest(chip_bd, chip_lbd)
        }
        BdGroup::PedalsOnly => single_channel_hit(
            EChannel::BassDrum,
            audio_ms,
            chart,
            judged,
            base_bpm,
            bpm_changes,
        ),
        BdGroup::AllBd => {
            resolve_bd_pedal_group(audio_ms, chart, judged, base_bpm, bpm_changes, bd)
        }
    }
}

fn resolve_lp_pad(
    audio_ms: i64,
    chart: &Chart,
    judged: &HashSet<usize>,
    base_bpm: f32,
    bpm_changes: &[BpmChange],
    bd: BdGroup,
) -> Vec<Candidate> {
    match bd {
        BdGroup::Separate => single_channel_hit(
            EChannel::LeftPedal,
            audio_ms,
            chart,
            judged,
            base_bpm,
            bpm_changes,
        ),
        BdGroup::BdAndLbd | BdGroup::PedalsOnly => {
            let lp = closest_candidate(
                EChannel::LeftPedal,
                audio_ms,
                chart,
                judged,
                base_bpm,
                bpm_changes,
            );
            let lbd = closest_candidate(
                EChannel::LeftBassDrum,
                audio_ms,
                chart,
                judged,
                base_bpm,
                bpm_changes,
            );
            pick_pair_earliest(lp, lbd)
        }
        BdGroup::AllBd => {
            resolve_bd_pedal_group(audio_ms, chart, judged, base_bpm, bpm_changes, bd)
        }
    }
}

fn resolve_lbd_pad(
    audio_ms: i64,
    chart: &Chart,
    judged: &HashSet<usize>,
    base_bpm: f32,
    bpm_changes: &[BpmChange],
    bd: BdGroup,
) -> Vec<Candidate> {
    match bd {
        BdGroup::Separate => single_channel_hit(
            EChannel::LeftBassDrum,
            audio_ms,
            chart,
            judged,
            base_bpm,
            bpm_changes,
        ),
        BdGroup::BdAndLbd => resolve_bd_pad(audio_ms, chart, judged, base_bpm, bpm_changes, bd),
        BdGroup::PedalsOnly => {
            let lp = closest_candidate(
                EChannel::LeftPedal,
                audio_ms,
                chart,
                judged,
                base_bpm,
                bpm_changes,
            );
            let lbd = closest_candidate(
                EChannel::LeftBassDrum,
                audio_ms,
                chart,
                judged,
                base_bpm,
                bpm_changes,
            );
            pick_pair_earliest(lbd, lp)
        }
        BdGroup::AllBd => {
            resolve_bd_pedal_group(audio_ms, chart, judged, base_bpm, bpm_changes, bd)
        }
    }
}

/// Lanes to try for empty-hit sound lookup (NoChip templates, then nearest chip).
/// Reference: `CStagePerfCommonScreen.cs:989-1119`.
pub fn empty_hit_fallback_lanes(pad: DrumPad, groups: &EffectiveGroups) -> &'static [LaneId] {
    match pad {
        DrumPad::Hh => match groups.hh {
            HhGroup::SeparateAll => &[0][..],
            HhGroup::HhAndLc => &[0, 9][..],
            HhGroup::HhAndHo => &[0, 7][..],
            HhGroup::CommonAll => &[0, 7, 9][..],
        },
        DrumPad::Hho => match groups.hh {
            HhGroup::SeparateAll => &[7][..],
            HhGroup::HhAndLc => &[7, 9][..],
            HhGroup::HhAndHo => &[7, 0][..],
            HhGroup::CommonAll => &[7, 0, 9][..],
        },
        DrumPad::Lt => match groups.ft {
            FtGroup::Separate => &[4][..],
            FtGroup::Common => &[4, 5][..],
        },
        DrumPad::Ft => match groups.ft {
            FtGroup::Separate => &[5][..],
            FtGroup::Common => &[5, 4][..],
        },
        DrumPad::Cy => match groups.cy {
            CyGroup::Separate => &[6][..],
            CyGroup::Common => &[6, 8][..],
        },
        DrumPad::Rd => match (groups.cy, groups.cymbal_free) {
            (CyGroup::Separate, false) => &[8][..],
            (CyGroup::Separate, true) => &[8, 9][..],
            (CyGroup::Common, _) => &[6, 8][..],
        },
        DrumPad::Lc => {
            if groups.cymbal_free {
                &[9, 6, 8][..]
            } else {
                &[9, 0, 7][..]
            }
        }
        DrumPad::Lp => match groups.bd {
            BdGroup::Separate => &[10][..],
            BdGroup::BdAndLbd | BdGroup::PedalsOnly => &[10, 11][..],
            BdGroup::AllBd => &[10, 11, 2][..],
        },
        DrumPad::Lbd => match groups.bd {
            BdGroup::Separate => &[11][..],
            BdGroup::BdAndLbd | BdGroup::PedalsOnly => &[11, 10][..],
            BdGroup::AllBd => &[11, 10, 2][..],
        },
        DrumPad::Bd => match groups.bd {
            BdGroup::Separate => &[2][..],
            BdGroup::BdAndLbd => &[2, 11][..],
            BdGroup::PedalsOnly => &[2][..],
            BdGroup::AllBd => &[2, 10, 11][..],
        },
        DrumPad::Sd => &[1][..],
        DrumPad::Ht => &[3][..],
    }
}

/// Pad channel for pad-over-chip sound lookup, with missing-lane remap.
/// Reference: `CStagePerfDrumsScreen.cs:610-622`.
pub fn sound_pad_channel(pad: DrumPad, presence: &ChartChipPresence) -> EChannel {
    match pad {
        DrumPad::Hho if !presence.hh_open => EChannel::HiHatClose,
        DrumPad::Rd if !presence.ride => EChannel::Cymbal,
        DrumPad::Lc if !presence.left_cymbal => EChannel::HiHatClose,
        _ => pad_channel(pad),
    }
}

pub fn chip_over_pad(pad: DrumPad, config: &DrumsConfig) -> bool {
    let pri = match pad {
        DrumPad::Hh | DrumPad::Hho | DrumPad::Lc => config.hit_sound_priority_hh,
        DrumPad::Lt | DrumPad::Ft => config.hit_sound_priority_ft,
        DrumPad::Cy | DrumPad::Rd => config.hit_sound_priority_cy,
        DrumPad::Bd | DrumPad::Lp | DrumPad::Lbd => config.hit_sound_priority_lp,
        DrumPad::Sd | DrumPad::Ht => HitSoundPriority::ChipOverPad,
    };
    pri == HitSoundPriority::ChipOverPad
}

/// Nearest chip on pad channel (judged or not) for pad-over-chip priority.
pub fn nearest_chip_on_channel(
    channel: EChannel,
    audio_ms: i64,
    chart: &Chart,
    base_bpm: f32,
    bpm_changes: &[BpmChange],
) -> Option<(usize, u32, EChannel)> {
    let mut best: Option<(usize, i64)> = None;
    for (idx, chip) in chart.chips.iter().enumerate() {
        if chip.channel != channel || chip.wav_slot == 0 {
            continue;
        }
        let target_ms = chip_target_ms(chip, base_bpm, bpm_changes);
        let dist = (audio_ms - target_ms).abs();
        match best {
            Some((_, d)) if d <= dist => {}
            _ => best = Some((idx, dist)),
        }
    }
    best.map(|(idx, _)| {
        let chip = &chart.chips[idx];
        (idx, chip.wav_slot, chip.channel)
    })
}

pub fn lane_count() -> usize {
    LANE_COUNT
}

pub fn lane_of_channel(channel: EChannel) -> Option<LaneId> {
    lane_of(channel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::Chip;

    fn chart_with(chips: Vec<Chip>) -> Chart {
        let mut c = Chart::default();
        c.metadata.bpm = Some(120.0);
        c.chips = chips;
        c
    }

    fn at(measure: u32, ch: EChannel, value: f32) -> Chip {
        Chip::new(measure, ch, value)
    }

    #[test]
    fn cy_separate_only_hits_cy() {
        let chart = chart_with(vec![
            at(0, EChannel::Cymbal, 0.5),
            at(0, EChannel::RideCymbal, 0.5),
        ]);
        let groups = EffectiveGroups {
            cy: CyGroup::Separate,
            hh: HhGroup::SeparateAll,
            ft: FtGroup::Separate,
            bd: BdGroup::Separate,
            cymbal_free: false,
        };
        let judged = HashSet::new();
        let hits = resolve_judgments(DrumPad::Cy, 1000, &chart, &judged, 120.0, &[], &groups);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 0);
    }

    #[test]
    fn cy_common_accepts_ride_on_cy_pad() {
        let chart = chart_with(vec![at(0, EChannel::RideCymbal, 0.5)]);
        let groups = EffectiveGroups {
            cy: CyGroup::Common,
            ..EffectiveGroups::from_config(&DrumsConfig::default(), &ChartChipPresence::default())
        };
        let hits = resolve_judgments(
            DrumPad::Cy,
            1000,
            &chart,
            &HashSet::new(),
            120.0,
            &[],
            &groups,
        );
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn cymbal_free_cy_separate_accepts_lc() {
        let chart = chart_with(vec![at(0, EChannel::LeftCymbal, 0.5)]);
        let groups = EffectiveGroups {
            cy: CyGroup::Separate,
            cymbal_free: true,
            ..EffectiveGroups::from_config(&DrumsConfig::default(), &ChartChipPresence::default())
        };
        let hits = resolve_judgments(
            DrumPad::Cy,
            1000,
            &chart,
            &HashSet::new(),
            120.0,
            &[],
            &groups,
        );
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn ride_missing_downgrades_cy_group() {
        let presence = ChartChipPresence {
            cymbal: true,
            ride: false,
            ..Default::default()
        };
        let eff = EffectiveGroups::from_config(
            &DrumsConfig {
                cy_group: CyGroup::Separate,
                ..Default::default()
            },
            &presence,
        );
        assert_eq!(eff.cy, CyGroup::Common);
    }

    #[test]
    fn ft_common_lt_hits_ft_chip() {
        let chart = chart_with(vec![at(0, EChannel::FloorTom, 0.5)]);
        let groups = EffectiveGroups {
            ft: FtGroup::Common,
            ..EffectiveGroups::from_config(&DrumsConfig::default(), &ChartChipPresence::default())
        };
        let hits = resolve_judgments(
            DrumPad::Lt,
            1000,
            &chart,
            &HashSet::new(),
            120.0,
            &[],
            &groups,
        );
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn sound_pad_remaps_rd_to_cy_when_no_ride() {
        let presence = ChartChipPresence {
            ride: false,
            ..Default::default()
        };
        assert_eq!(sound_pad_channel(DrumPad::Rd, &presence), EChannel::Cymbal);
    }

    #[test]
    fn twelve_lanes_in_order() {
        assert_eq!(LANE_COUNT, 12);
        assert_eq!(LANE_ORDER[9], EChannel::LeftCymbal);
        assert_eq!(LANE_ORDER[11], EChannel::LeftBassDrum);
    }

    #[test]
    fn lc_separate_all_hits_lc_only() {
        let chart = chart_with(vec![
            at(0, EChannel::LeftCymbal, 0.5),
            at(0, EChannel::HiHatClose, 0.5),
        ]);
        let groups = EffectiveGroups {
            hh: HhGroup::SeparateAll,
            ..EffectiveGroups::from_config(&DrumsConfig::default(), &ChartChipPresence::default())
        };
        let hits = resolve_judgments(
            DrumPad::Lc,
            1000,
            &chart,
            &HashSet::new(),
            120.0,
            &[],
            &groups,
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 0);
    }

    #[test]
    fn lc_common_all_without_cymbal_free_picks_earliest() {
        let chart = chart_with(vec![
            at(0, EChannel::HiHatClose, 0.49),
            at(0, EChannel::LeftCymbal, 0.51),
        ]);
        let groups = EffectiveGroups {
            hh: HhGroup::CommonAll,
            ..EffectiveGroups::from_config(&DrumsConfig::default(), &ChartChipPresence::default())
        };
        let hits = resolve_judgments(
            DrumPad::Lc,
            1000,
            &chart,
            &HashSet::new(),
            120.0,
            &[],
            &groups,
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 0);
    }

    #[test]
    fn lc_cymbal_free_separate_hits_lc_not_hh() {
        let chart = chart_with(vec![
            at(0, EChannel::LeftCymbal, 0.5),
            at(0, EChannel::HiHatClose, 0.5),
        ]);
        let groups = EffectiveGroups {
            hh: HhGroup::SeparateAll,
            cymbal_free: true,
            ..EffectiveGroups::from_config(&DrumsConfig::default(), &ChartChipPresence::default())
        };
        let hits = resolve_judgments(
            DrumPad::Lc,
            1000,
            &chart,
            &HashSet::new(),
            120.0,
            &[],
            &groups,
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 0);
    }
}
