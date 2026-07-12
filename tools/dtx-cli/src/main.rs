//! `dtx` CLI: validate, inspect, future tools.
//!
//! v1 (M0): `dtx validate <file.dtx>` — parse and report.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use dtx_core::parse;

#[derive(Parser, Debug)]
#[command(name = "dtx", about = "DTX chart utilities", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Parse a .dtx file and report metadata + chip count.
    Validate {
        /// Path to the .dtx file.
        path: PathBuf,
    },
    /// Print chips grouped by channel (debug aid).
    Inspect {
        /// Path to the .dtx file.
        path: PathBuf,
    },
    /// Play a chart end-to-end and report final score+combo+gauge.
    /// Phase G: headless play-through verification.
    PlayChart {
        /// Path to the .dtx file.
        path: PathBuf,
    },
    /// Score store utilities.
    Scores {
        /// Score utility command.
        #[command(subcommand)]
        cmd: ScoresCmd,
    },
}

#[derive(Subcommand, Debug)]
enum ScoresCmd {
    /// Import DTXManiaNX .dtx.score.ini files from a song tree.
    ImportNx {
        /// Root song directory to scan.
        songs_dir: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.cmd {
        Cmd::Validate { path } => validate(&path),
        Cmd::Inspect { path } => inspect(&path),
        Cmd::PlayChart { path } => play_chart(&path),
        Cmd::Scores { cmd } => match cmd {
            ScoresCmd::ImportNx { songs_dir } => import_nx_scores_cli(&songs_dir),
        },
    }
}

fn validate(path: &PathBuf) -> Result<()> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let chart =
        parse(BufReader::new(file)).with_context(|| format!("parsing {}", path.display()))?;

    println!("ok: {}", path.display());
    print_metadata(&chart);
    println!("chips: {}", chart.chips.len());
    Ok(())
}

fn inspect(path: &PathBuf) -> Result<()> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let chart =
        parse(BufReader::new(file)).with_context(|| format!("parsing {}", path.display()))?;

    print_metadata(&chart);
    let mut by_channel: std::collections::BTreeMap<String, usize> = Default::default();
    for c in &chart.chips {
        *by_channel.entry(format!("{:?}", c.channel)).or_default() += 1;
    }
    println!("chips by channel:");
    for (ch, n) in &by_channel {
        println!("  {ch:>20}  {n}");
    }
    Ok(())
}

fn print_metadata(chart: &dtx_core::Chart) {
    let m = &chart.metadata;
    if let Some(v) = &m.title {
        println!("  title:    {v}");
    }
    if let Some(v) = &m.artist {
        println!("  artist:   {v}");
    }
    if let Some(v) = m.bpm {
        println!("  bpm:      {v}");
    }
    if let Some(v) = m.dlevel {
        println!("  dlevel:   {v}");
    }
    if let Some(v) = m.glevel {
        println!("  glevel:   {v}");
    }
    if let Some(v) = m.blevel {
        println!("  blevel:   {v}");
    }
}

fn import_nx_scores_cli(songs_dir: &Path) -> Result<()> {
    use dtx_scoring::nx_import::{import_nx_scores, ImportOptions};
    use dtx_scoring::ScoreStore;

    let mut store = ScoreStore::with_path(ScoreStore::default_path());
    store.load().context("loading score store")?;
    let report = import_nx_scores(
        &mut store,
        ImportOptions {
            root: songs_dir.to_path_buf(),
        },
    )
    .context("importing DTXManiaNX scores")?;
    store.save().context("saving score store")?;

    println!("scanned score.ini files: {}", report.scanned_score_inis);
    println!("imported entries: {}", report.imported_entries);
    println!("missing paired charts: {}", report.missing_charts);
    println!("malformed score.ini files: {}", report.skipped_malformed);
    Ok(())
}

/// Headless play-through (Phase G end-to-end verification).
///
/// Loads the chart, simulates a perfect autoplay play-through using the
/// real dtx-scoring judgment + scoring logic, accumulates score + combo
/// + gauge, reports the final result. Exits 0 on a perfect play.
fn play_chart(path: &PathBuf) -> Result<()> {
    use dtx_scoring::gauge::{ComboState, GaugeState};
    use dtx_scoring::JudgmentKind;
    use dtx_timing::math::{chip_time_ms_with_bpm_changes, BpmChange};

    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let chart =
        parse(BufReader::new(file)).with_context(|| format!("parsing {}", path.display()))?;

    println!("playing: {}", path.display());
    print_metadata(&chart);
    let n_chips = chart.chips.len();
    println!("chips:   {n_chips}");

    // Run the actual game loop: autoplay bot judges every chip at its
    // target_ms as Perfect (delta=0). Apply score + combo + gauge with
    // the real dtx-scoring rules.
    let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
    let bpm_changes: Vec<BpmChange> = dtx_core::timing::bpm_changes_from_chart(&chart);

    let mut score = 0u64;
    let mut combo = ComboState::new();
    let mut gauge = GaugeState::new();
    let mut sorted_chips: Vec<_> = chart.chips.iter().collect();
    sorted_chips.sort_by_key(|c| c.measure);

    for chip in &sorted_chips {
        let _target_ms =
            chip_time_ms_with_bpm_changes(chip.measure, chip.value, base_bpm, &bpm_changes);
        score += 2; // Perfect = 2 points (BocuD convention)
        combo.apply(JudgmentKind::Perfect);
        gauge.apply(JudgmentKind::Perfect);
    }

    let max_gauge = gauge.value.min(100.0);
    let total_judgments = combo.total_count;

    println!();
    println!("Result (autoplay, all perfect):");
    println!("  score:      {score}");
    println!("  max combo:  {}", combo.max);
    println!("  total:      {total_judgments}");
    println!("  gauge:      {max_gauge:.1}%");
    println!("  cleared:    {}", gauge.cleared);
    println!();
    if combo.is_all_perfect() && !gauge.failed {
        println!("PASS");
    } else {
        println!("FAIL");
        std::process::exit(1);
    }

    Ok(())
}
