//! DTXManiaNX `.score.ini` import.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::identity::{canonical_chart_hash, raw_file_sha256, ChartIdentity};
use crate::score_ini::{parse_score_ini_text, DrumScoreIni};
use crate::store::{JudgmentTotals, NxImportRecord, ScoreEntry, ScoreSource, ScoreStore};
use crate::Rank;

/// Import options.
#[derive(Debug, Clone)]
pub struct ImportOptions {
    /// Directory to scan recursively.
    pub root: PathBuf,
}

/// Import summary.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ImportReport {
    /// Found score.ini files.
    pub scanned_score_inis: u32,
    /// Imported score entries.
    pub imported_entries: u32,
    /// Files skipped due to malformed content.
    pub skipped_malformed: u32,
    /// `.score.ini` files with no paired `.dtx`.
    pub missing_charts: u32,
}

/// Import errors.
#[derive(Debug, Error)]
pub enum ImportError {
    /// I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Chart parse error.
    #[error("chart parse error: {0}")]
    Parse(#[from] dtx_core::DtxError),
}

/// Import DTXManiaNX score files into a store.
pub fn import_nx_scores(
    store: &mut ScoreStore,
    options: ImportOptions,
) -> Result<ImportReport, ImportError> {
    let mut report = ImportReport::default();
    let mut files = Vec::new();
    collect_score_ini_files(&options.root, &mut files)?;
    files.sort();

    for score_ini_path in files {
        report.scanned_score_inis += 1;
        let chart_path = chart_path_for_score_ini(&score_ini_path);
        if !chart_path.exists() {
            report.missing_charts += 1;
            continue;
        }

        let bytes = std::fs::read(&score_ini_path)?;
        let text = String::from_utf8_lossy(&bytes);
        let Some(parsed) = parse_score_ini_text(&text) else {
            report.skipped_malformed += 1;
            continue;
        };

        let file = File::open(&chart_path)?;
        let chart = dtx_core::parse(BufReader::new(file))?;
        let raw = raw_file_sha256(&chart_path).ok();
        let identity = ChartIdentity::new(canonical_chart_hash(&chart), raw, Some(chart_path));

        let title = chart
            .metadata
            .title
            .clone()
            .unwrap_or_else(|| parsed.file.title.clone());
        let artist = chart.metadata.artist.clone().unwrap_or_default();
        let chart_level = chart
            .metadata
            .dlevel
            .map(dtx_core::display_dlevel)
            .map(f64::from)
            .unwrap_or(0.0);

        let before = store.entries.len();
        if let Some(best) = parsed.hi_score_drums.clone() {
            store.add_if_new(entry_from_ini(
                &identity,
                &title,
                &artist,
                &best,
                chart_level,
                ScoreSource::ImportedNxHiScore,
            ));
        }
        if let Some(best_skill) = parsed.hi_skill_drums.clone() {
            store.add_if_new(entry_from_ini(
                &identity,
                &title,
                &artist,
                &best_skill,
                chart_level,
                ScoreSource::ImportedNxHiSkill,
            ));
        }
        if let Some(last) = parsed.last_play_drums.clone() {
            store.add_if_new(entry_from_ini(
                &identity,
                &title,
                &artist,
                &last,
                chart_level,
                ScoreSource::ImportedNxLastPlay,
            ));
        }
        report.imported_entries += (store.entries.len() - before) as u32;

        if !store.nx_imports.iter().any(|record| {
            record.chart.canonical_hash == identity.canonical_hash
                && record.score_ini_path == score_ini_path
        }) {
            store.nx_imports.push(NxImportRecord {
                chart: identity,
                score_ini_path,
                play_count: parsed.file.play_count_drums,
                clear_count: parsed.file.clear_count_drums,
                bgm_adjust: parsed.file.bgm_adjust,
                history: parsed.file.history,
            });
        }
    }

    Ok(report)
}

fn collect_score_ini_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_score_ini_files(&path, out)?;
        } else if path.to_string_lossy().ends_with(".dtx.score.ini") {
            out.push(path);
        }
    }
    Ok(())
}

fn chart_path_for_score_ini(score_ini_path: &Path) -> PathBuf {
    let text = score_ini_path.to_string_lossy();
    let chart = text.strip_suffix(".score.ini").unwrap_or(&text);
    PathBuf::from(chart)
}

fn entry_from_ini(
    identity: &ChartIdentity,
    title: &str,
    artist: &str,
    ini: &DrumScoreIni,
    chart_level: f64,
    source: ScoreSource,
) -> ScoreEntry {
    let performance_skill = ini.performance_skill();
    ScoreEntry {
        id: format!(
            "{}:{source:?}:{}:{}",
            identity.canonical_hash, ini.score, ini.date_time
        ),
        chart: identity.clone(),
        title: title.to_string(),
        artist: artist.to_string(),
        score: ini.score,
        chart_level,
        performance_skill,
        song_skill: if ini.song_skill != 0.0 {
            ini.song_skill
        } else {
            crate::skill::drum_song_skill(chart_level, performance_skill, false)
        },
        max_combo: ini.max_combo,
        judgments: JudgmentTotals {
            perfect: ini.perfect,
            great: ini.great,
            good: ini.good,
            poor: ini.poor,
            miss: ini.miss,
        },
        rank: rank_from_ini(&ini.rank),
        played_at: 0,
        source,
        replay_ref: None,
        no_fail: false,
    }
}

fn rank_from_ini(rank: &str) -> Rank {
    match rank {
        "SS" => Rank::SS,
        "S" => Rank::S,
        "A" => Rank::A,
        "B" => Rank::B,
        "C" => Rank::C,
        "D" => Rank::D,
        "E" => Rank::E,
        _ => Rank::Unknown,
    }
}
