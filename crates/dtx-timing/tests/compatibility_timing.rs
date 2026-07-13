use dtx_core::timing::{bar_changes_from_chart, bpm_changes_from_chart};

#[test]
fn parsed_bpm_and_bar_changes_keep_expected_timeline() {
    let chart = dtx_core::parse_str(
        "#BPM: 120\n#BPM01: 240\n#00002: 0.5\n#00008: 01\n\
         #00013: 0A\n#00113: 0A\n#00213: 0A\n",
    )
    .expect("timing fixture parses");
    let bpm = bpm_changes_from_chart(&chart);
    let bars = bar_changes_from_chart(&chart);
    let timing = dtx_timing::math::ChartTiming {
        bpm_changes: &bpm,
        bar_changes: &bars,
    };
    let times: Vec<_> = chart
        .drum_chips()
        .map(|chip| {
            dtx_timing::math::chip_time_ms_with_bpm_and_bar_changes(
                chip.measure,
                chip.value,
                120.0,
                timing,
            )
        })
        .collect();
    assert_eq!(times, vec![2_000, 2_500, 3_000]);
    assert!(times.windows(2).all(|pair| pair[0] < pair[1]));
}
