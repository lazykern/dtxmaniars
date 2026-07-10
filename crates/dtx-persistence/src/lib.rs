use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

use atomicwrites::{AtomicFile, Error as AtomicWriteError, OverwriteBehavior};
use thiserror::Error;

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
    use super::{replace_bytes, PersistenceError};
    use std::{fs, path::PathBuf};

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join("dtx-persistence-tests")
            .join(std::process::id().to_string())
            .join(name)
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
