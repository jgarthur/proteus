#!/usr/bin/env bash
# Compare causal artifacts across serial and Rayon executions of a growing world.
set -euo pipefail
AUDIT_RUST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$AUDIT_RUST_DIR"
AUDIT_WORK="$(mktemp -d)"
trap 'rm -rf "$AUDIT_WORK"' EXIT
python3 - "$AUDIT_WORK" <<'PY'
import json
from pathlib import Path
import sys
sys.path.insert(0, "scripts")
from reproduction_experiment import manifest
Path(sys.argv[1], "input.json").write_text(json.dumps(manifest("squarestep", "mutation-default", 1, 400, 100)))
PY
cargo run --quiet --bin proteus-audit -- --manifest "$AUDIT_WORK/input.json" \
    --output "$AUDIT_WORK/serial" --trace-end 400
for threads in 1 4; do
    RAYON_NUM_THREADS="$threads" cargo run --quiet --features rayon --bin proteus-audit -- \
        --manifest "$AUDIT_WORK/input.json" --output "$AUDIT_WORK/rayon-$threads" --trace-end 400
    # Build identity and thread count intentionally differ in manifest.json.
    for artifact in metrics.jsonl lineage.jsonl genomes.jsonl births.jsonl events.jsonl actions.jsonl summary.json; do
        cmp "$AUDIT_WORK/serial/$artifact" "$AUDIT_WORK/rayon-$threads/$artifact"
    done
done
python3 - "$AUDIT_WORK/serial/summary.json" <<'PY'
import json
from pathlib import Path
import sys
counts = json.loads(Path(sys.argv[1]).read_text())["genome_events"]
for key in ["appends", "boots", "moves", "base_mutations", "background_mutations", "maintenance_deleted_bytes"]:
    assert counts[key] > 0, ("inactive parity fixture", key)
print("Genome audit artifacts match byte-for-byte: serial == Rayon at 1 and 4 threads.")
PY
