//! Structured compatibility diagnostics retained through scan and loading.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticKind {
    Conditional,
    UnknownOptional,
    UnsupportedChannel,
    MalformedVisual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartDiagnostic {
    pub line: Option<usize>,
    pub kind: DiagnosticKind,
    pub detail: String,
    pub recovery: Option<String>,
}

impl ChartDiagnostic {
    pub fn conditional(warning: &crate::conditional::ParseWarning) -> Self {
        Self {
            line: Some(warning.line),
            kind: DiagnosticKind::Conditional,
            detail: format!("{:?}", warning.kind),
            recovery: Some("Check #RANDOM/#IF/#ENDIF structure.".into()),
        }
    }
}
