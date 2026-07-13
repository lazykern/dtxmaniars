//! Source chart formats accepted by the drums parser front ends.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ChartFormat {
    #[default]
    Dtx,
    Gda,
    G2d,
}

impl ChartFormat {
    pub fn from_extension(extension: &str) -> Option<Self> {
        if extension.eq_ignore_ascii_case("dtx") {
            Some(Self::Dtx)
        } else if extension.eq_ignore_ascii_case("gda") {
            Some(Self::Gda)
        } else if extension.eq_ignore_ascii_case("g2d") {
            Some(Self::G2d)
        } else {
            None
        }
    }
}

/// NX level representation: `tenths = 35`, `hundredths = 7` displays 3.57.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ChartLevel {
    pub tenths: u16,
    pub hundredths: u8,
}

impl ChartLevel {
    /// Normalize a DLEVEL/PLAYLEVEL value exactly like NX `ProcessLevel`.
    pub fn from_raw(raw: u32) -> Self {
        let raw = raw.clamp(0, 1000) as u16;
        if raw >= 100 {
            Self {
                tenths: raw / 10,
                hundredths: (raw % 10) as u8,
            }
        } else {
            Self {
                tenths: raw,
                hundredths: 0,
            }
        }
    }

    pub fn with_decimal(mut self, value: i32) -> Self {
        self.hundredths = value.clamp(0, 10) as u8;
        self
    }

    pub fn display(self) -> f32 {
        self.tenths as f32 / 10.0 + self.hundredths as f32 / 100.0
    }

    /// Legacy packed representation retained while downstream public structs
    /// still expose `dlevel: u32`.
    pub fn compatibility_raw(self) -> u32 {
        if self.tenths >= 100 || self.hundredths > 0 {
            u32::from(self.tenths) * 10 + u32::from(self.hundredths)
        } else {
            u32::from(self.tenths)
        }
    }
}
