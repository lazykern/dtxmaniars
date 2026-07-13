//! GDA/G2D channel-name normalization.
//!
//! The mapping is the drums-relevant subset of NX's conversion table at
//! `references/DTXmaniaNX/DTXMania/Score,Song/CDTX.cs:1198-1223`.

use crate::EChannel;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyChannelError {
    Unsupported(String),
}

const SE_CHANNELS: [EChannel; 32] = [
    EChannel::SE01,
    EChannel::SE02,
    EChannel::SE03,
    EChannel::SE04,
    EChannel::SE05,
    EChannel::SE06,
    EChannel::SE07,
    EChannel::SE08,
    EChannel::SE09,
    EChannel::SE10,
    EChannel::SE11,
    EChannel::SE12,
    EChannel::SE13,
    EChannel::SE14,
    EChannel::SE15,
    EChannel::SE16,
    EChannel::SE17,
    EChannel::SE18,
    EChannel::SE19,
    EChannel::SE20,
    EChannel::SE21,
    EChannel::SE22,
    EChannel::SE23,
    EChannel::SE24,
    EChannel::SE25,
    EChannel::SE26,
    EChannel::SE27,
    EChannel::SE28,
    EChannel::SE29,
    EChannel::SE30,
    EChannel::SE31,
    EChannel::SE32,
];

fn legacy_se_channel(head: &str) -> Option<EChannel> {
    let index = u8::from_str_radix(head, 16).ok()?;
    (1..=32)
        .contains(&index)
        .then(|| SE_CHANNELS[usize::from(index - 1)])
}

pub fn normalize_gda_head(head: &str) -> Result<Option<EChannel>, LegacyChannelError> {
    let upper = head.to_ascii_uppercase();
    if let Some(channel) = legacy_se_channel(&upper) {
        return Ok(Some(channel));
    }
    Ok(Some(match upper.as_str() {
        "TC" => EChannel::BPM,
        "BL" => EChannel::BarLength,
        "HH" => EChannel::HiHatClose,
        "SD" => EChannel::Snare,
        "BD" => EChannel::BassDrum,
        "HT" => EChannel::HighTom,
        "LT" => EChannel::LowTom,
        "CY" => EChannel::Cymbal,
        // Flow-speed and guitar/bass commands do not produce drums gameplay.
        "GS" | "DS" | "FI" | "G0" | "G1" | "G2" | "G3" | "G4" | "G5" | "G6" | "G7" | "GW"
        | "B0" | "B1" | "B2" | "B3" | "B4" | "B5" | "B6" | "B7" | "BW" => return Ok(None),
        other => return Err(LegacyChannelError::Unsupported(other.to_owned())),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_drums_and_all_legacy_se_names() {
        assert_eq!(normalize_gda_head("hh"), Ok(Some(EChannel::HiHatClose)));
        assert_eq!(normalize_gda_head("01"), Ok(Some(EChannel::SE01)));
        assert_eq!(normalize_gda_head("20"), Ok(Some(EChannel::SE32)));
    }
}
