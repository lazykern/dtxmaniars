//! BocuD-compatible `.score.ini` persistence for drums.
//!
//! DTXManiaNX writes a `<chart>.score.ini` next to each chart (Shift-JIS,
//! INI-style sections). This is a bounded port of the drums-relevant sections
//! that BocuD reads back for the song-select best-score panel and the result
//! screen.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/CScoreIni.cs`
//! (`[File]`, `[HiScore.Drums]`, `[LastPlay.Drums]` sections).
//!
//! Scope notes:
//! - Only the drums high-score / last-play sections are modelled. The
//!   guitar/bass sections BocuD writes are emitted empty for compatibility.
//! - Values we write are ASCII, so UTF-8 bytes equal the Shift-JIS encoding.
//!   Reading uses a lossy decode: numeric/ASCII fields (all we consume) parse
//!   correctly even if a foreign `Title=`/`Name=` line is present.
//! - The BocuD anti-tamper MD5 `Hash=` field is not reproduced; BocuD still
//!   reads the numeric fields when the hash is absent.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::Rank;

/// A drums score record backing one `.score.ini` file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DrumScoreIni {
    /// Best (or current) score value.
    pub score: u32,
    /// Judgment tallies.
    pub perfect: u32,
    /// Judgment tallies.
    pub great: u32,
    /// Judgment tallies.
    pub good: u32,
    /// Judgment tallies (BocuD "Poor").
    pub poor: u32,
    /// Judgment tallies.
    pub miss: u32,
    /// Maximum combo.
    pub max_combo: u32,
    /// Total judgeable chips.
    pub total_chips: u32,
    /// Rank name (`SS`/`S`/`A`.../`UNKNOWN`).
    pub rank: String,
    /// Times this chart has been played (drums).
    pub play_count: u32,
    /// Times this chart has been cleared (drums).
    pub clear_count: u32,
    /// Per-song BGM auto-chip offset (`[File] BGMAdjust=`).
    pub bgm_adjust: i32,
    /// `YYYY/M/D H:M:S` timestamp of the record.
    pub date_time: String,
}

/// Parsed `[File]` metadata from DTXManiaNX `.score.ini`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScoreIniFileSection {
    /// Title field.
    pub title: String,
    /// Name field.
    pub name: String,
    /// Hash field when present.
    pub hash: String,
    /// Drums play count.
    pub play_count_drums: u32,
    /// Drums clear count.
    pub clear_count_drums: u32,
    /// Best rank code for drums.
    pub best_rank_drums: i32,
    /// Number of history entries reported by NX.
    pub history_count: u32,
    /// History0..History4 values.
    pub history: Vec<String>,
    /// BGM adjust.
    pub bgm_adjust: i32,
}

/// Parsed drums-focused `.score.ini`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedScoreIni {
    /// File section.
    pub file: ScoreIniFileSection,
    /// HiScore.Drums section.
    pub hi_score_drums: Option<DrumScoreIni>,
    /// LastPlay.Drums section.
    pub last_play_drums: Option<DrumScoreIni>,
}

impl DrumScoreIni {
    /// Perfect+Great accuracy (0..100).
    pub fn accuracy(&self) -> f32 {
        let total = self.perfect + self.great + self.good + self.poor + self.miss;
        if total == 0 {
            0.0
        } else {
            100.0 * (self.perfect + self.great) as f32 / total as f32
        }
    }

    /// Weighted achievement percentage used in the player-facing UI.
    pub fn achievement_pct(&self) -> f32 {
        let total = self.perfect + self.great + self.good + self.poor + self.miss;
        if total == 0 {
            0.0
        } else {
            let weighted = self.perfect as f32 * 100.0
                + self.great as f32 * 80.0
                + self.good as f32 * 60.0
                + self.poor as f32 * 40.0;
            weighted / total as f32
        }
    }

    /// True when `self` is a better result than `other` (score, then accuracy,
    /// then combo). Mirrors BocuD's hi-score replacement rule.
    fn beats(&self, other: &DrumScoreIni) -> bool {
        if self.score != other.score {
            return self.score > other.score;
        }
        let (a, b) = (self.accuracy(), other.accuracy());
        if (a - b).abs() > f32::EPSILON {
            return a > b;
        }
        self.max_combo > other.max_combo
    }
}

