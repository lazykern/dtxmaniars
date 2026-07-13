use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use regex::Regex;
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckFailure {
    pub file: PathBuf,
    pub line: usize,
    pub target: String,
    pub reason: FailureReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureReason {
    MissingLocalTarget,
    ObsoleteReferenceRoot,
    ReferenceEscapesRoot,
    MissingCanonicalDocument,
    CanonicalDocumentNotLinked,
    FalseMissingCanonicalClaim,
    InvalidReferenceLineRange,
}

impl fmt::Display for FailureReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::MissingLocalTarget => "missing local target",
            Self::ObsoleteReferenceRoot => "obsolete reference root",
            Self::ReferenceEscapesRoot => "reference escapes canonical root",
            Self::MissingCanonicalDocument => "missing canonical document",
            Self::CanonicalDocumentNotLinked => "canonical document not linked",
            Self::FalseMissingCanonicalClaim => "false missing-canonical claim",
            Self::InvalidReferenceLineRange => "invalid reference line range",
        };
        formatter.write_str(name)
    }
}

#[derive(Debug, Clone)]
pub struct CheckOptions {
    pub enforce_canonical_map: bool,
}

impl CheckOptions {
    pub const fn fixture() -> Self {
        Self {
            enforce_canonical_map: false,
        }
    }

    pub const fn repository() -> Self {
        Self {
            enforce_canonical_map: true,
        }
    }
}

const CANONICAL_DOCUMENTS: &[&str] = &[
    "docs/roadmap.md",
    "docs/player-guide.md",
    "docs/compatibility.md",
    "docs/data-and-persistence.md",
    "docs/contributing.md",
    "docs/decisions/README.md",
];

fn checked_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension, "md" | "rs" | "toml"))
}

fn enter(entry: &DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return true;
    }
    !matches!(
        entry.file_name().to_string_lossy().as_ref(),
        ".git" | ".worktrees" | "target" | "references"
    )
}

fn discover_checked_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_entry(enter) {
        let entry = entry.map_err(io::Error::other)?;
        if entry.file_type().is_file() && checked_extension(entry.path()) {
            files.push(entry.path().to_path_buf());
        }
    }
    files.sort();
    Ok(files)
}

/// Number of text files inspected by [`check_repository`].
pub fn checked_file_count(root: &Path) -> io::Result<usize> {
    discover_checked_files(root).map(|files| files.len())
}

fn relative_file(root: &Path, file: &Path) -> PathBuf {
    file.strip_prefix(root).unwrap_or(file).to_path_buf()
}

fn allow_historical_obsolete_token(file: &Path) -> bool {
    matches!(
        file.to_string_lossy().as_ref(),
        "docs/superpowers/specs/2026-07-13-documentation-truth-repair-design.md"
            | "tools/docs-check/tests/fixtures/stale-root/README.md"
    )
}

fn normalize_link_target(raw: &str) -> String {
    raw.trim()
        .trim_matches(|character| matches!(character, '<' | '>'))
        .split('#')
        .next()
        .unwrap_or_default()
        .replace("%20", " ")
}

fn is_external_or_anchor(target: &str) -> bool {
    target.is_empty()
        || target.starts_with('#')
        || target.starts_with("mailto:")
        || target.starts_with("app://")
        || target.contains("::")
        || target.contains("://")
}

fn local_target_path(root: &Path, file: &Path, target: &str) -> PathBuf {
    if let Some(root_relative) = target.strip_prefix('/') {
        root.join(root_relative)
    } else {
        file.parent().unwrap_or(root).join(target)
    }
}

fn reference_repository_root(root: &Path) -> &Path {
    if root.join("references/DTXmaniaNX").is_dir() {
        return root;
    }
    root.parent()
        .and_then(Path::parent)
        .filter(|candidate| candidate.join("references/DTXmaniaNX").is_dir())
        .unwrap_or(root)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReferenceLineRange {
    None,
    Bounds { start: usize, end: usize },
    Invalid,
}

fn parse_reference_location(token: &str) -> (&str, ReferenceLineRange) {
    let token = token.trim_end_matches(['.', ',', ';', ')', ']']);
    if let Some((path, suffix)) = token.rsplit_once(':') {
        if Path::new(path).extension().is_some() && !suffix.contains('/') {
            let location = suffix.strip_prefix('L').unwrap_or(suffix);
            let mut bounds = location.split('-');
            if let (Some(start), end, None) = (bounds.next(), bounds.next(), bounds.next()) {
                if let Ok(start) = start.parse::<usize>() {
                    match end {
                        Some(value) => match value.parse::<usize>() {
                            Ok(end) => {
                                return (path, ReferenceLineRange::Bounds { start, end });
                            }
                            Err(_) => return (path, ReferenceLineRange::Invalid),
                        },
                        None => {
                            return (path, ReferenceLineRange::Bounds { start, end: start });
                        }
                    }
                }
            }
            let looks_like_location = suffix.starts_with('L')
                || location
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_digit());
            return (
                path,
                if looks_like_location {
                    ReferenceLineRange::Invalid
                } else {
                    ReferenceLineRange::None
                },
            );
        }
    }
    (token, ReferenceLineRange::None)
}

