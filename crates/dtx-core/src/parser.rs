//! DTX file parser.
//!
//! Format spec: lines starting with `#` are commands.
//! - `#TITLE: text`, `#ARTIST: text`, `#BPM: 120.0`, etc. for metadata.
//! - `#MMMCC: <data>` for chip lines, where MMM = 3-digit measure (000..),
//!   CC = 2-hex-digit channel. `data` is a string of `0`/`1` chars; each `1`
//!   produces a [`Chip`] at that fractional measure position.
//! - Lines starting with `//` are comments.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/CDTX.cs` (7295 LOC).

use std::io::{BufRead, BufReader, Read};

use crate::channel::EChannel;
use crate::chart::{Chart, Chip, Metadata};
use crate::error::{DtxError, Result};

/// Parse a DTX stream.
pub fn parse<R: Read>(reader: R) -> Result<Chart> {
    let buf = BufReader::new(reader);
    let mut chart = Chart::default();

    for (idx, line) in buf.lines().enumerate() {
        let line_no = idx + 1;
        let line = line?;
        process_line(&line, line_no, &mut chart)?;
    }

    Ok(chart)
}

fn process_line(line: &str, line_no: usize, chart: &mut Chart) -> Result<()> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with("//") {
        return Ok(());
    }

    let Some(body) = trimmed.strip_prefix('#') else {
        // Non-command line — silently ignore (some DTX files have stray text).
        return Ok(());
    };

    let Some((head, value)) = body.split_once(':') else {
        // `#START` / `#END` markers have no value.
        return Ok(());
    };
    let head = head.trim();
    let value = value.trim();

    if let Some(metadata_field) = parse_metadata_command(head, value) {
        apply_metadata(&mut chart.metadata, head, metadata_field, value, line_no)?;
        return Ok(());
    }

    parse_chip_line(head, value, line_no, &mut chart.chips)?;
    Ok(())
}

enum MetadataField {
    Title,
    Artist,
    Genre,
    Maker,
    Comment,
    Preview,
    Preimage,
    Bpm,
    DLevel,
    GLevel,
    BLevel,
}

fn parse_metadata_command(head: &str, _value: &str) -> Option<MetadataField> {
    Some(match head.to_ascii_uppercase().as_str() {
        "TITLE" => MetadataField::Title,
        "ARTIST" => MetadataField::Artist,
        "GENRE" => MetadataField::Genre,
        "MAKER" => MetadataField::Maker,
        "COMMENT" => MetadataField::Comment,
        "PREVIEW" => MetadataField::Preview,
        "PREIMAGE" => MetadataField::Preimage,
        "BPM" => MetadataField::Bpm,
        "DLEVEL" => MetadataField::DLevel,
        "GLEVEL" => MetadataField::GLevel,
        "BLEVEL" => MetadataField::BLevel,
        _ => return None,
    })
}

fn apply_metadata(
    meta: &mut Metadata,
    head: &str,
    field: MetadataField,
    value: &str,
    line_no: usize,
) -> Result<()> {
    match field {
        MetadataField::Title => meta.title = Some(value.to_string()),
        MetadataField::Artist => meta.artist = Some(value.to_string()),
        MetadataField::Genre => meta.genre = Some(value.to_string()),
        MetadataField::Maker => meta.maker = Some(value.to_string()),
        MetadataField::Comment => meta.comment = Some(value.to_string()),
        MetadataField::Preview => meta.preview_filename = Some(value.to_string()),
        MetadataField::Preimage => meta.preimage_filename = Some(value.to_string()),
        MetadataField::Bpm => {
            meta.bpm = Some(value.parse().map_err(|_| DtxError::InvalidLine {
                line: line_no,
                message: format!("#BPM not a float: {value:?}"),
            })?);
        }
        MetadataField::DLevel => {
            meta.dlevel = Some(value.parse().map_err(|_| DtxError::InvalidLine {
                line: line_no,
                message: format!("#DLEVEL not an int: {value:?}"),
            })?);
        }
        MetadataField::GLevel => {
            meta.glevel = Some(value.parse().map_err(|_| DtxError::InvalidLine {
                line: line_no,
                message: format!("#GLEVEL not an int: {value:?}"),
            })?);
        }
        MetadataField::BLevel => {
            meta.blevel = Some(value.parse().map_err(|_| DtxError::InvalidLine {
                line: line_no,
                message: format!("#BLEVEL not an int: {value:?}"),
            })?);
        }
    }
    // head is unused for non-error paths, but kept for future header variants.
    let _ = head;
    Ok(())
}

