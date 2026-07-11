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

/// Remove lane `index` from the arrangement; every channel it hosted becomes
/// unassigned (absent from `arr.map`) and is returned so callers (Hidden strip)
/// can offer each for restore. Judgment is untouched — `lane_of` in
/// gameplay-drums routes scoring off the fixed logical order, never off this
/// display arrangement, so an unassigned channel still gets judged; it just
/// stops rendering a falling note / lane column.
/// No-op on the last remaining lane or an out-of-range index (returns empty,
/// arrangement untouched).
pub fn hide_lane(arr: &mut LaneArrangement, index: usize) -> Vec<EChannel> {
    if arr.lanes.len() <= 1 || index >= arr.lanes.len() {
        return Vec::new();
    }
    let removed = arr.lanes.remove(index);
    let hidden: Vec<EChannel> = crate::lanes::DRUM_CHANNELS
        .into_iter()
        .filter(|ch| arr.map.get(ch) == Some(&removed.id))
        .collect();
    for ch in &hidden {
        arr.map.remove(ch);
    }
    arr.preset = LanePreset::Custom;
    hidden
}

/// Every drum channel present in no lane, in canonical (`DRUM_CHANNELS`) order.
pub fn unassigned_channels(arr: &LaneArrangement) -> Vec<EChannel> {
    crate::lanes::DRUM_CHANNELS
        .into_iter()
        .filter(|ch| !arr.map.contains_key(ch))
        .collect()
}

/// Append a new default-width lane whose primary (and sole) channel is
/// `primary`. Returns false (no-op) when `primary` is already assigned to a
/// lane; true when it appended (or remapped a degenerate id).
pub fn restore_lane(arr: &mut LaneArrangement, primary: EChannel) -> bool {
    if arr.map.contains_key(&primary) {
        return false;
    }
    let Some(name) = channel_short_name(primary) else {
        return false;
    };
    // Degenerate state guard (mirrors split_channel): a lane with this id
    // already exists but the channel isn't mapped to it — just remap.
    if arr.lanes.iter().any(|l| l.id == name) {
        arr.map.insert(primary, name.to_string());
        arr.preset = LanePreset::Custom;
        return true;
    }
    arr.lanes.push(DisplayLane {
        id: name.to_string(),
        label: name.to_string(),
        width: default_lane_width(primary),
        color: None,
        primary,
    });
    arr.map.insert(primary, name.to_string());
    arr.preset = LanePreset::Custom;
    true
}

