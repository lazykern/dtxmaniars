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

use std::io::Read;

use crate::channel::EChannel;
use crate::chart::{Chart, Chip, Metadata};
use crate::error::{DtxError, Result};

/// Parse a DTX stream.
///
/// DTX text is encoded in Shift-JIS (Japanese Windows standard, used by
/// DTXManiaNX). Some tooling exports UTF-8; we try UTF-8 first and fall
/// back to Shift-JIS if that fails. Binary data (chip lines, #MMMCC) is
/// ASCII-only so encoding doesn't matter for them.
pub fn parse<R: Read>(mut reader: R) -> Result<Chart> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    let text = decode_dtx_text(&bytes);
    let mut chart = Chart::default();

    for (idx, line) in text.lines().enumerate() {
        let line_no = idx + 1;
        process_line(line, line_no, &mut chart)?;
    }

    Ok(chart)
}

/// Decode DTX text bytes. Tries UTF-8 first (covers ASCII-only and modern
/// exports), falls back to Shift-JIS for legacy DTXManiaNX files.
fn decode_dtx_text(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let (cow, _, _had_errors) = encoding_rs::SHIFT_JIS.decode(bytes);
            cow.into_owned()
        }
    }
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
///
/// DTX files contain many other commands (WAV, VOLUME, PAN, BMP, AVI defs)
/// that share the `#XXXX:` prefix shape. We silently skip those rather
/// than erroring out — they're definitions we don't model yet.
fn parse_chip_line(head: &str, value: &str, line_no: usize, chips: &mut Vec<Chip>) -> Result<()> {
    if head.len() != 5 {
        // Other commands like `#WAV01:`, `#VOLUME02:`, `#PAN03:` are not chip lines.
        // Silently skip.
        let _ = (line_no, value);
        return Ok(());
    }

    let measure_str = &head[0..3];
    let channel_str = &head[3..5];

    // Measure must be a decimal number. If not, this isn't a chip line
    // (e.g. `#WAV01` → "WAV" is not a number).
    let Ok(measure) = measure_str.parse::<u32>() else {
        let _ = value;
        return Ok(());
    };

    // Channel is hex (uppercase). If not, skip silently — many DTX commands
    // happen to be 5 chars but aren't chip lines.
    let Ok(channel_byte) = u8::from_str_radix(channel_str, 16) else {
        let _ = value;
        return Ok(());
    };
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

    // Chip data: each char represents a position in the measure.
    //   '0' = nothing
    //   '1' = chip (hit window)
    //   '2' = chip + strong (doublescore)
    //   'W' = chip + weak (half score, autoplay indicator)
    //   'X' = chip + extra (counts as bad note if hit)
    //   '5' (and other non-zero) = chip variant (DTX extensions)
    //
    // Real-world DTX files use many non-standard chars (e.g. '5' for strong,
    // 'A'-'F' for variants). We treat any non-'0' char as a chip; variant
    // info isn't preserved yet.
    let total = value.chars().count();
    if total == 0 {
        return Ok(());
    }
    for (i, ch) in value.chars().enumerate() {
        if ch != '0' {
            let fraction = i as f32 / total as f32;
            chips.push(Chip::new(measure, channel, fraction));
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
        // #abc11 — first 3 chars aren't digits, so it's not a chip line.
        // Silently skipped (DTX has many non-chip commands that share the
        // `#XXXX:` shape, e.g. #WAV01, #VOLUME02).
        let input = "#abc11: 10\n";
        let chart = parse(Cursor::new(input)).expect("non-chip lines are skipped, not errors");
        assert!(chart.chips.is_empty());
    }

    #[test]
    fn empty_and_comment_lines_ok() {
        let input = "\n// hello\n#TITLE: OK\n\n";
        let chart = parse(Cursor::new(input)).unwrap();
        assert_eq!(chart.metadata.title.as_deref(), Some("OK"));
        assert_eq!(chart.chips.len(), 0);
    }

    #[test]
    fn skips_wav_volume_pan_definitions() {
        // These look like chip lines but aren't — they're sound/image defs.
        let input = "\
#WAV01: kick.ogg
#VOLUME01: 80
#PAN01: -10
#BMP01: bg.bmp
#AVI01: movie.avi
#00011: 1
";
        let chart = parse(Cursor::new(input)).unwrap();
        // Only the #00011 chip should be parsed; the rest are defs.
        assert_eq!(chart.chips.len(), 1);
        assert_eq!(chart.chips[0].channel, crate::channel::EChannel::HiHatClose);
    }

    #[test]
    fn parses_chip_data_with_non_binary_chars() {
        // '1' normal, '2' strong, '5' strong (DTX ext), 'W' weak, 'X' extra,
        // 'A' variant. All non-zero chars produce a chip; only '0' is empty.
        let input = "#00011: 125WXA\n";
        let chart = parse(Cursor::new(input)).unwrap();
        // 6 non-zero chars → 6 chips, one per position.
        assert_eq!(chart.chips.len(), 6);
        for chip in &chart.chips {
            assert_eq!(chip.channel, crate::channel::EChannel::HiHatClose);
        }
    }

    #[test]
    fn zero_chars_skip_in_chip_data() {
        // '0' chars produce no chip; '1' chars produce one. Pattern has 2 '1's.
        let input = "#00011: 0100010\n";
        let chart = parse(Cursor::new(input)).unwrap();
        assert_eq!(chart.chips.len(), 2);
    }

    #[test]
    fn decodes_shift_jis_metadata() {
        // Real DTXManiaNX exports use Shift-JIS for Japanese text. A 0x83 0x41
        // is a 2-byte Shift-JIS sequence ("\u{30A2}"); in UTF-8 it'd be 0xE3
        // 0x82 0xA2. We pick a sequence that's invalid as UTF-8 so the parser
        // is forced to use the Shift-JIS fallback.
        let shift_jis_bytes: &[u8] = b"#TITLE: \x83\x41\x83\x42\n";
        let chart = parse(Cursor::new(shift_jis_bytes)).expect("Shift-JIS must decode");
        assert!(chart.metadata.title.is_some());
        // Should contain non-ASCII chars (Shift-JIS decoded).
        let title = chart.metadata.title.as_deref().unwrap();
        assert!(!title.is_ascii());
    }
}
