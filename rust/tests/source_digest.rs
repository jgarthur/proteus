#[path = "../src/build_support.rs"]
mod build_support;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use build_support::{hash_inventory, source_inventory};
use proteus::runner::build_provenance;

static SANDBOX_COUNTER: AtomicU64 = AtomicU64::new(0);

struct Sandbox(PathBuf);

impl Sandbox {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "proteus-source-digest-{}-{}",
            std::process::id(),
            SANDBOX_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("test sandbox should be unique");
        Self(path)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn compiled_source_digest_matches_the_current_canonical_inventory() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let files = source_inventory(&crate_root);
    assert_eq!(
        build_provenance().source_digest,
        hash_inventory(&crate_root, &files)
    );
}

#[test]
fn source_digest_is_stable_excludes_web_and_changes_for_selected_bytes() {
    let sandbox = Sandbox::new();
    fs::create_dir_all(sandbox.0.join("src/web")).unwrap();
    fs::create_dir_all(sandbox.0.join("src/bin")).unwrap();
    fs::write(sandbox.0.join("Cargo.toml"), b"manifest").unwrap();
    fs::write(sandbox.0.join("src/lib.rs"), b"selected-a").unwrap();
    fs::write(sandbox.0.join("src/web/page.html"), b"web-a").unwrap();
    fs::write(sandbox.0.join("src/bin/proteus-server.rs"), b"server-a").unwrap();

    let files = source_inventory(&sandbox.0);
    let first = hash_inventory(&sandbox.0, &files);
    assert_eq!(first, hash_inventory(&sandbox.0, &files));

    fs::write(sandbox.0.join("src/web/page.html"), b"web-b").unwrap();
    fs::write(sandbox.0.join("src/bin/proteus-server.rs"), b"server-b").unwrap();
    let after_web_change = source_inventory(&sandbox.0);
    assert_eq!(first, hash_inventory(&sandbox.0, &after_web_change));

    fs::write(sandbox.0.join("src/lib.rs"), b"selected-b").unwrap();
    let after_selected_change = source_inventory(&sandbox.0);
    assert_ne!(first, hash_inventory(&sandbox.0, &after_selected_change));
}