/// `<chart>.score.ini` next to the chart file (BocuD suffix convention).
pub fn score_ini_path(chart_path: impl AsRef<Path>) -> PathBuf {
    let mut os = chart_path.as_ref().as_os_str().to_os_string();
    os.push(".score.ini");
    PathBuf::from(os)
}

/// Numeric rank code BocuD uses in `[File] BestRankDrums` (`SS=0 … E=6`, 99=unknown).
pub fn rank_code(rank: &str) -> i32 {
    match rank {
        "SS" => 0,
        "S" => 1,
        "A" => 2,
        "B" => 3,
        "C" => 4,
        "D" => 5,
        "E" => 6,
        _ => 99,
    }
}

/// Inverse of [`rank_code`].
pub fn rank_name(code: i32) -> &'static str {
    match code {
        0 => "SS",
        1 => "S",
        2 => "A",
        3 => "B",
        4 => "C",
        5 => "D",
        6 => "E",
        _ => "UNKNOWN",
    }
}

/// Convert the crate [`Rank`] enum to its BocuD name.
pub fn rank_from_enum(rank: Rank) -> String {
    rank.to_string()
}

/// Read `[File] BGMAdjust=` from a `.score.ini` (0 when missing).
pub fn read_bgm_adjust(path: impl AsRef<Path>) -> i32 {
    let bytes = match std::fs::read(path.as_ref()) {
        Ok(b) => b,
        Err(_) => return 0,
    };
    let text = String::from_utf8_lossy(&bytes);
    let sections = parse_sections(&text);
    sections
        .get("File")
        .map(|f| get_i32(f, "BGMAdjust", 0))
        .unwrap_or(0)
}

/// One ghost entry: lag in ms (signed i16 range, high bit = combo-break marker
/// in BocuD). Reads the binary `.ghost` file format used by
/// `CStageSongLoading.ReadGhost` (CDTX.cs:1038-1080). Returns `None` if the
/// file is missing or malformed.
///
/// File layout:
/// - `i32` count
/// - `count` × `i16` lag values (little-endian)
pub fn read_ghost_lag(path: impl AsRef<Path>) -> Option<Vec<i16>> {
    use std::io::Read;
    let mut file = std::fs::File::open(path.as_ref()).ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    if buf.len() < 4 {
        return None;
    }
    let count = i32::from_le_bytes(buf[0..4].try_into().ok()?) as usize;
    let needed = 4 + count.saturating_mul(2);
    if buf.len() < needed {
        return None;
    }
    let mut lags = Vec::with_capacity(count);
    for i in 0..count {
        let off = 4 + i * 2;
        let bytes: [u8; 2] = buf[off..off + 2].try_into().ok()?;
        lags.push(i16::from_le_bytes(bytes));
    }
    Some(lags)
}

/// BocuD ghost prefix list (CStageSongLoading.cs:269). Order matches
/// `ETargetGhostData` / `EAutoGhost` enum values.
pub const GHOST_PREFIXES: &[&str] = &[
    "none", "perfect", "lastplay", "hiskill", "hiscore", "online",
];

/// Resolve the path to a `.ghost` file for a chart + instrument + target kind.
///
/// `instrument` ∈ {`"dr"`, `"gt"`, `"bs"`}, `kind` ∈ `GHOST_PREFIXES`.
/// Example: `ghost_path("foo.dtx", "dr", "perfect")` → `foo.perfect.dr.ghost`.
pub fn ghost_path(
    chart_path: impl AsRef<Path>,
    instrument: &str,
    kind: &str,
) -> std::path::PathBuf {
    let mut s = chart_path.as_ref().as_os_str().to_os_string();
    s.push(format!(".{kind}.{instrument}.ghost"));
    std::path::PathBuf::from(s)
}

