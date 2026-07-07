use dtx_scoring::score_ini::{parse_score_ini_text, DrumScoreIni};

fn sample_score_ini() -> &'static str {
    r#"[File]
Title=Sample
Name=Tester
PlayCountDrums=7
PlayCountGuitars=0
PlayCountBass=0
ClearCountDrums=3
ClearCountGuitars=0
ClearCountBass=0
BestRankDrums=1
BestRankGuitar=99
BestRankBass=99
HistoryCount=2
History0=2.26/7/8 Stage cleared
History1=1.26/7/7 Stage failed
BGMAdjust=-12

[HiScore.Drums]
Score=900000
Perfect=80
Great=10
Good=5
Poor=3
Miss=2
MaxCombo=88
TotalChips=100
Drums=1
DateTime=2026/7/8 10:11:12

[LastPlay.Drums]
Score=800000
Perfect=70
Great=15
Good=5
Poor=5
Miss=5
MaxCombo=66
TotalChips=100
Drums=1
DateTime=2026/7/8 11:12:13
"#
}

#[test]
fn parses_file_history_best_and_last_play() {
    let parsed = parse_score_ini_text(sample_score_ini()).unwrap();

    assert_eq!(parsed.file.play_count_drums, 7);
    assert_eq!(parsed.file.clear_count_drums, 3);
    assert_eq!(parsed.file.bgm_adjust, -12);
    assert_eq!(
        parsed.file.history,
        vec!["2.26/7/8 Stage cleared", "1.26/7/7 Stage failed"]
    );

    assert_eq!(parsed.hi_score_drums.as_ref().unwrap().score, 900000);
    assert_eq!(parsed.last_play_drums.as_ref().unwrap().score, 800000);
}

#[test]
fn rendered_score_ini_keeps_history_fields() {
    let best = DrumScoreIni {
        score: 100,
        perfect: 10,
        great: 0,
        good: 0,
        poor: 0,
        miss: 0,
        max_combo: 10,
        total_chips: 10,
        rank: "SS".to_string(),
        play_count: 2,
        clear_count: 1,
        bgm_adjust: 5,
        date_time: "2026/7/8 1:02:03".to_string(),
    };
    let text = dtx_scoring::score_ini::render_with_history(
        &best,
        &best,
        &[
            "2.26/7/8 Stage cleared".to_string(),
            "1.26/7/7 Stage failed".to_string(),
        ],
    );
    let parsed = parse_score_ini_text(&text).unwrap();

    assert_eq!(parsed.file.history_count, 2);
    assert_eq!(parsed.file.history[0], "2.26/7/8 Stage cleared");
    assert_eq!(parsed.hi_score_drums.unwrap().score, 100);
}
