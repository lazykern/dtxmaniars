//! `EnumConverter` (207 LOC) — bidirectional string ↔ int conversion.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/EnumConverter.cs:1-207`
//!
//! v1 strict-port: generic utility for parsing/writing enum values in
//! INI files. C# uses reflection; Rust uses trait + macro-free lookup.

/// Trait: types that can round-trip through string + int.
pub trait EnumConverter: Sized + Copy + 'static {
    /// All variants in declaration order.
    fn all() -> &'static [Self];
    /// Integer tag for this variant.
    fn as_int(&self) -> i32;
    /// Construct from int tag, or None if out of range.
    fn from_int(v: i32) -> Option<Self>;
    /// Canonical string name.
    fn as_str(&self) -> &'static str;
    /// Parse from string name, or None.
    fn from_str_enum(s: &str) -> Option<Self> {
        Self::all().iter().find(|v| v.as_str() == s).copied()
    }
    /// Convert int → string (returns "?" if unknown).
    fn int_to_str(v: i32) -> String {
        Self::from_int(v)
            .map(|e| e.as_str().to_string())
            .unwrap_or_else(|| format!("?{v}"))
    }
    /// Convert string → int (returns -1 if unknown).
    fn str_to_int(s: &str) -> i32 {
        Self::from_str_enum(s).map(|e| e.as_int()).unwrap_or(-1)
    }
    /// Implement FromStr using the canonical name.
    fn parse(s: &str) -> Result<Self, String> {
        Self::from_str_enum(s).ok_or_else(|| format!("unknown variant: {s:?}"))
    }
}

// (Removed FromStr impl - not dyn compatible)
pub fn _unused() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Color {
        Red = 0,
        Green = 1,
        Blue = 2,
    }

    impl EnumConverter for Color {
        fn all() -> &'static [Self] {
            &[Self::Red, Self::Green, Self::Blue]
        }
        fn as_int(&self) -> i32 {
            *self as i32
        }
        fn from_int(v: i32) -> Option<Self> {
            match v {
                0 => Some(Self::Red),
                1 => Some(Self::Green),
                2 => Some(Self::Blue),
                _ => None,
            }
        }
        fn as_str(&self) -> &'static str {
            match self {
                Self::Red => "Red",
                Self::Green => "Green",
                Self::Blue => "Blue",
            }
        }
    }

    #[test]
    fn round_trip_int() {
        for v in [Color::Red, Color::Green, Color::Blue] {
            assert_eq!(Color::from_int(v.as_int()), Some(v));
        }
        assert_eq!(Color::from_int(99), None);
    }

    #[test]
    fn round_trip_str() {
        assert_eq!(Color::from_str_enum("Red"), Some(Color::Red));
        assert_eq!(Color::from_str_enum("Blue"), Some(Color::Blue));
        assert_eq!(Color::from_str_enum("Yellow"), None);
    }

    #[test]
    fn int_to_str() {
        assert_eq!(Color::int_to_str(0), "Red");
        assert_eq!(Color::int_to_str(1), "Green");
        assert_eq!(Color::int_to_str(99), "?99");
    }

    #[test]
    fn str_to_int() {
        assert_eq!(Color::str_to_int("Red"), 0);
        assert_eq!(Color::str_to_int("Blue"), 2);
        assert_eq!(Color::str_to_int("Yellow"), -1);
    }

    #[test]
    fn parse_helper() {
        assert_eq!(Color::parse("Green").unwrap(), Color::Green);
        assert!(Color::parse("Yellow").is_err());
    }

    #[test]
    fn fromstr_helper() {
        let v: Result<Color, _> = EnumConverter::from_str_enum("Red").ok_or("?");
        assert!(v.is_ok());
    }
}
