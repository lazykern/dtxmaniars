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

use crate::base36;
use crate::channel::EChannel;
use crate::chart::{Chart, Chip, EmptyHitEvent, Metadata};
use crate::chip_classify::{is_bad_note_byte, nosound_byte_to_lane};
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

    resolve_bpm_ex_chips(&mut chart);

    Ok(chart)
}

/// Resolve BPMEx (channel 0x08) chips: fill `value` with the BPM from the
/// `#BPMxx` definition referenced by `wav_slot`. The chip's `fraction` (its
/// in-measure position) is untouched. Chips whose slot has no definition are
/// dropped so they cannot corrupt the timeline with a bogus BPM.
fn resolve_bpm_ex_chips(chart: &mut Chart) {
    let bpm_defs = chart.assets.bpm.clone();
    chart.chips.retain_mut(|chip| {
        if chip.channel != EChannel::BPMEx {
            return true;
        }
        match bpm_defs.get(&chip.wav_slot) {
            Some(&bpm) if bpm > 0.0 => {
                chip.value = bpm;
                true
            }
            _ => false,
        }
    });
}

/// Decode DTX text bytes. A BOM, if present, picks the encoding outright —
/// some authoring tools export UTF-16, whose bytes are not valid UTF-8 and
/// would otherwise be mangled into garbage by the Shift-JIS fallback, yielding
/// a chart with zero chips. Without a BOM: UTF-8 first (ASCII-only and modern
/// exports), then Shift-JIS for legacy DTXManiaNX files.
///
/// The BOM is stripped rather than decoded. `str::lines` does not treat U+FEFF
/// as whitespace, so a surviving BOM would glue itself to the first directive
/// and silently drop it.
fn decode_dtx_text(bytes: &[u8]) -> String {
    match bytes {
        [0xFF, 0xFE, rest @ ..] => encoding_rs::UTF_16LE.decode(rest).0.into_owned(),
        [0xFE, 0xFF, rest @ ..] => encoding_rs::UTF_16BE.decode(rest).0.into_owned(),
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => match std::str::from_utf8(bytes) {
            Ok(s) => s.to_string(),
            Err(_) => encoding_rs::SHIFT_JIS.decode(bytes).0.into_owned(),
        },
    }
}

fn process_line(line: &str, line_no: usize, chart: &mut Chart) -> Result<()> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with("//") {
        return Ok(());
    }

    let Some(body) = trimmed.strip_prefix('#') else {
        return Ok(());
    };

    let Some((head, value)) = body.split_once(':') else {
        return Ok(());
    };
    let head = head.trim();
    let value = value.trim();

    if let Some(metadata_field) = parse_metadata_command(head, value) {
        apply_metadata(&mut chart.metadata, head, metadata_field, value, line_no)?;
        return Ok(());
    }

    if chart.assets.process_line(trimmed) {
        return Ok(());
    }

    if head.eq_ignore_ascii_case("BGMWAV") {
        let param = strip_dtx_param(value);
        if let Some(slot) = base36::parse_id_suffix(param) {
            chart.metadata.bgm_wav_slots.push(slot);
        }
        return Ok(());
    }

    parse_chip_line(
        head,
        value,
        line_no,
        &chart.assets.wav,
        &mut chart.chips,
        &mut chart.empty_hit_events,
    )?;
    Ok(())
}

/// Strip trailing DTX inline comment / tab-separated annotation.
fn strip_dtx_param(s: &str) -> &str {
    s.split([';', '\t']).next().unwrap_or(s).trim()
}