/// Move channel `ch` onto lane `index` (remaps `ch` → that lane's id), for
/// the Lanes detail card's "+ add" popup. No-op when `index` is out of
/// range, `ch` is already the lane's own primary (nothing to do), or `ch` is
/// currently the PRIMARY of some other lane — moving a primary away would
/// leave that lane empty; split/hide it first instead.
pub fn merge_channel_into_lane(arr: &mut LaneArrangement, ch: EChannel, index: usize) -> bool {
    let Some(lane) = arr.lanes.get(index) else {
        return false;
    };
    let id = lane.id.clone();
    if arr.map.get(&ch) == Some(&id) {
        return false;
    }
    if let Some(cur) = arr.lane_index_of(ch) {
        if arr.lanes[cur].primary == ch {
            return false;
        }
    }
    arr.map.insert(ch, id);
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
    // Deliberately excludes `preset`: any lane edit flips the preset to Custom,
    // and if that flip changed the signature the editor panel would rebuild
    // mid-width-drag and despawn the slider. Preset-cycle still changes this
    // signature (the built-in presets have distinct lane-id ORDERS), and the
    // preset label updates reactively, so nothing that needs a rebuild rides on
    // `preset` alone.
    let mut s = String::new();
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
    fn hide_lane_unassigns_its_channels_and_restore_reinserts() {
        let mut arr = classic();
        let n = arr.lanes.len();
        let hh_index = arr
            .lanes
            .iter()
            .position(|l| l.primary == EChannel::HiHatClose)
            .unwrap();
        let hidden = hide_lane(&mut arr, hh_index);
        assert_eq!(arr.lanes.len(), n - 1);
        assert!(hidden.contains(&EChannel::HiHatClose));
        // classic's HH lane also hosts HHO as a secondary chip.
        assert!(hidden.contains(&EChannel::HiHatOpen));
        assert!(unassigned_channels(&arr).contains(&EChannel::HiHatClose));
        assert_eq!(arr.preset, LanePreset::Custom);

        assert!(restore_lane(&mut arr, EChannel::HiHatClose));
        assert_eq!(arr.lanes.len(), n);
        assert!(!unassigned_channels(&arr).contains(&EChannel::HiHatClose));
        // Secondary (HHO) isn't auto-restored — only the primary is; it stays
        // unassigned until re-merged/split by the editor, same as spec says.
        assert!(unassigned_channels(&arr).contains(&EChannel::HiHatOpen));
    }

    #[test]
    fn hide_last_lane_is_refused() {
        let mut arr = classic();
        while arr.lanes.len() > 1 {
            hide_lane(&mut arr, 0);
        }
        assert!(hide_lane(&mut arr, 0).is_empty(), "cannot hide the only lane");
        assert_eq!(arr.lanes.len(), 1);
    }

    #[test]
    fn hide_lane_out_of_range_is_noop() {
        let mut arr = classic();
        let n = arr.lanes.len();
        assert!(hide_lane(&mut arr, n).is_empty());
        assert_eq!(arr.lanes.len(), n);
    }

    #[test]
    fn restore_lane_is_noop_when_already_assigned() {
        let mut arr = classic();
        let n = arr.lanes.len();
        assert!(!restore_lane(&mut arr, EChannel::Snare));
        assert_eq!(arr.lanes.len(), n, "SD already has a lane");
    }

    #[test]
    fn unassigned_channels_reports_none_for_a_complete_arrangement() {
        assert!(unassigned_channels(&classic()).is_empty());
    }

    #[test]
    fn merge_channel_into_lane_remaps_a_secondary() {
        let mut arr = classic();
        // HHO starts as a secondary chip on the HH lane.
        let hh = arr.lane_index_of(EChannel::HiHatClose).unwrap();
        let cy = arr.lane_index_of(EChannel::Cymbal).unwrap();
        assert!(merge_channel_into_lane(&mut arr, EChannel::HiHatOpen, cy));
        assert_eq!(arr.map[&EChannel::HiHatOpen], arr.lanes[cy].id);
        assert!(!lane_chips(&arr, hh).contains(&EChannel::HiHatOpen));
        assert_eq!(arr.preset, LanePreset::Custom);
        assert_invariant(&arr);
    }

    #[test]
    fn merge_channel_into_lane_reassigns_a_hidden_channel() {
        let mut arr = classic();
        let hh = arr.lane_index_of(EChannel::HiHatClose).unwrap();
        hide_lane(&mut arr, hh);
        assert!(unassigned_channels(&arr).contains(&EChannel::HiHatClose));
        let cy = arr.lane_index_of(EChannel::Cymbal).unwrap();
        assert!(merge_channel_into_lane(&mut arr, EChannel::HiHatClose, cy));
        assert!(!unassigned_channels(&arr).contains(&EChannel::HiHatClose));
        assert_eq!(arr.map[&EChannel::HiHatClose], arr.lanes[cy].id);
    }

    #[test]
    fn merge_channel_into_lane_refuses_to_empty_a_primary() {
        let mut arr = classic();
        let hh = arr.lane_index_of(EChannel::HiHatClose).unwrap();
        let cy = arr.lane_index_of(EChannel::Cymbal).unwrap();
        assert!(!merge_channel_into_lane(&mut arr, EChannel::HiHatClose, cy));
        assert_eq!(arr.map[&EChannel::HiHatClose], arr.lanes[hh].id);
        assert_eq!(arr.preset, LanePreset::Classic);
    }

    #[test]
    fn merge_channel_into_lane_is_noop_when_already_there() {
        let mut arr = classic();
        let hh = arr.lane_index_of(EChannel::HiHatClose).unwrap();
        assert!(!merge_channel_into_lane(&mut arr, EChannel::HiHatOpen, hh));
        assert_eq!(arr.preset, LanePreset::Classic);
    }

    #[test]
    fn width_already_clamps_to_the_shared_floor() {
        // MIN_LANE_WIDTH lives in `lanes.rs` and is already enforced by
        // `set_lane_width` (see `width_clamps` above) — no second floor needed.
        let mut arr = classic();
        set_lane_width(&mut arr, 0, 0.0);
        assert_eq!(arr.lanes[0].width, MIN_LANE_WIDTH);
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
        // Signature captured on the NAMED preset, BEFORE any edit — a width
        // change (which flips Classic→Custom) must NOT change it, or the panel
        // rebuilds mid-drag. Only structural edits (split/reorder/merge) do.
        let sig0 = structure_signature(&arr);
        set_lane_width(&mut arr, 0, 100.0);
        assert_eq!(arr.preset, LanePreset::Custom, "width edit flips preset");
        assert_eq!(
            sig0,
            structure_signature(&arr),
            "width edit keeps signature"
        );
        set_lane_width(&mut arr, 0, 120.0);
        assert_eq!(sig0, structure_signature(&arr));
        split_channel(&mut arr, EChannel::HiHatOpen);
        assert_ne!(sig0, structure_signature(&arr), "split is structural");
    }

    #[test]
    fn signature_distinguishes_named_presets() {
        // The panel-rebuild fix drops `preset` from the signature and relies on
        // the built-in presets having distinct lane-id orders so a preset cycle
        // still triggers a rebuild.
        let c = structure_signature(&classic());
        let b = structure_signature(&crate::presets::nx_type_b());
        let d = structure_signature(&crate::presets::nx_type_d());
        assert_ne!(c, b);
        assert_ne!(c, d);
        assert_ne!(b, d);
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
