//! DTX base-36 id encoding (BocuD `CConversion.nConvert2DigitBase36StringToNumber`).
//!
//! Reference: `references/DTXmaniaNX-BocuD/FDK/Common/CConversion.cs:113-133`

const BASE36_CHARS: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// Parse a 2-char base-36 DTX id (WAV/BMP/BGMWAV suffix, chip object pairs).
pub fn parse_2digit(s: &str, offset: usize) -> Option<u32> {
    let bytes = s.as_bytes();
    if offset + 2 > bytes.len() {
        return None;
    }
    let c0 = bytes[offset] as char;
    let c1 = bytes[offset + 1] as char;
    let mut digit2 = BASE36_CHARS.find(c0)? as u32;
    let mut digit1 = BASE36_CHARS.find(c1)? as u32;
    if digit2 >= 36 {
        digit2 -= 36 - 10;
    }
    if digit1 >= 36 {
        digit1 -= 36 - 10;
    }
    Some(digit2 * 36 + digit1)
}

/// Parse a WAV/BMP/BGMWAV suffix (`"0X"`, `"01"`, …).
pub fn parse_id_suffix(s: &str) -> Option<u32> {
    let s = s.trim();
    if s.len() == 2 {
        parse_2digit(s, 0)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_0x_slot() {
        assert_eq!(parse_id_suffix("0X"), Some(33));
    }

    #[test]
    fn parse_01_slot() {
        assert_eq!(parse_id_suffix("01"), Some(1));
    }

    #[test]
    fn parse_0a_slot() {
        assert_eq!(parse_id_suffix("0a"), Some(10));
    }
}