enum MetadataField {
    Title,
    Artist,
    Genre,
    Maker,
    Comment,
    Preview,
    Preimage,
    SoundNowLoading,
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
        "SOUND_NOWLOADING" => MetadataField::SoundNowLoading,
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
        MetadataField::SoundNowLoading => meta.sound_nowloading = Some(value.to_string()),
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
fn parse_chip_line(
    head: &str,
    value: &str,
    line_no: usize,
    wav: &crate::assets::WavRegistry,
    chips: &mut Vec<Chip>,
    empty_hits: &mut Vec<EmptyHitEvent>,
) -> Result<()> {
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
    let Ok(raw_measure) = measure_str.parse::<u32>() else {
        let _ = value;
        return Ok(());
    };
    // DTXManiaNX inserts one empty measure before the chart.
    // `#000xx` therefore lands at chart measure 1, not measure 0.
    let measure = raw_measure + 1;

    // Channel is hex (uppercase). If not, skip silently — many DTX commands
    // happen to be 5 chars but aren't chip lines.
    let Ok(channel_byte) = u8::from_str_radix(channel_str, 16) else {
        let _ = value;
        return Ok(());
    };

    // NoChip templates (0xB1–0xBE): empty-hit sound definitions.
    if is_bad_note_byte(channel_byte) {
        let Some(lane) = nosound_byte_to_lane(channel_byte) else {
            return Ok(());
        };
        push_empty_hit_events(measure, lane, value, wav, empty_hits);
        return Ok(());
    }

    let Some(channel) = EChannel::from_byte(channel_byte) else {
        // Unknown channel: skip silently. DTX files have many extension channels
        // we don't care about yet (e.g. SE06+, BGA layers).
        return Ok(());
    };

    // BarLength (0x02): value is a single decimal fraction of a whole note.
    if channel == EChannel::BarLength {
        if let Ok(v) = value.parse::<f32>() {
            chips.push(Chip::new(measure, channel, v));
        }
        return Ok(());
    }

    // BeatLineDisplay (0xC2): 1 = show lines, 2 = hide (BocuD CDTX.cs:3614-3624).
    if channel == EChannel::BeatLineDisplay {
        if let Ok(v) = value.parse::<f32>() {
            chips.push(Chip::new(measure, channel, v));
        }
        return Ok(());
    }

    // BPM (0x03) and BPMEx (0x08): sequences of 2-digit slots across the measure,
    // exactly like note channels. Each non-"00" slot is a BPM-change event.
    //   - 0x03: the 2 hex digits ARE the integer BPM (0x00..0xFF).
    //   - 0x08: the 2 base36 digits reference a `#BPMxx` definition; the real
    //     BPM is resolved from `chart.assets.bpm` in a post-parse pass.
    // Reference: `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/CDTX.cs`
    //   channel 0x03 → direct BPM, channel 0x08 → listBPM lookup.
    if matches!(channel, EChannel::BPM | EChannel::BPMEx) {
        let data = strip_dtx_param(value).replace(' ', "");
        if !data.len().is_multiple_of(2) || data.is_empty() {
            return Ok(());
        }
        let num_slots = data.len() / 2;
        for i in 0..num_slots {
            let pair = &data[i * 2..i * 2 + 2];
            if pair == "00" {
                continue;
            }
            let fraction = i as f32 / num_slots as f32;
            if channel == EChannel::BPM {
                // Direct hex BPM. Store the resolved value immediately.
                if let Ok(bpm) = u32::from_str_radix(pair, 16) {
                    if bpm > 0 {
                        chips.push(Chip::bpm_change(measure, channel, bpm as f32, fraction));
                    }
                }
            } else if let Some(slot) = base36::parse_2digit(&data, i * 2) {
                // BPMEx reference: keep the slot id in `wav_slot`; the value is
                // filled in by `resolve_bpm_ex_chips` once all `#BPMxx` defs are read.
                if slot > 0 {
                    chips.push(Chip::bpm_ex_ref(measure, slot, fraction));
                }
            }
        }
        return Ok(());
    }

    // BGA / Movie channels use the same two-character slot sequence as note
    // channels: each non-"00" slot references a `#BMPxx` / `#AVIxx` asset id
    // and carries a fractional measure position. Handled by the generic
    // chip-data parser below so visual timing is preserved
    // (`references/DTXmaniaNX-BocuD/DTXMania/Score,Song/CDTX.cs:1296-1476`).

    // Chip data: pairs of hex digits (standard) or one char per slot (legacy).
    let data = strip_dtx_param(value).replace(' ', "");
    if data.is_empty() {
        return Ok(());
    }

    if is_binary_only(&data) && !binary_pairs_reference_defined_wav(&data, wav) {
        push_single_char_chips(measure, channel, &data, chips);
        return Ok(());
    }

    if data.len().is_multiple_of(2) {
        let mut hex_chips = Vec::new();
        let num_slots = data.len() / 2;
        for i in 0..num_slots {
            let pair = &data[i * 2..i * 2 + 2];
            if pair == "00" {
                continue;
            }
            if let Some(wav_id) = base36::parse_2digit(&data, i * 2) {
                if wav_id == 0 {
                    continue;
                }
                let fraction = i as f32 / num_slots as f32;
                hex_chips.push(Chip::with_wav(measure, channel, fraction, wav_id));
            }
        }
        if !hex_chips.is_empty() {
            chips.extend(hex_chips);
            return Ok(());
        }
    }

    push_single_char_chips(measure, channel, &data, chips);
    Ok(())
}

fn push_empty_hit_events(
    measure: u32,
    lane: u8,
    value: &str,
    wav: &crate::assets::WavRegistry,
    empty_hits: &mut Vec<EmptyHitEvent>,
) {
    let data = strip_dtx_param(value).replace(' ', "");
    if data.is_empty() {
        return;
    }

    if data.len().is_multiple_of(2)
        && (!is_binary_only(&data) || binary_pairs_reference_defined_wav(&data, wav))
    {
        let num_slots = data.len() / 2;
        for i in 0..num_slots {
            let pair = &data[i * 2..i * 2 + 2];
            if pair == "00" {
                continue;
            }
            if let Some(wav_id) = base36::parse_2digit(&data, i * 2) {
                if wav_id == 0 {
                    continue;
                }
                let fraction = i as f32 / num_slots as f32;
                empty_hits.push(EmptyHitEvent {
                    lane,
                    measure,
                    value: fraction,
                    wav_slot: wav_id,
                });
            }
        }
        return;
    }

    let total = data.chars().count();
    if total == 0 {
        return;
    }
    for (i, ch) in data.chars().enumerate() {
        if ch == '0' {
            continue;
        }
        let fraction = i as f32 / total as f32;
        let wav_slot = char_to_wav_slot(ch);
        if wav_slot == 0 {
            continue;
        }
        empty_hits.push(EmptyHitEvent {
            lane,
            measure,
            value: fraction,
            wav_slot,
        });
    }
}

fn push_single_char_chips(measure: u32, channel: EChannel, data: &str, chips: &mut Vec<Chip>) {
    let total = data.chars().count();
    if total == 0 {
        return;
    }
    for (i, ch) in data.chars().enumerate() {
        if ch == '0' {
            continue;
        }
        let fraction = i as f32 / total as f32;
        let wav_slot = char_to_wav_slot(ch);
        chips.push(Chip::with_wav(measure, channel, fraction, wav_slot));
    }
}

fn char_to_wav_slot(ch: char) -> u32 {
    match ch {
        '1'..='9' => (ch as u32) - ('0' as u32),
        'A'..='F' | 'a'..='f' => u32::from_str_radix(&ch.to_string(), 16).unwrap_or(0),
        _ => 0,
    }
}

/// Returns true if the data string only contains '0' and '1' characters,
/// indicating a binary-format chip line rather than hex-pair format.
fn is_binary_only(data: &str) -> bool {
    data.chars().all(|c| c == '0' || c == '1')
}

fn binary_pairs_reference_defined_wav(data: &str, wav: &crate::assets::WavRegistry) -> bool {
    data.as_bytes().chunks_exact(2).any(|pair| {
        std::str::from_utf8(pair)
            .ok()
            .and_then(base36::parse_id_suffix)
            .is_some_and(|id| id != 0 && wav.get(id).is_some())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Real charts ship as UTF-16LE (the `546 - TOGENASHI TOGEARI` set does).
    /// Their bytes are not valid UTF-8, so before the BOM sniff they fell into
    /// the Shift-JIS fallback, decoded to mojibake, and produced *zero* chips —
    /// a chart that loaded silently empty.
    #[test]
    fn parses_utf16le_with_bom() {
        let src = "#TITLE: Test Song\n#WAV01: snare.ogg\n#00112: 0101\n";
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(src.encode_utf16().flat_map(u16::to_le_bytes));

        let chart = parse(Cursor::new(bytes)).unwrap();

        assert_eq!(chart.metadata.title.as_deref(), Some("Test Song"));
        assert_eq!(chart.chips.len(), 2);
    }

    #[test]
    fn parses_utf16be_with_bom() {
        let src = "#TITLE: Test Song\n#WAV01: snare.ogg\n#00112: 0101\n";
        let mut bytes = vec![0xFE, 0xFF];
        bytes.extend(src.encode_utf16().flat_map(u16::to_be_bytes));

        let chart = parse(Cursor::new(bytes)).unwrap();

        assert_eq!(chart.metadata.title.as_deref(), Some("Test Song"));
        assert_eq!(chart.chips.len(), 2);
    }

    /// A UTF-8 BOM decodes cleanly via `from_utf8`, so it used to survive into
    /// the text as U+FEFF. `str::trim` does not strip it, so `#TITLE` on line 1
    /// failed its `strip_prefix('#')` and was dropped without a word.
    #[test]
    fn utf8_bom_does_not_swallow_the_first_directive() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(b"#TITLE: Test Song\n#WAV01: snare.ogg\n");

        let chart = parse(Cursor::new(bytes)).unwrap();

        assert_eq!(chart.metadata.title.as_deref(), Some("Test Song"));
    }

    #[test]
    fn bomless_shift_jis_still_decodes() {
        // "#TITLE: 曲" in Shift-JIS — invalid UTF-8, must hit the fallback.
        let mut bytes = b"#TITLE: ".to_vec();
        bytes.extend_from_slice(&[0x8B, 0xC8]);
        bytes.push(b'\n');

        let chart = parse(Cursor::new(bytes)).unwrap();

        assert_eq!(chart.metadata.title.as_deref(), Some("曲"));
    }

    #[test]
    fn binary_looking_pairs_keep_base36_wav_id() {
        let chart = parse(Cursor::new(
            "#WAV01: snare.ogg\n#WAV11: ride.ogg\n#00119: 1100\n",
        ))
        .unwrap();

        assert_eq!(chart.chips.len(), 1);
        assert_eq!(chart.chips[0].channel, EChannel::RideCymbal);
        assert_eq!(chart.chips[0].wav_slot, 37);
    }

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
        assert_eq!(chart.chips[0].measure, 2);
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
    fn parses_nochip_empty_hit_events() {
        let input = "#000B1: 01\n#000B2: 0200\n";
        let chart = parse(Cursor::new(input)).unwrap();
        assert_eq!(chart.chips.len(), 0);
        assert_eq!(chart.empty_hit_events.len(), 2);
        assert_eq!(chart.empty_hit_events[0].lane, 0);
        assert_eq!(chart.empty_hit_events[0].wav_slot, 1);
        assert_eq!(chart.empty_hit_events[1].lane, 1);
        assert_eq!(chart.empty_hit_events[1].wav_slot, 2);
    }

    #[test]
    fn skips_wav_volume_pan_definitions() {
        let input = "\
#WAV01: kick.ogg
#VOLUME01: 80
#PAN01: -10
#BMP01: bg.bmp
#AVI01: movie.avi
#00011: 1
";
        let chart = parse(Cursor::new(input)).unwrap();
        assert_eq!(chart.chips.len(), 1);
        assert_eq!(chart.chips[0].channel, crate::channel::EChannel::HiHatClose);
        assert_eq!(chart.assets.wav.get(1), Some("kick.ogg"));
        assert_eq!(chart.assets.wav.volume(1), 80);
        assert_eq!(chart.assets.wav.pan(1), -10);
        assert_eq!(chart.assets.bmp.get(1), Some("bg.bmp"));
    }

    #[test]
    fn parses_hex_pair_chip_data() {
        // Hex pair format: "01000200" → WAV #01 at pos 0/4, WAV #02 at pos 2/4.
        let input = "#00011: 01000200\n";
        let chart = parse(Cursor::new(input)).unwrap();
        assert_eq!(chart.chips.len(), 2);
        assert_eq!(chart.chips[0].wav_slot, 0x01);
        assert!((chart.chips[0].value - 0.0).abs() < 0.01);
        assert_eq!(chart.chips[1].wav_slot, 0x02);
        assert!((chart.chips[1].value - 0.5).abs() < 0.01);
    }

    #[test]
    fn bpm_ex_chip_resolves_to_definition_not_slot() {
        // Regression: channel 0x08 references a `#BPMxx` definition. The chip's
        // value must become the DEFINED bpm (193), never the raw slot ref (5).
        let input = "\
#BPM: 193
#BPM05: 193
#00008: 05
#00113: 01
";
        let chart = parse(Cursor::new(input)).unwrap();
        let bpm_chips: Vec<_> = chart
            .chips
            .iter()
            .filter(|c| c.channel == EChannel::BPMEx)
            .collect();
        assert_eq!(bpm_chips.len(), 1);
        assert_eq!(bpm_chips[0].measure, 1);
        assert!(
            (bpm_chips[0].value - 193.0).abs() < 0.01,
            "BPMEx value should resolve to #BPM05 (193), got {}",
            bpm_chips[0].value
        );
    }

    #[test]
    fn bpm_ex_chips_keep_their_in_measure_position() {
        // Regression: a measure can hold several BPM changes at different
        // fractional positions. `#20208: 090B` (from the real Rock Lady chart)
        // means BPM09 at 0.0 and BPM0B at 0.5. Losing the position snapped both
        // to the bar line and mistimed everything after the slow section.
        let input = "\
#BPM: 181
#BPM09: 71
#BPM0B: 179
#20208: 090B
";
        let chart = parse(Cursor::new(input)).unwrap();
        let bpm_chips: Vec<_> = chart
            .chips
            .iter()
            .filter(|c| c.channel == EChannel::BPMEx)
            .collect();

        assert_eq!(bpm_chips.len(), 2);
        assert_eq!(bpm_chips[0].value, 71.0);
        assert_eq!(bpm_chips[0].fraction, 0.0);
        assert_eq!(bpm_chips[1].value, 179.0);
        assert_eq!(bpm_chips[1].fraction, 0.5);
    }

    #[test]
    fn bpm_direct_chips_keep_their_in_measure_position() {
        // Same for channel 0x03: 4 slots, changes at 0.0 and 0.5.
        // 0x78 = 120, 0xF0 = 240.
        let input = "#00103: 780000F0\n";
        let chart = parse(Cursor::new(input)).unwrap();
        let bpm_chips: Vec<_> = chart
            .chips
            .iter()
            .filter(|c| c.channel == EChannel::BPM)
            .collect();

        assert_eq!(bpm_chips.len(), 2);
        assert_eq!(bpm_chips[0].value, 120.0);
        assert_eq!(bpm_chips[0].fraction, 0.0);
        assert_eq!(bpm_chips[1].value, 240.0);
        assert_eq!(bpm_chips[1].fraction, 0.75);
    }

    #[test]
    fn bpm_ex_chip_without_definition_is_dropped() {
        // A BPMEx reference with no matching #BPMxx def must be discarded rather
        // than poisoning the timeline with a bogus BPM.
        let input = "#00008: 05\n";
        let chart = parse(Cursor::new(input)).unwrap();
        assert!(chart.chips.iter().all(|c| c.channel != EChannel::BPMEx));
    }

    #[test]
    fn bpm_direct_channel_uses_hex_value() {
        // Channel 0x03: the 2 hex digits ARE the integer BPM. 0xC0 = 192.
        let input = "#001 03: C0\n".replace(' ', "");
        let chart = parse(Cursor::new(&input)).unwrap();
        let bpm_chips: Vec<_> = chart
            .chips
            .iter()
            .filter(|c| c.channel == EChannel::BPM)
            .collect();
        assert_eq!(bpm_chips.len(), 1);
        assert!((bpm_chips[0].value - 192.0).abs() < 0.01);
    }

    #[test]
    fn parses_binary_chip_data() {
        // Binary format: only '0' and '1' chars, treated as single-char mode.
        let input = "#00011: 10110000\n";
        let chart = parse(Cursor::new(input)).unwrap();
        assert_eq!(chart.chips.len(), 3);
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
    fn parses_bgmwav_directive() {
        let input = "#BGMWAV: 0X\n#WAV0X: bgm_d.ogg\n";
        let chart = parse(Cursor::new(input)).unwrap();
        assert_eq!(chart.metadata.bgm_wav_slots, vec![33]);
        assert_eq!(chart.assets.wav.get(33), Some("bgm_d.ogg"));
    }

    #[test]
    fn single_char_extended_chip_data() {
        let input = "#00061: 0W0W0W0W\n";
        let chart = parse(Cursor::new(input)).unwrap();
        assert_eq!(chart.chips.len(), 4);
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
