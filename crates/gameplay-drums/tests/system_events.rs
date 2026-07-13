use dtx_core::{Chart, Chip, EChannel};
use gameplay_drums::system_events::{SystemEventCursor, SystemEventKind, SystemEventSchedule};
use gameplay_drums::timeline::ChipTimeline;

fn chart_and_timeline() -> (Chart, ChipTimeline) {
    let chart = Chart {
        chips: vec![
            Chip::with_wav(0, EChannel::HiHatCloseHidden, 0.25, 1),
            Chip::with_wav(0, EChannel::MIDIChorus, 0.50, 2),
            Chip::with_wav(0, EChannel::FillIn, 0.75, 3),
            Chip::with_wav(1, EChannel::Click, 0.00, 4),
            Chip::with_wav(1, EChannel::FirstSoundChip, 0.25, 5),
            Chip::with_wav(1, EChannel::Snare, 0.50, 6),
        ],
        ..Default::default()
    };
    let timeline = ChipTimeline {
        entries: vec![],
        judge_ms_by_idx: vec![100, 200, 300, 400, 500, 600],
        ..Default::default()
    };
    (chart, timeline)
}

#[test]
fn schedule_classifies_system_events_without_counting_notes() {
    let (chart, timeline) = chart_and_timeline();
    let schedule = SystemEventSchedule::from_chart(&chart, &timeline);

    assert_eq!(chart.drum_chips().count(), 1);
    assert_eq!(schedule.events.len(), 5);
    assert!(matches!(
        schedule.events[0].kind,
        SystemEventKind::Hidden {
            sound_lane: EChannel::HiHatClose
        }
    ));
    assert_eq!(schedule.events[1].kind, SystemEventKind::MidiChorus);
    assert_eq!(schedule.events[2].kind, SystemEventKind::FillIn);
    assert_eq!(schedule.events[3].kind, SystemEventKind::Click);
    assert_eq!(schedule.events[4].kind, SystemEventKind::FirstSound);
}

#[test]
fn cursor_consumes_forward_once_and_rebuilds_on_backward_seek() {
    let (chart, timeline) = chart_and_timeline();
    let schedule = SystemEventSchedule::from_chart(&chart, &timeline);
    let mut cursor = SystemEventCursor::default();

    assert_eq!(cursor.advance_to(&schedule, 250).len(), 2);
    assert!(cursor.advance_to(&schedule, 250).is_empty());
    assert_eq!(cursor.advance_to(&schedule, 450).len(), 2);
    assert!(cursor.advance_to(&schedule, 150).is_empty());
    assert_eq!(cursor.advance_to(&schedule, 350).len(), 2);
}

#[test]
fn system_events_are_absent_from_note_density() {
    let (chart, _) = chart_and_timeline();
    let timeline = ChipTimeline::from_chart(
        &chart,
        &gameplay_drums::judge::BpmChangeList::from_chart(&chart),
        &gameplay_drums::judge::BarLengthChangeList::from_chart(&chart),
        0,
        4_000,
    );

    assert_eq!(
        timeline
            .density
            .iter()
            .filter(|value| **value > 0.0)
            .count(),
        1
    );
}
