#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseWarning {
    pub line: usize,
    pub kind: ParseWarningKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseWarningKind {
    InvalidRandom,
    InvalidIf,
    UnmatchedEndIf,
    UnclosedIf { depth: usize },
    ConditionalDepthExceeded,
}

struct SeededSelector(u64);

impl SeededSelector {
    fn choose(&mut self, max: u32) -> u32 {
        let selected = (self.0 % u64::from(max.max(1))) as u32 + 1;
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        selected
    }
}

enum Directive<'a> {
    Random(&'a str),
    If(&'a str),
    EndIf,
}

fn named_argument<'a>(body: &'a str, name: &str) -> Option<&'a str> {
    let prefix = body.get(..name.len())?;
    if !prefix.eq_ignore_ascii_case(name) {
        return None;
    }
    let remainder = body[name.len()..].trim_start();
    if remainder.is_empty() {
        return Some("");
    }
    if let Some(argument) = remainder.strip_prefix(':') {
        return Some(argument.trim());
    }
    let first = remainder.as_bytes()[0];
    if first.is_ascii_digit() || first == b'+' || first == b'-' {
        return Some(remainder);
    }
    if body
        .as_bytes()
        .get(name.len())
        .is_some_and(u8::is_ascii_whitespace)
    {
        return Some(remainder);
    }
    None
}

fn directive(line: &str) -> Option<Directive<'_>> {
    let body = line.trim().strip_prefix('#')?.trim_start();
    if body.eq_ignore_ascii_case("ENDIF") {
        return Some(Directive::EndIf);
    }
    if let Some(argument) = named_argument(body, "RANDOM") {
        return Some(Directive::Random(argument));
    }
    named_argument(body, "IF").map(Directive::If)
}

fn parse_positive(argument: &str) -> Option<u32> {
    argument
        .split([';', '\t', ' '])
        .next()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|value| *value > 0)
}

pub(crate) fn select_active_lines(
    text: &str,
    seed: u64,
) -> (Vec<(usize, &str)>, Vec<ParseWarning>) {
    const MAX_DEPTH: usize = 255;

    let mut selector = SeededSelector(seed);
    let mut selected = 1;
    let mut active_stack = Vec::new();
    let mut overflow_depth = 0usize;
    let mut lines = Vec::new();
    let mut warnings = Vec::new();

    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let parent_active = overflow_depth == 0 && active_stack.last().copied().unwrap_or(true);
        match directive(line) {
            Some(Directive::Random(argument)) => {
                if parent_active {
                    let max = match parse_positive(argument) {
                        Some(value) => value,
                        None => {
                            warnings.push(ParseWarning {
                                line: line_number,
                                kind: ParseWarningKind::InvalidRandom,
                            });
                            1
                        }
                    };
                    selected = selector.choose(max);
                }
            }
            Some(Directive::If(argument)) => {
                let expected = match parse_positive(argument) {
                    Some(value) => value,
                    None => {
                        warnings.push(ParseWarning {
                            line: line_number,
                            kind: ParseWarningKind::InvalidIf,
                        });
                        1
                    }
                };
                if overflow_depth > 0 || active_stack.len() >= MAX_DEPTH {
                    overflow_depth += 1;
                    warnings.push(ParseWarning {
                        line: line_number,
                        kind: ParseWarningKind::ConditionalDepthExceeded,
                    });
                } else {
                    active_stack.push(parent_active && selected == expected);
                }
            }
            Some(Directive::EndIf) => {
                if overflow_depth > 0 {
                    overflow_depth -= 1;
                } else if active_stack.pop().is_none() {
                    warnings.push(ParseWarning {
                        line: line_number,
                        kind: ParseWarningKind::UnmatchedEndIf,
                    });
                }
            }
            None if parent_active => lines.push((line_number, line)),
            None => {}
        }
    }

    let unclosed_depth = active_stack.len() + overflow_depth;
    if unclosed_depth > 0 {
        warnings.push(ParseWarning {
            line: text.lines().count().max(1),
            kind: ParseWarningKind::UnclosedIf {
                depth: unclosed_depth,
            },
        });
    }

    (lines, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_seeds_select_opposite_branches() {
        let src = "#RANDOM 2\n#IF 1\n#TITLE: One\n#ENDIF\n#IF 2\n#TITLE: Two\n#ENDIF\n";
        let (one, warnings) = select_active_lines(src, 0);
        assert!(warnings.is_empty());
        assert!(one.iter().any(|(_, line)| *line == "#TITLE: One"));
        assert!(!one.iter().any(|(_, line)| *line == "#TITLE: Two"));

        let (two, warnings) = select_active_lines(src, 1);
        assert!(warnings.is_empty());
        assert!(two.iter().any(|(_, line)| *line == "#TITLE: Two"));
        assert!(!two.iter().any(|(_, line)| *line == "#TITLE: One"));
    }

    #[test]
    fn inactive_parent_forces_nested_branch_inactive() {
        let src = "#RANDOM: 2\n#IF 2\n#IF 1\n#TITLE: Hidden\n#ENDIF\n#ENDIF\n#TITLE: Visible\n";
        let (lines, warnings) = select_active_lines(src, 0);
        assert!(warnings.is_empty());
        assert!(!lines.iter().any(|(_, line)| line.contains("Hidden")));
        assert!(lines.iter().any(|(_, line)| line.contains("Visible")));
    }

    #[test]
    fn malformed_structure_warns_without_panicking() {
        let src = "#ENDIF\n#RANDOM nope\n#IF nope\n#TITLE: Recovered\n";
        let (_, warnings) = select_active_lines(src, 0);
        assert!(warnings
            .iter()
            .any(|w| w.kind == ParseWarningKind::UnmatchedEndIf));
        assert!(warnings
            .iter()
            .any(|w| w.kind == ParseWarningKind::InvalidRandom));
        assert!(warnings
            .iter()
            .any(|w| w.kind == ParseWarningKind::InvalidIf));
        assert!(warnings
            .iter()
            .any(|w| matches!(w.kind, ParseWarningKind::UnclosedIf { .. })));
    }
}
