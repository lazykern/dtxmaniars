//! `dtx` CLI: validate, inspect, future tools.
//!
//! v1 (M0): `dtx validate <file.dtx>` — parse and report.

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
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