/// Update `[File] BGMAdjust=` in an existing `.score.ini` (preserves other sections).
pub fn write_bgm_adjust(path: impl AsRef<Path>, bgm_adjust: i32) -> std::io::Result<()> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut sections = parse_sections(&text);
    let file = sections.entry("File".to_string()).or_default();
    file.insert("BGMAdjust".to_string(), bgm_adjust.to_string());
    file.entry("PlayCountDrums".to_string())
        .or_insert_with(|| "0".to_string());
    file.entry("ClearCountDrums".to_string())
        .or_insert_with(|| "0".to_string());
    file.entry("BestRankDrums".to_string())
        .or_insert_with(|| "99".to_string());

    let mut out = render_file_header(file);
    let mut names: Vec<_> = sections.keys().filter(|n| *n != "File").cloned().collect();
    names.sort();
    for name in names {
        let section = &sections[&name];
        out.push_str(&format!("[{name}]\n"));
        for (k, v) in section {
            out.push_str(&format!("{k}={v}\n"));
        }
        out.push('\n');
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, out.into_bytes())
}

fn render_file_header(file: &HashMap<String, String>) -> String {
    let play = get_u32(file, "PlayCountDrums");
    let clear = get_u32(file, "ClearCountDrums");
    let rank = get_i32(file, "BestRankDrums", 99);
    let bgm = get_i32(file, "BGMAdjust", 0);
    format!(
        "[File]\nTitle=\nName=\nPlayCountDrums={play}\nPlayCountGuitars=0\nPlayCountBass=0\nClearCountDrums={clear}\nClearCountGuitars=0\nClearCountBass=0\nBestRankDrums={rank}\nBestRankGuitar=99\nBestRankBass=99\nHistoryCount=0\nBGMAdjust={bgm}\n\n"
    )
}

/// Read the best drums score from a `.score.ini`, if present and parseable.
pub fn read_best(path: impl AsRef<Path>) -> Option<DrumScoreIni> {
    let bytes = std::fs::read(path.as_ref()).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    parse_best(&text)
}

/// Parse drums-focused DTXManiaNX score.ini text.
pub fn parse_score_ini_text(text: &str) -> Option<ParsedScoreIni> {
    let sections = parse_sections(text);
    let file = parse_file_section(sections.get("File"));
    let hi_score_drums = sections
        .get("HiScore.Drums")
        .map(|section| parse_drum_section(section, &file));
    let last_play_drums = sections
        .get("LastPlay.Drums")
        .map(|section| parse_drum_section(section, &file));
    Some(ParsedScoreIni {
        file,
        hi_score_drums,
        last_play_drums,
    })
}

/// Merge `result` into the existing `.score.ini` at `path` (best-of + play/clear
/// counts) and write it back. `cleared` bumps the clear counter.
pub fn write_result(
    path: impl AsRef<Path>,
    result: &DrumScoreIni,
    cleared: bool,
) -> std::io::Result<()> {
    let path = path.as_ref();
    let existing = read_best(path);

    let (play_count, clear_count, best) = match existing {
        Some(prev) => {
            let play = prev.play_count.saturating_add(1);
            let clear = prev.clear_count + u32::from(cleared);
            let best = if result.beats(&prev) {
                result.clone()
            } else {
                prev
            };
            (play, clear, best)
        }
        None => (1, u32::from(cleared), result.clone()),
    };

    let mut best = best;
    best.play_count = play_count;
    best.clear_count = clear_count;

    let text = render(&best, result);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text.into_bytes())
}

/// Render `[File]` + drums hi-score/last-play sections. Non-drums sections are
/// emitted empty for BocuD compatibility.
fn render(best: &DrumScoreIni, last: &DrumScoreIni) -> String {
    render_internal(best, last, &[])
}

/// Render `[File]` + drums sections with preserved history lines.
pub fn render_with_history(best: &DrumScoreIni, last: &DrumScoreIni, history: &[String]) -> String {
    render_internal(best, last, history)
}

fn render_internal(best: &DrumScoreIni, last: &DrumScoreIni, history: &[String]) -> String {
    let rank = rank_code(&best.rank);
    let mut text = format!(
        "[File]\nTitle=\nName=\nPlayCountDrums={play}\nPlayCountGuitars=0\nPlayCountBass=0\nClearCountDrums={clear}\nClearCountGuitars=0\nClearCountBass=0\nBestRankDrums={rank}\nBestRankGuitar=99\nBestRankBass=99\nHistoryCount={history_count}\n",
        play = best.play_count,
        clear = best.clear_count,
        rank = rank,
        history_count = history.len().min(5),
    );
    for idx in 0..5 {
        let value = history.get(idx).map(String::as_str).unwrap_or("");
        text.push_str(&format!("History{idx}={value}\n"));
    }
    text.push_str(&format!("BGMAdjust={}\n\n", best.bgm_adjust));
    render_section(&mut text, "HiScore.Drums", best);
    render_section(&mut text, "HiSkill.Drums", best);
    render_section(&mut text, "LastPlay.Drums", last);
    text
}

