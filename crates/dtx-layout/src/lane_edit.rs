//! Editor-facing lane transforms. Pure (no bevy). Every mutation flips the
//! arrangement to `LanePreset::Custom` and maintains the invariant: all 12
//! drum channels map to a lane id present in `lanes`, and `lanes` is non-empty.

use dtx_core::EChannel;

use crate::lanes::{
    channel_short_name, default_lane_width, DisplayLane, LaneArrangement, MAX_LANE_WIDTH,
    MIN_LANE_WIDTH,
};
use crate::presets::LanePreset;

/// Swap lane `index` with its neighbor in `dir` (-1 left, +1 right).
/// Returns false (no-op) when the move would leave the strip.
pub fn reorder_lane(arr: &mut LaneArrangement, index: usize, dir: i32) -> bool {
    let Some(target) = index.checked_add_signed(dir as isize) else {
        return false;
    };
    if index >= arr.lanes.len() || target >= arr.lanes.len() {
        return false;
    }
    arr.lanes.swap(index, target);
    arr.preset = LanePreset::Custom;
    true
}

/// Clamp + set lane width (ref px).
pub fn set_lane_width(arr: &mut LaneArrangement, index: usize, width: f32) -> bool {
    let Some(lane) = arr.lanes.get_mut(index) else {
        return false;
    };
    let clamped = width.clamp(MIN_LANE_WIDTH, MAX_LANE_WIDTH);
    if (lane.width - clamped).abs() < f32::EPSILON {
        return false;
    }
    lane.width = clamped;
    arr.preset = LanePreset::Custom;
    true
}

/// Split a secondary channel out of its host lane into its own new lane,
/// inserted directly after the host. No-op when `ch` already has its own lane
/// (is primary) or isn't a drum channel.
pub fn split_channel(arr: &mut LaneArrangement, ch: EChannel) -> bool {
    let Some(name) = channel_short_name(ch) else {
        return false;
    };
    let Some(host) = arr.lane_index_of(ch) else {
        return false;
    };
    if arr.lanes[host].primary == ch {
        return false;
    }
    // Degenerate state guard: a lane with this id already exists but the
    // channel points elsewhere — just remap.
    if arr.lanes.iter().any(|l| l.id == name) {
        arr.map.insert(ch, name.to_string());
        arr.preset = LanePreset::Custom;
        return true;
    }
    arr.lanes.insert(
        host + 1,
        DisplayLane {
            id: name.to_string(),
            label: name.to_string(),
            width: default_lane_width(ch),
            color: None,
            primary: ch,
        },
    );
    arr.map.insert(ch, name.to_string());
    arr.preset = LanePreset::Custom;
    true
}

/// Remove lane `index`, remapping every channel it hosted onto the left
/// neighbor (right neighbor when leftmost). No-op on the last remaining lane.
pub fn merge_lane(arr: &mut LaneArrangement, index: usize) -> bool {
    if arr.lanes.len() <= 1 || index >= arr.lanes.len() {
        return false;
    }
    let target_idx = if index > 0 { index - 1 } else { index + 1 };
    let target_id = arr.lanes[target_idx].id.clone();
    let removed = arr.lanes.remove(index);
    for id in arr.map.values_mut() {
        if *id == removed.id {
            *id = target_id.clone();
        }
    }
    arr.preset = LanePreset::Custom;
    true
}

/// Channels mapped to lane `index`, primary first, rest in DRUM_CHANNELS order.
pub fn lane_chips(arr: &LaneArrangement, index: usize) -> Vec<EChannel> {
    let Some(lane) = arr.lanes.get(index) else {
        return Vec::new();
    };
    let mut chips: Vec<EChannel> = crate::lanes::DRUM_CHANNELS
        .into_iter()
        .filter(|ch| arr.map.get(ch) == Some(&lane.id))
        .collect();
    chips.sort_by_key(|ch| (*ch != lane.primary,));
    chips
}