fn check_file(root: &Path, file: &Path, failures: &mut Vec<CheckFailure>) -> io::Result<()> {
    let content = fs::read_to_string(file)?;
    let relative = relative_file(root, file);
    let markdown_link =
        Regex::new(r#"\[[^\]]*\]\((?P<target><[^>]+>|[^\s\)]+)"#).map_err(io::Error::other)?;
    let reference_path = Regex::new(r#"references/[^\s`\"'<>]+"#).map_err(io::Error::other)?;
    let obsolete = ["DTXmaniaNX", "BocuD"].join("-");

    for (line_index, line) in content.lines().enumerate() {
        let line_number = line_index + 1;
        if line.contains(&obsolete) && !allow_historical_obsolete_token(&relative) {
            failures.push(CheckFailure {
                file: relative.clone(),
                line: line_number,
                target: obsolete.clone(),
                reason: FailureReason::ObsoleteReferenceRoot,
            });
        }

        for captures in markdown_link.captures_iter(line).filter(|_| {
            relative
                .extension()
                .is_some_and(|extension| extension == "md")
        }) {
            let raw = captures
                .name("target")
                .map_or("", |capture| capture.as_str());
            if is_external_or_anchor(raw) {
                continue;
            }
            let target = normalize_link_target(raw);
            if target.is_empty() {
                continue;
            }
            if !local_target_path(root, file, &target).exists() {
                failures.push(CheckFailure {
                    file: relative.clone(),
                    line: line_number,
                    target,
                    reason: FailureReason::MissingLocalTarget,
                });
            }
        }

        if !line.contains(&obsolete) {
            for found in reference_path.find_iter(line) {
                let (token, parsed_line_range) = parse_reference_location(found.as_str());
                if token
                    .chars()
                    .any(|character| matches!(character, '[' | ']' | '*'))
                    || token.contains("...")
                {
                    continue;
                }
                let path = Path::new(token);
                let canonical_root = Path::new("references/DTXmaniaNX");
                if !path.starts_with(canonical_root)
                    || path
                        .components()
                        .any(|component| matches!(component, std::path::Component::ParentDir))
                {
                    failures.push(CheckFailure {
                        file: relative.clone(),
                        line: line_number,
                        target: token.to_owned(),
                        reason: FailureReason::ReferenceEscapesRoot,
                    });
                    continue;
                }
                let check_token = token
                    .split('*')
                    .next()
                    .unwrap_or(token)
                    .trim_end_matches('/');
                let reference_file = reference_repository_root(root).join(check_token);
                if !check_token.is_empty() && !reference_file.exists() {
                    failures.push(CheckFailure {
                        file: relative.clone(),
                        line: line_number,
                        target: token.to_owned(),
                        reason: FailureReason::MissingLocalTarget,
                    });
                } else if parsed_line_range == ReferenceLineRange::Invalid {
                    failures.push(CheckFailure {
                        file: relative.clone(),
                        line: line_number,
                        target: found
                            .as_str()
                            .trim_end_matches(['.', ',', ';', ')', ']'])
                            .to_owned(),
                        reason: FailureReason::InvalidReferenceLineRange,
                    });
                } else if let ReferenceLineRange::Bounds { start, end } = parsed_line_range {
                    let line_count = fs::read_to_string(&reference_file)?.lines().count();
                    if start == 0 || start > end || end > line_count {
                        failures.push(CheckFailure {
                            file: relative.clone(),
                            line: line_number,
                            target: found
                                .as_str()
                                .trim_end_matches(['.', ',', ';', ')', ']'])
                                .to_owned(),
                            reason: FailureReason::InvalidReferenceLineRange,
                        });
                    }
                }
            }
        }

        if !allow_historical_obsolete_token(&relative) {
            let lower = line.to_ascii_lowercase();
            let says_missing = lower.contains("missing")
                || lower.contains("does not exist")
                || lower.contains("absent");
            if says_missing {
                for canonical in CANONICAL_DOCUMENTS {
                    if line.contains(canonical) {
                        failures.push(CheckFailure {
                            file: relative.clone(),
                            line: line_number,
                            target: (*canonical).to_owned(),
                            reason: FailureReason::FalseMissingCanonicalClaim,
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

fn check_canonical_map(root: &Path, failures: &mut Vec<CheckFailure>) -> io::Result<()> {
    for document in CANONICAL_DOCUMENTS {
        if !root.join(document).is_file() {
            failures.push(CheckFailure {
                file: PathBuf::from(document),
                line: 0,
                target: (*document).to_owned(),
                reason: FailureReason::MissingCanonicalDocument,
            });
        }
    }

    let entry_points: &[(&str, &[&str])] = &[
        ("README.md", CANONICAL_DOCUMENTS),
        (
            "AGENTS.md",
            &[
                "docs/roadmap.md",
                "docs/contributing.md",
                "docs/decisions/README.md",
            ],
        ),
    ];
    for (entry_point, required) in entry_points {
        let path = root.join(entry_point);
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for target in *required {
            if !content.contains(target) {
                failures.push(CheckFailure {
                    file: PathBuf::from(entry_point),
                    line: 0,
                    target: (*target).to_owned(),
                    reason: FailureReason::CanonicalDocumentNotLinked,
                });
            }
        }
    }
    Ok(())
}

pub fn check_repository(root: &Path, options: CheckOptions) -> io::Result<Vec<CheckFailure>> {
    let root = fs::canonicalize(root)?;
    let files = discover_checked_files(&root)?;
    let mut failures = Vec::new();
    for file in files {
        let relative = relative_file(&root, &file);
        if options.enforce_canonical_map && relative.starts_with("tools/docs-check/tests/fixtures")
        {
            continue;
        }
        check_file(&root, &file, &mut failures)?;
    }
    if options.enforce_canonical_map {
        check_canonical_map(&root, &mut failures)?;
    }
    failures.sort_by(|left, right| {
        (&left.file, left.line, &left.target).cmp(&(&right.file, right.line, &right.target))
    });
    failures.dedup();
    Ok(failures)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    #[test]
    fn valid_fixture_has_no_failures() {
        assert!(check_repository(&fixture("valid"), CheckOptions::fixture())
            .unwrap()
            .is_empty());
    }

    #[test]
    fn failures_include_file_line_target_and_reason() {
        let failures = check_repository(&fixture("broken-link"), CheckOptions::fixture()).unwrap();
        assert_eq!(failures[0].file, PathBuf::from("README.md"));
        assert_eq!(failures[0].line, 3);
        assert_eq!(failures[0].target, "missing.md");
        assert_eq!(failures[0].reason, FailureReason::MissingLocalTarget);
    }

    #[test]
    fn stale_reference_path_and_missing_canonical_doc_fail() {
        let stale = check_repository(&fixture("stale-root"), CheckOptions::fixture()).unwrap();
        assert!(stale
            .iter()
            .any(|failure| failure.reason == FailureReason::ObsoleteReferenceRoot));

        let missing =
            check_repository(&fixture("missing-canonical"), CheckOptions::repository()).unwrap();
        assert!(missing
            .iter()
            .any(|failure| failure.reason == FailureReason::MissingCanonicalDocument));

        let false_claim =
            check_repository(&fixture("false-missing"), CheckOptions::fixture()).unwrap();
        assert!(false_claim
            .iter()
            .any(|failure| failure.reason == FailureReason::FalseMissingCanonicalClaim));
    }

    #[test]
    fn reference_ranges_must_be_ordered_and_within_file() {
        let failures =
            check_repository(&fixture("invalid-range"), CheckOptions::fixture()).unwrap();
        assert_eq!(failures.len(), 2);
        assert!(failures
            .iter()
            .all(|failure| failure.reason == FailureReason::InvalidReferenceLineRange));
    }

    #[test]
    fn malformed_reference_ranges_fail() {
        let failures =
            check_repository(&fixture("malformed-range"), CheckOptions::fixture()).unwrap();
        assert_eq!(failures.len(), 2);
        assert!(failures
            .iter()
            .all(|failure| failure.reason == FailureReason::InvalidReferenceLineRange));
    }
}