fn render_section(text: &mut String, section: &str, s: &DrumScoreIni) {
    text.push_str(&format!(
        "[{section}]\nScore={score}\nPerfect={perfect}\nGreat={great}\nGood={good}\nPoor={poor}\nMiss={miss}\nMaxCombo={max_combo}\nTotalChips={total_chips}\nDrums=1\nDateTime={date_time}\n\n",
        score = s.score,
        perfect = s.perfect,
        great = s.great,
        good = s.good,
        poor = s.poor,
        miss = s.miss,
        max_combo = s.max_combo,
        total_chips = s.total_chips,
        date_time = s.date_time,
    ));
}

fn parse_best(text: &str) -> Option<DrumScoreIni> {
    parse_score_ini_text(text)?.hi_score_drums
}

fn parse_file_section(section: Option<&HashMap<String, String>>) -> ScoreIniFileSection {
    let Some(section) = section else {
        return ScoreIniFileSection::default();
    };
    let history_count = get_u32(section, "HistoryCount");
    let mut history = Vec::new();
    for idx in 0..5 {
        let key = format!("History{idx}");
        if let Some(value) = section.get(&key) {
            if !value.is_empty() {
                history.push(value.clone());
            }
        }
    }
    ScoreIniFileSection {
        title: section.get("Title").cloned().unwrap_or_default(),
        name: section.get("Name").cloned().unwrap_or_default(),
        hash: section.get("Hash").cloned().unwrap_or_default(),
        play_count_drums: get_u32(section, "PlayCountDrums"),
        clear_count_drums: get_u32(section, "ClearCountDrums"),
        best_rank_drums: get_i32(section, "BestRankDrums", 99),
        history_count,
        history,
        bgm_adjust: get_i32(section, "BGMAdjust", 0),
    }
}

fn parse_drum_section(drums: &HashMap<String, String>, file: &ScoreIniFileSection) -> DrumScoreIni {
    DrumScoreIni {
        score: get_u32(drums, "Score"),
        perfect: get_u32(drums, "Perfect"),
        great: get_u32(drums, "Great"),
        good: get_u32(drums, "Good"),
        poor: get_u32(drums, "Poor"),
        miss: get_u32(drums, "Miss"),
        max_combo: get_u32(drums, "MaxCombo"),
        total_chips: get_u32(drums, "TotalChips"),
        rank: rank_name(file.best_rank_drums).to_string(),
        play_count: file.play_count_drums,
        clear_count: file.clear_count_drums,
        bgm_adjust: file.bgm_adjust,
        date_time: drums.get("DateTime").cloned().unwrap_or_default(),
    }
}

fn parse_sections(text: &str) -> HashMap<String, HashMap<String, String>> {
    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut current = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            current = name.trim().to_string();
            sections.entry(current.clone()).or_default();
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            sections
                .entry(current.clone())
                .or_default()
                .insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    sections
}

fn get_u32(section: &HashMap<String, String>, key: &str) -> u32 {
    section.get(key).and_then(|v| v.parse().ok()).unwrap_or(0)
}