/// Parse a chip line: head is "MMMCC" (5 chars), value is binary data.
fn parse_chip_line(head: &str, value: &str, line_no: usize, chips: &mut Vec<Chip>) -> Result<()> {
    if head.len() != 5 {
        return Err(DtxError::InvalidLine {
            line: line_no,
            message: format!("expected 5-char measure+channel head, got {head:?}"),
        });
    }

    let measure_str = &head[0..3];
    let channel_str = &head[3..5];

    let measure: u32 = measure_str.parse().map_err(|_| DtxError::InvalidMeasure {
        line: line_no,
        value: measure_str.parse().unwrap_or(0),
    })?;

    // Channel is hex (uppercase) — both 0x11 (decimal 17) and 17 must parse.
    let channel_byte =
        u8::from_str_radix(channel_str, 16).map_err(|_| DtxError::InvalidChannel {
            line: line_no,
            value: 0,
        })?;
    let Some(channel) = EChannel::from_byte(channel_byte) else {
        // Unknown channel: skip silently. DTX files have many extension channels
        // we don't care about yet (e.g. SE06+, BGA layers).
        return Ok(());
    };

    // For BGM channel, value is a WAV filename (no per-chip encoding yet).
    if channel == EChannel::BGM {
        // Future: store BGM chip with filename as value. For M0, just emit a
        // marker chip so callers know the file references a BGM.
        chips.push(Chip::new(measure, channel, 0.0));
        return Ok(());
    }

    // For BarLength / BPM / BPMEx: value is a number (BarLength) or base64 (BPM).
    // We don't decode base64 in v1; store the raw value as f32 (best-effort).
    if matches!(
        channel,
        EChannel::BarLength | EChannel::BPM | EChannel::BPMEx
    ) {
        if let Ok(v) = value.parse::<f32>() {
            chips.push(Chip::new(measure, channel, v));
        }
        return Ok(());
    }

    // For BGA / Movie channels: value is a decimal BMP/AVI index. Store as f32.
    if matches!(
        channel,
        EChannel::BGALayer1
            | EChannel::BGALayer2
            | EChannel::BGALayer3
            | EChannel::BGALayer4
            | EChannel::BGALayer5
            | EChannel::BGALayer6
            | EChannel::BGALayer7
            | EChannel::BGALayer8
            | EChannel::Movie
            | EChannel::MovieFull
    ) {
        if let Ok(v) = value.parse::<f32>() {
            chips.push(Chip::new(measure, channel, v));
        }
        return Ok(());
    }

    // Chip data: '0'/'1' string. Each '1' = one chip at fractional position.
    let total = value.len();
    if total == 0 {
        return Ok(());
    }
    for (i, ch) in value.chars().enumerate() {
        if ch == '1' {
            let fraction = i as f32 / total as f32;
            chips.push(Chip::new(measure, channel, fraction));
        } else if ch != '0' {
            return Err(DtxError::InvalidLine {
                line: line_no,
                message: format!("chip data contains non-binary char {ch:?}"),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn parses_minimal_dtx() {
        let input = "\
#TITLE: Test Song
#ARTIST: Tester
#BPM: 120.00
#DLEVEL: 50
#00111: 10000000
";
        let chart = parse(Cursor::new(input)).unwrap();
        assert_eq!(chart.metadata.title.as_deref(), Some("Test Song"));
        assert_eq!(chart.metadata.artist.as_deref(), Some("Tester"));
        assert_eq!(chart.metadata.bpm, Some(120.0));
        assert_eq!(chart.metadata.dlevel, Some(50));
        assert_eq!(chart.chips.len(), 1);
        assert_eq!(chart.chips[0].channel, EChannel::HiHatClose);
        assert_eq!(chart.chips[0].measure, 1);
        assert!((chart.chips[0].value - 0.0).abs() < 0.01);
    }

    #[test]
    fn known_channel_emits_chips() {
        let input = "#20061: 1011\n"; // SE01 channel, known — emits 3 chips (3 '1's)
        let chart = parse(Cursor::new(input)).unwrap();
        assert_eq!(chart.chips.len(), 3);
    }

    #[test]
    fn unknown_channel_silently_skipped() {
        // 0xAB is not in our EChannel table — should be ignored, not error.
        let input = "#200AB: 1111\n";
        let chart = parse(Cursor::new(input)).unwrap();
        assert_eq!(chart.chips.len(), 0);
    }

    #[test]
    fn invalid_channel_head() {
        let input = "#abc11: 10\n";
        let err = parse(Cursor::new(input)).unwrap_err();
        assert!(matches!(err, DtxError::InvalidMeasure { .. }));
    }

    #[test]
    fn empty_and_comment_lines_ok() {
        let input = "\n// hello\n#TITLE: OK\n\n";
        let chart = parse(Cursor::new(input)).unwrap();
        assert_eq!(chart.metadata.title.as_deref(), Some("OK"));
        assert_eq!(chart.chips.len(), 0);
    }
}
