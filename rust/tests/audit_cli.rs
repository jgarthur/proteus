//! Exercises reproducible audit artifacts against ordinary runner execution.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Sandbox(PathBuf);
impl Sandbox {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "proteus-audit-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn read(path: PathBuf) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// Audit artifacts replay byte-for-byte and final metrics equal the regular runner.
#[test]
fn audit_artifacts_are_deterministic_and_match_normal_execution() {
    let sandbox = Sandbox::new();
    let mut manifest: Value = serde_json::from_str(include_str!("../example-run.json")).unwrap();
    manifest["simulation"]["width"] = json!(8);
    manifest["simulation"]["height"] = json!(8);
    manifest["bootstrap"]["programs"][0]["x"] = json!(3);
    manifest["bootstrap"]["programs"][0]["y"] = json!(3);
    manifest["bootstrap"]["environment"] = json!([]);
    manifest["limits"]["ticks"] = json!(31);
    manifest["observation"]["every_n_ticks"] = json!(10);
    manifest["output_directory"] = json!(sandbox.0.join("normal"));
    let input = sandbox.0.join("input.json");
    fs::write(&input, serde_json::to_vec(&manifest).unwrap()).unwrap();
    for name in ["a", "b"] {
        let output = Command::new(env!("CARGO_BIN_EXE_proteus-audit"))
            .arg("--manifest")
            .arg(&input)
            .arg("--output")
            .arg(sandbox.0.join(name))
            .args(["--trace-start", "0", "--trace-end", "31"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    for artifact in [
        "manifest.json",
        "metrics.jsonl",
        "lineage.jsonl",
        "genomes.jsonl",
        "events.jsonl",
        "births.jsonl",
        "actions.jsonl",
        "summary.json",
    ] {
        assert_eq!(
            fs::read(sandbox.0.join("a").join(artifact)).unwrap(),
            fs::read(sandbox.0.join("b").join(artifact)).unwrap(),
            "{artifact}"
        );
    }
    let births = fs::read_to_string(sandbox.0.join("a/births.jsonl")).unwrap();
    let boot_count = read(sandbox.0.join("a/summary.json"))["genome_events"]["boots"]
        .as_u64()
        .unwrap();
    assert!(boot_count > 0, "CLI fixture must exercise boot logging");
    assert_eq!(births.lines().count() as u64, boot_count);
    let metrics = fs::read_to_string(sandbox.0.join("a/metrics.jsonl")).unwrap();
    let ticks: Vec<_> = metrics
        .lines()
        .map(|s| {
            serde_json::from_str::<Value>(s).unwrap()["tick"]
                .as_u64()
                .unwrap()
        })
        .collect();
    assert_eq!(ticks, [0, 10, 20, 30, 31]);
    let output = Command::new(env!("CARGO_BIN_EXE_proteus-run"))
        .arg("--manifest")
        .arg(&input)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        read(sandbox.0.join("a/summary.json"))["final_metrics"],
        read(sandbox.0.join("normal/summary.json"))["final_metrics"]
    );
    let output = Command::new(env!("CARGO_BIN_EXE_proteus-audit"))
        .arg("--manifest")
        .arg(&input)
        .arg("--output")
        .arg(sandbox.0.join("a"))
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "audit must not overwrite existing output"
    );
}