fn get_i32(section: &HashMap<String, String>, key: &str, default: i32) -> i32 {
    section
        .get(key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Format a `YYYY/M/D H:M:S` timestamp from Unix seconds (UTC), matching
/// BocuD's `DateTime` field style. Uses Howard Hinnant's civil-from-days
/// algorithm so we avoid a chrono dependency in this Pure crate.
pub fn format_datetime(unix_secs: u64) -> String {
    let days = (unix_secs / 86_400) as i64;
    let secs_of_day = unix_secs % 86_400;
    let (hour, minute, second) = (
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60,
    );

    // civil_from_days (days since 1970-01-01) → (year, month, day).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };

    format!("{year}/{month}/{day} {hour}:{minute:02}:{second:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> DrumScoreIni {
        DrumScoreIni {
            score: 987_654,
            perfect: 100,
            great: 20,
            good: 3,
            poor: 2,
            miss: 1,
            max_combo: 111,
            total_chips: 126,
            rank: "S".into(),
            play_count: 1,
            clear_count: 1,
            bgm_adjust: 0,
            date_time: "2026/6/21 12:34:56".into(),
        }
    }

    #[test]
    fn path_appends_bocud_suffix() {
        assert_eq!(
            score_ini_path("/songs/x.dtx"),
            PathBuf::from("/songs/x.dtx.score.ini")
        );
    }

    #[test]
    fn achievement_weights_all_judgments() {
        let score = DrumScoreIni {
            perfect: 1,
            great: 1,
            good: 1,
            poor: 1,
            miss: 1,
            ..Default::default()
        };

        assert!((score.achievement_pct() - 56.0).abs() < f32::EPSILON);
    }

    #[test]
    fn render_then_parse_round_trips_key_fields() {
        let s = sample();
        let text = render(&s, &s);
        let parsed = parse_best(&text).unwrap();
        assert_eq!(parsed.score, s.score);
        assert_eq!(parsed.perfect, s.perfect);
        assert_eq!(parsed.miss, s.miss);
        assert_eq!(parsed.max_combo, s.max_combo);
        assert_eq!(parsed.total_chips, s.total_chips);
        assert_eq!(parsed.rank, "S");
        assert_eq!(parsed.play_count, 1);
        assert_eq!(parsed.clear_count, 1);
    }

    #[test]
    fn beats_prefers_higher_score() {
        let mut lo = sample();
        lo.score = 100;
        let mut hi = sample();
        hi.score = 200;
        assert!(hi.beats(&lo));
        assert!(!lo.beats(&hi));
    }

    #[test]
    fn write_result_increments_play_and_keeps_best() {
        let dir = std::env::temp_dir().join(format!("dtx_scoreini_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("chart.dtx.score.ini");
        let _ = std::fs::remove_file(&path);

        let mut first = sample();
        first.score = 500;
        write_result(&path, &first, true).unwrap();

        let mut worse = sample();
        worse.score = 100;
        write_result(&path, &worse, false).unwrap();

        let best = read_best(&path).unwrap();
        assert_eq!(best.score, 500, "best score must be retained");
        assert_eq!(best.play_count, 2, "play count increments each write");
        assert_eq!(best.clear_count, 1, "only the cleared play counts");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn format_datetime_epoch_is_1970() {
        assert_eq!(format_datetime(0), "1970/1/1 0:00:00");
    }

    #[test]
    fn format_datetime_known_value() {
        // 2021-01-01 00:00:00 UTC = 1609459200.
        assert_eq!(format_datetime(1_609_459_200), "2021/1/1 0:00:00");
    }

    #[test]
    fn read_and_write_bgm_adjust_round_trip() {
        let dir = std::env::temp_dir().join(format!("dtx_bgm_adj_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("chart.dtx.score.ini");
        let _ = std::fs::remove_file(&path);

        write_bgm_adjust(&path, 15).unwrap();
        assert_eq!(read_bgm_adjust(&path), 15);

        write_bgm_adjust(&path, -20).unwrap();
        assert_eq!(read_bgm_adjust(&path), -20);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_ghost_lag_round_trip() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("dtx_ghost_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("chart.dtx.perfect.dr.ghost");
        let _ = std::fs::remove_file(&path);

        let lags: Vec<i16> = vec![-5, 12, -128, 255];
        let mut buf = Vec::new();
        buf.extend_from_slice(&(lags.len() as i32).to_le_bytes());
        for v in &lags {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        std::fs::File::create(&path)
            .unwrap()
            .write_all(&buf)
            .unwrap();

        let read = read_ghost_lag(&path).expect("ghost file must read");
        assert_eq!(read, lags);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_ghost_lag_missing_file_returns_none() {
        let dir = std::env::temp_dir().join(format!("dtx_ghost_miss_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("nope.dtx.perfect.dr.ghost");
        let _ = std::fs::remove_file(&path);
        assert!(read_ghost_lag(&path).is_none());
    }

    #[test]
    fn ghost_path_appends_prefix_and_inst() {
        let p = ghost_path("/songs/foo/bar.dtx", "dr", "perfect");
        assert_eq!(p.to_str().unwrap(), "/songs/foo/bar.dtx.perfect.dr.ghost");
    }
}