/// Structural signature: changes when rows / chip sets / preset change (used
/// by the editor to know when to rebuild the panel vs just refresh values).
pub fn structure_signature(arr: &LaneArrangement) -> String {
    use std::fmt::Write;
    let mut s = format!("{:?}|", arr.preset);
    for (i, lane) in arr.lanes.iter().enumerate() {
        let _ = write!(s, "{};", lane.id);
        for ch in lane_chips(arr, i) {
            let _ = write!(s, "{},", channel_short_name(ch).unwrap_or("?"));
        }
        s.push('|');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lanes::DRUM_CHANNELS;
    use crate::presets::classic;

    fn assert_invariant(arr: &LaneArrangement) {
        assert!(!arr.lanes.is_empty());
        for ch in DRUM_CHANNELS {
            let idx = arr.lane_index_of(ch);
            assert!(idx.is_some(), "{ch:?} unmapped");
        }
    }

    #[test]
    fn reorder_swaps_and_flips_custom() {
        let mut arr = classic();
        let first = arr.lanes[0].id.clone();
        let second = arr.lanes[1].id.clone();
        assert!(reorder_lane(&mut arr, 0, 1));
        assert_eq!(arr.lanes[0].id, second);
        assert_eq!(arr.lanes[1].id, first);
        assert_eq!(arr.preset, LanePreset::Custom);
        assert_invariant(&arr);
    }

    #[test]
    fn reorder_off_the_edge_is_noop() {
        let mut arr = classic();
        assert!(!reorder_lane(&mut arr, 0, -1));
        let last = arr.lanes.len() - 1;
        assert!(!reorder_lane(&mut arr, last, 1));
        assert_eq!(arr.preset, LanePreset::Classic);
    }

    #[test]
    fn width_clamps() {
        let mut arr = classic();
        assert!(set_lane_width(&mut arr, 0, 999.0));
        assert_eq!(arr.lanes[0].width, MAX_LANE_WIDTH);
        assert!(set_lane_width(&mut arr, 0, 1.0));
        assert_eq!(arr.lanes[0].width, MIN_LANE_WIDTH);
    }

    #[test]
    fn split_hho_out_of_hh_lane() {
        let mut arr = classic();
        let before = arr.lanes.len();
        assert!(arr.is_secondary(EChannel::HiHatOpen));
        assert!(split_channel(&mut arr, EChannel::HiHatOpen));
        assert_eq!(arr.lanes.len(), before + 1);
        assert!(!arr.is_secondary(EChannel::HiHatOpen));
        let hh = arr.lane_index_of(EChannel::HiHatClose).unwrap();
        let hho = arr.lane_index_of(EChannel::HiHatOpen).unwrap();
        assert_eq!(hho, hh + 1, "split lane inserted right after host");
        assert_invariant(&arr);
    }

    #[test]
    fn split_primary_is_noop() {
        let mut arr = classic();
        assert!(!split_channel(&mut arr, EChannel::Snare));
        assert_eq!(arr.preset, LanePreset::Classic);
    }

    #[test]
    fn merge_rd_moves_chips_to_left_neighbor() {
        let mut arr = classic();
        let rd_idx = arr.lane_index_of(EChannel::RideCymbal).unwrap();
        let left_id = arr.lanes[rd_idx - 1].id.clone();
        let before = arr.lanes.len();
        assert!(merge_lane(&mut arr, rd_idx));
        assert_eq!(arr.lanes.len(), before - 1);
        assert_eq!(arr.map[&EChannel::RideCymbal], left_id);
        assert_invariant(&arr);
    }

    #[test]
    fn merge_leftmost_uses_right_neighbor() {
        let mut arr = classic();
        let right_id = arr.lanes[1].id.clone();
        let first_primary = arr.lanes[0].primary;
        assert!(merge_lane(&mut arr, 0));
        assert_eq!(arr.map[&first_primary], right_id);
        assert_invariant(&arr);
    }

    #[test]
    fn merge_last_lane_refused() {
        let mut arr = classic();
        while arr.lanes.len() > 1 {
            assert!(merge_lane(&mut arr, 0));
        }
        assert!(!merge_lane(&mut arr, 0));
        assert_invariant(&arr);
    }

    #[test]
    fn chips_list_primary_first() {
        let arr = classic();
        let hh = arr.lane_index_of(EChannel::HiHatClose).unwrap();
        let chips = lane_chips(&arr, hh);
        assert_eq!(chips[0], arr.lanes[hh].primary);
        assert!(chips.contains(&EChannel::HiHatOpen));
    }

    #[test]
    fn signature_changes_on_structure_not_width() {
        let mut arr = classic();
        let sig0 = structure_signature(&arr);
        set_lane_width(&mut arr, 0, 100.0);
        // Width change flips preset → signature changes once (Custom), then
        // further width edits keep it stable.
        let sig1 = structure_signature(&arr);
        assert_ne!(sig0, sig1);
        set_lane_width(&mut arr, 0, 120.0);
        assert_eq!(sig1, structure_signature(&arr));
        split_channel(&mut arr, EChannel::HiHatOpen);
        assert_ne!(sig1, structure_signature(&arr));
    }

    #[test]
    fn edited_arrangement_round_trips_through_file() {
        let mut arr = classic();
        split_channel(&mut arr, EChannel::HiHatOpen);
        reorder_lane(&mut arr, 0, 1);
        let last = arr.lanes.len() - 1;
        merge_lane(&mut arr, last);
        set_lane_width(&mut arr, 2, 88.0);
        let section = crate::LanesSection::from_arrangement(&arr);
        assert_eq!(section.resolve(), arr);
    }
}
