use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

use atomicwrites::{AtomicFile, Error as AtomicWriteError, OverwriteBehavior};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileName(String);

impl ProfileName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProfileNameError {
    #[error("profile name is blank")]
    Blank,
    #[error("profile name exceeds 48 characters")]
    TooLong,
    #[error("profile name contains a control character")]
    ControlCharacter,
    #[error("profile name is reserved")]
    Reserved,
    #[error("profile name already exists")]
    Duplicate,
}

pub fn comparison_key(name: &str) -> String {
    name.trim().chars().flat_map(char::to_lowercase).collect()
}

pub fn validate_profile_name<'a>(
    raw: &str,
    reserved: impl IntoIterator<Item = &'a str>,
    existing: impl IntoIterator<Item = &'a str>,
    current: Option<&str>,
) -> Result<ProfileName, ProfileNameError> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(ProfileNameError::Blank);
    }
    if name.chars().count() > 48 {
        return Err(ProfileNameError::TooLong);
    }
    if raw.chars().any(char::is_control) {
        return Err(ProfileNameError::ControlCharacter);
    }

    let key = comparison_key(name);
    if reserved.into_iter().any(|name| comparison_key(name) == key) {
        return Err(ProfileNameError::Reserved);
    }
    if current.is_none_or(|name| comparison_key(name) != key)
        && existing.into_iter().any(|name| comparison_key(name) == key)
    {
        return Err(ProfileNameError::Duplicate);
    }

    Ok(ProfileName(name.to_owned()))
}

pub fn suggest_copy_name<'a>(base: &str, existing: impl IntoIterator<Item = &'a str>) -> String {
    let base = base.trim();
    let stem = base
        .rsplit_once(' ')
        .filter(|(_, suffix)| {
            !suffix.is_empty() && suffix.chars().all(|char| char.is_ascii_digit())
        })
        .map_or(base, |(stem, _)| stem);
    let existing: Vec<_> = existing.into_iter().map(comparison_key).collect();

    for suffix in 2.. {
        let candidate = format!("{stem} {suffix}");
        if !existing
            .iter()
            .any(|name| name == &comparison_key(&candidate))
        {
            return candidate;
        }
    }

    unreachable!("finite existing names leave a copy suffix available")
}

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("cannot create parent directory for {path}: {source}")]
    CreateParent { path: PathBuf, source: io::Error },
    #[error("cannot write replacement for {path}: {source}")]
    Write { path: PathBuf, source: io::Error },
    #[error("cannot commit replacement for {path}: {source}")]
    Commit { path: PathBuf, source: io::Error },
}

pub fn replace_bytes(path: &Path, bytes: &[u8]) -> Result<(), PersistenceError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|source| PersistenceError::CreateParent {
            path: path.to_path_buf(),
            source,
        })?;
    }

    AtomicFile::new(path, OverwriteBehavior::AllowOverwrite)
        .write(|file| file.write_all(bytes))
        .map_err(|error| match error {
            AtomicWriteError::User(source) => PersistenceError::Write {
                path: path.to_path_buf(),
                source,
            },
            AtomicWriteError::Internal(source) => PersistenceError::Commit {
                path: path.to_path_buf(),
                source,
            },
        })
}

