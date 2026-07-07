//! Stable chart identity types and hash functions.

use std::path::{Path, PathBuf};

use dtx_core::{Chart, EChannel};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Stable identity for a parsed chart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChartIdentity {
    /// Primary chart key. New charts use `dtx1:<sha256>`.
    pub canonical_hash: String,
    /// Raw-file SHA-256 for compatibility with legacy stores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_sha256: Option<String>,
    /// Additional raw hashes seen for the same canonical chart.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub raw_sha256_aliases: Vec<String>,
    /// Optional provenance hint. Not used for identity lookup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path_hint: Option<PathBuf>,
}

impl ChartIdentity {
    /// Build identity for a parsed chart, with optional raw hash and path hint.
    pub fn new(
        canonical_hash: String,
        raw_sha256: Option<String>,
        source_path_hint: Option<PathBuf>,
    ) -> Self {
        Self {
            canonical_hash,
            raw_sha256,
            raw_sha256_aliases: Vec::new(),
            source_path_hint,
        }
    }

    /// Build a migrated identity when only the old raw hash is known.
    pub fn legacy_raw(raw_sha256: String) -> Self {
        Self {
            canonical_hash: format!("legacy-raw:{raw_sha256}"),
            raw_sha256: Some(raw_sha256),
            raw_sha256_aliases: Vec::new(),
            source_path_hint: None,
        }
    }

    /// Add a raw hash alias if it is distinct from the primary raw hash.
    pub fn add_raw_alias(&mut self, raw_sha256: String) {
        if self.raw_sha256.as_deref() == Some(raw_sha256.as_str()) {
            return;
        }
        if !self.raw_sha256_aliases.iter().any(|h| h == &raw_sha256) {
            self.raw_sha256_aliases.push(raw_sha256);
        }
    }

    /// True when any raw hash slot matches `raw`.
    pub fn matches_raw(&self, raw: &str) -> bool {
        self.raw_sha256.as_deref() == Some(raw)
            || self.raw_sha256_aliases.iter().any(|h| h == raw)
    }
}

/// Durable practice/result section key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SectionId {
    /// Canonical chart hash.
    pub canonical_chart_hash: String,
    /// Inclusive start bar.
    pub bar_start: u32,
    /// Exclusive end bar.
    pub bar_end: u32,
}

impl SectionId {
    /// Construct a section key.
    pub fn new(canonical_chart_hash: String, bar_start: u32, bar_end: u32) -> Self {
        Self {
            canonical_chart_hash,
            bar_start,
            bar_end,
        }
    }
}

/// Compute SHA-256 over raw file bytes.
pub fn raw_file_sha256(path: impl AsRef<Path>) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(hex_sha256(&bytes))
}

/// Compute the v1 canonical chart hash from parsed gameplay content.
pub fn canonical_chart_hash(chart: &Chart) -> String {
    let payload = canonical_payload(chart);
    format!("dtx1:{}", hex_sha256(payload.as_bytes()))
}

fn canonical_payload(chart: &Chart) -> String {
    let mut lines = Vec::new();
    lines.push("dtx-chart-id-v1".to_string());
    let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
    lines.push(format!("base_bpm={}", stable_f32(base_bpm)));

    let mut chips = chart.chips.clone();
    chips.sort_by(|a, b| {
        (
            a.measure,
            a.channel as u8,
            stable_f32(a.value),
            a.wav_slot,
        )
            .cmp(&(
                b.measure,
                b.channel as u8,
                stable_f32(b.value),
                b.wav_slot,
            ))
    });

    for chip in chips {
        if identity_channel(chip.channel) {
            lines.push(format!(
                "chip m={} c={:02X} p={} v={} wav={}",
                chip.measure,
                chip.channel as u8,
                stable_f32(chip.value),
                stable_f32(chip.value),
                chip.wav_slot
            ));
        }
    }

    lines.join("\n")
}

fn identity_channel(channel: EChannel) -> bool {
    channel.is_drum()
        || channel.is_guitar()
        || matches!(
            channel,
            EChannel::BGM | EChannel::BPM | EChannel::BPMEx | EChannel::BarLength
        )
}

fn stable_f32(value: f32) -> String {
    format!("{value:.6}")
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut hex, "{b:02x}");
    }
    hex
}
