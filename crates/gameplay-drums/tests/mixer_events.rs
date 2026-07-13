use dtx_core::EChannel;
use gameplay_drums::mixer_events::{
    apply_mixer_event, rebuild_mixer_at, MixerAction, MixerEligibility, MixerEvent,
    MixerEventCursor, MixerEventKind,
};

fn add(at_ms: i64, slot: u32) -> MixerEvent {
    MixerEvent {
        at_ms,
        slot,
        kind: MixerEventKind::Add,
    }
}

fn remove(at_ms: i64, slot: u32) -> MixerEvent {
    MixerEvent {
        at_ms,
        slot,
        kind: MixerEventKind::Remove,
    }
}

#[test]
fn channel_values_match_the_chart_contract() {
    assert_eq!(EChannel::from_byte(0xEE), Some(EChannel::MixerAdd));
    assert_eq!(EChannel::from_byte(0xEF), Some(EChannel::MixerRemove));
}

#[test]
fn repeated_add_remove_is_idempotent_and_seek_rebuilds_state() {
    let events = [
        add(1_000, 5),
        add(1_000, 5),
        remove(2_000, 5),
        add(3_000, 7),
    ];
    let at_1500 = rebuild_mixer_at(&events, 1_500);
    assert!(at_1500.is_slot_eligible(5));
    let at_2500 = rebuild_mixer_at(&events, 2_500);
    assert!(!at_2500.is_slot_eligible(5));
    assert!(!at_2500.is_slot_eligible(7));
}

#[test]
fn remove_does_not_request_a_choke_for_an_active_voice() {
    let mut eligibility = MixerEligibility::restricted();
    assert_eq!(
        apply_mixer_event(&mut eligibility, remove(2_000, 5)),
        MixerAction::EligibilityOnly
    );
}

#[test]
fn charts_without_mixer_events_keep_every_slot_eligible() {
    let eligibility = rebuild_mixer_at(&[], 25_000);
    assert!(eligibility.is_slot_eligible(1));
    assert!(eligibility.is_slot_eligible(999));
}

#[test]
fn cursor_advances_once_and_reconstructs_on_backward_seek() {
    let events = vec![add(1_000, 5), remove(2_000, 5), add(3_000, 7)];
    let mut cursor = MixerEventCursor::new(events);
    let mut eligibility = MixerEligibility::restricted();

    cursor.advance_to(1_500, &mut eligibility);
    assert!(eligibility.is_slot_eligible(5));
    cursor.advance_to(2_500, &mut eligibility);
    assert!(!eligibility.is_slot_eligible(5));
    cursor.advance_to(1_500, &mut eligibility);
    assert!(eligibility.is_slot_eligible(5));
}