#[cfg(test)]
mod tests {
    use super::{
        comparison_key, replace_bytes, suggest_copy_name, validate_profile_name, PersistenceError,
        ProfileNameError,
    };
    use std::{fs, path::PathBuf};

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join("dtx-persistence-tests")
            .join(std::process::id().to_string())
            .join(name)
    }

    #[test]
    fn profile_name_trims_and_accepts_48_scalars() {
        let raw = format!("  {}  ", "é".repeat(48));

        let name = validate_profile_name(&raw, [], [], None).expect("name is valid");

        assert_eq!(name.0, "é".repeat(48));
    }

    #[test]
    fn profile_name_rejects_blank_control_and_49_scalars() {
        assert!(matches!(
            validate_profile_name("  \t", [], [], None),
            Err(ProfileNameError::Blank)
        ));
        assert!(matches!(
            validate_profile_name("\tkit", [], [], None),
            Err(ProfileNameError::ControlCharacter)
        ));
        assert!(matches!(
            validate_profile_name(&"é".repeat(49), [], [], None),
            Err(ProfileNameError::TooLong)
        ));
    }

    #[test]
    fn profile_name_rejects_reserved_case_insensitively() {
        assert!(matches!(
            validate_profile_name("  ADMIN  ", ["admin"], [], None),
            Err(ProfileNameError::Reserved)
        ));
    }

    #[test]
    fn profile_name_rejects_duplicate_case_insensitively() {
        assert!(matches!(
            validate_profile_name("  Studio Kit  ", [], ["studio kit"], None),
            Err(ProfileNameError::Duplicate)
        ));
    }

    #[test]
    fn profile_name_allows_current_name_case_insensitively() {
        let name = validate_profile_name("  Studio Kit  ", [], ["studio kit"], Some("Studio Kit"))
            .expect("current name is valid");

        assert_eq!(name.0, "Studio Kit");
    }

    #[test]
    fn profile_name_exposes_validated_text() {
        let name = validate_profile_name("  Studio Kit  ", [], [], None).expect("name is valid");

        assert_eq!(name.as_str(), "Studio Kit");
    }

    #[test]
    fn comparison_key_does_not_normalize_unicode() {
        assert_ne!(comparison_key("é"), comparison_key("e\u{301}"));
    }

    #[test]
    fn copy_name_increments_numeric_suffix() {
        assert_eq!(
            suggest_copy_name("Studio kit", ["Studio kit", "Studio kit 2", "Studio kit 3"],),
            "Studio kit 4"
        );
    }

    #[test]
    fn copy_name_strips_trailing_numeric_suffix_before_suggesting() {
        assert_eq!(suggest_copy_name("Studio kit 7", []), "Studio kit 2");
    }

    #[test]
    fn replace_bytes_creates_parent_and_file() {
        let path = temp_path("creates-parent/deep/file.bin");
        let root = path
            .parent()
            .expect("file has parent")
            .parent()
            .expect("parent has parent");
        let _ = fs::remove_dir_all(root);

        replace_bytes(&path, b"contents").expect("replacement succeeds");

        assert_eq!(fs::read(&path).expect("replacement exists"), b"contents");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn replace_bytes_writes_single_component_relative_path() {
        let path = PathBuf::from(format!(
            "dtx-persistence-single-component-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time is after epoch")
                .as_nanos(),
        ));
        assert_eq!(path.parent(), Some(std::path::Path::new("")));

        replace_bytes(&path, b"contents").expect("replacement succeeds");

        assert_eq!(fs::read(&path).expect("replacement exists"), b"contents");
        fs::remove_file(&path).expect("test file cleanup succeeds");
    }

    #[test]
    fn replace_bytes_overwrites_complete_contents() {
        let path = temp_path("overwrites/file.bin");
        fs::create_dir_all(path.parent().expect("file has parent"))
            .expect("parent creation succeeds");
        fs::write(&path, b"old contents that must not remain").expect("initial write succeeds");

        replace_bytes(&path, b"new").expect("replacement succeeds");

        assert_eq!(fs::read(&path).expect("replacement exists"), b"new");
        let _ = fs::remove_dir_all(path.parent().expect("file has parent"));
    }

    #[test]
    fn directory_target_reports_commit_error_without_deletion() {
        let path = temp_path("directory-target");
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("directory creation succeeds");

        let error = replace_bytes(&path, b"replacement").expect_err("directory cannot be replaced");

        assert!(matches!(error, PersistenceError::Commit { .. }));
        assert!(path.is_dir());
        let _ = fs::remove_dir_all(&path);
    }
}
