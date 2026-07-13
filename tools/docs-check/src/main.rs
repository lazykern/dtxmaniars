use std::path::PathBuf;
use std::process::ExitCode;

use docs_check::{check_repository, checked_file_count, CheckOptions};

fn main() -> ExitCode {
    let root = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    let checked = match checked_file_count(&root) {
        Ok(count) => count,
        Err(error) => {
            eprintln!("docs-check: {}: {error}", root.display());
            return ExitCode::FAILURE;
        }
    };
    match check_repository(&root, CheckOptions::repository()) {
        Ok(failures) => {
            for failure in &failures {
                println!(
                    "{}:{}: {}: {}",
                    failure.file.display(),
                    failure.line,
                    failure.target,
                    failure.reason
                );
            }
            println!(
                "docs-check: checked {checked} files; {} failure(s)",
                failures.len()
            );
            if failures.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("docs-check: {}: {error}", root.display());
            ExitCode::FAILURE
        }
    }
}
