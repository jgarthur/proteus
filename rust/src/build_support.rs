//! Build-script helpers for the canonical headless source inventory.

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

pub fn source_inventory(crate_root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for relative in ["Cargo.toml", "Cargo.lock", "build.rs"] {
        let path = crate_root.join(relative);
        if path.is_file() {
            files.push(path);
        }
    }
    collect_source_files(crate_root, &crate_root.join("src"), &mut files);
    files.sort_by(|left, right| {
        relative_bytes(crate_root, left).cmp(&relative_bytes(crate_root, right))
    });
    files
}

pub fn hash_inventory(crate_root: &Path, files: &[PathBuf]) -> String {
    let mut hasher = Sha256::new();
    for path in files {
        let relative = relative_bytes(crate_root, path);
        let content = fs::read(path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        hasher.update((relative.len() as u64).to_be_bytes());
        hasher.update(&relative);
        hasher.update((content.len() as u64).to_be_bytes());
        hasher.update(content);
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn collect_source_files(crate_root: &Path, directory: &Path, files: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|error| panic!("failed to enumerate {}: {error}", directory.display()));
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
        let relative = path
            .strip_prefix(crate_root)
            .expect("source path should be below crate root");
        if relative == Path::new("src/web") || relative == Path::new("src/bin/proteus-server.rs") {
            continue;
        }

        let file_type = entry
            .file_type()
            .unwrap_or_else(|error| panic!("failed to inspect {}: {error}", path.display()));
        if file_type.is_dir() {
            collect_source_files(crate_root, &path, files);
        } else if file_type.is_file() {
            files.push(path);
        } else {
            panic!(
                "selected build source is not a regular file: {}",
                path.display()
            );
        }
    }
}

fn relative_bytes(crate_root: &Path, path: &Path) -> Vec<u8> {
    path.strip_prefix(crate_root)
        .expect("source path should be below crate root")
        .to_str()
        .unwrap_or_else(|| panic!("source path is not UTF-8: {}", path.display()))
        .replace(std::path::MAIN_SEPARATOR, "/")
        .into_bytes()
}
