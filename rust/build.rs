#[path = "src/build_support.rs"]
mod build_support;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use build_support::{hash_inventory, source_inventory};

fn main() {
    let crate_root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let files = source_inventory(&crate_root);

    for path in &files {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=src/bin");
    emit_git_rerun_paths(&crate_root);

    emit(
        "PROTEUS_SOURCE_DIGEST",
        &hash_inventory(&crate_root, &files),
    );
    emit(
        "PROTEUS_GIT_COMMIT",
        &git_output(&crate_root, &["rev-parse", "HEAD"]).unwrap_or_default(),
    );
    emit(
        "PROTEUS_SOURCE_DIRTY",
        &source_dirty(&crate_root, &files)
            .map(|dirty| dirty.to_string())
            .unwrap_or_else(|| "unknown".to_owned()),
    );
    emit(
        "PROTEUS_RUSTC_VERSION",
        &Command::new(env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .arg("--version")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
            .unwrap_or_else(|| "unknown".to_owned()),
    );
    emit("PROTEUS_CARGO_FEATURES", &enabled_features().join(","));
    emit(
        "PROTEUS_CARGO_PROFILE",
        &env::var("PROFILE").unwrap_or_else(|_| "unknown".to_owned()),
    );
    emit(
        "PROTEUS_TARGET_TRIPLE",
        &env::var("TARGET").unwrap_or_else(|_| "unknown".to_owned()),
    );
}

fn emit(name: &str, value: &str) {
    println!("cargo:rustc-env={name}={value}");
}

fn git_output(crate_root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(crate_root)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn emit_git_rerun_paths(crate_root: &Path) {
    let Some(head) = git_path(crate_root, "HEAD") else {
        return;
    };
    println!("cargo:rerun-if-changed={}", head.display());
    if let Some(index) = git_path(crate_root, "index") {
        println!("cargo:rerun-if-changed={}", index.display());
    }
    if let Some(packed_refs) = git_path(crate_root, "packed-refs") {
        println!("cargo:rerun-if-changed={}", packed_refs.display());
    }

    let Ok(contents) = fs::read_to_string(&head) else {
        return;
    };
    if let Some(reference) = contents.trim().strip_prefix("ref: ") {
        if let Some(reference_path) = git_path(crate_root, reference) {
            println!("cargo:rerun-if-changed={}", reference_path.display());
        }
    }
}

fn git_path(crate_root: &Path, logical_path: &str) -> Option<PathBuf> {
    git_output(
        crate_root,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            logical_path,
        ],
    )
    .map(PathBuf::from)
}

fn source_dirty(crate_root: &Path, files: &[PathBuf]) -> Option<bool> {
    let relative_paths = files
        .iter()
        .map(|path| path.strip_prefix(crate_root).ok())
        .collect::<Option<Vec<_>>>()?;
    let output = Command::new("git")
        .arg("-C")
        .arg(crate_root)
        .args(["status", "--porcelain", "--untracked-files=all", "--"])
        .args(relative_paths)
        .output()
        .ok()?;
    output.status.success().then_some(!output.stdout.is_empty())
}

fn enabled_features() -> Vec<String> {
    let mut features = env::vars_os()
        .filter_map(|(key, _)| {
            let key = key.to_string_lossy();
            key.strip_prefix("CARGO_FEATURE_")
                .map(|feature| feature.to_ascii_lowercase().replace('_', "-"))
        })
        .collect::<Vec<_>>();
    features.sort();
    features
}
