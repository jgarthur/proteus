#!/usr/bin/env python3
"""Generate and run paired reproduction-fidelity experiments with proteus-audit."""

import argparse
from concurrent.futures import ThreadPoolExecutor
import csv
import hashlib
import json
import os
from pathlib import Path
import subprocess
import shutil

ORGANISMS = {
    "driftkin": ([81, 64, 0, 66, 83, 48, 0, 48, 81, 85, 0, 93, 81, 49, 64, 49, 81, 100, 81, 99, 81, 99, 81, 81, 81, 81, 81, 81, 49, 64, 85, 66, 95, 48, 66, 85, 48, 95, 81], .25, 1., 1., 7, 20, 12),
    "squarestep": ([83, 66, 48, 85, 95, 49, 100, 64, 99], .5, .3, .55, 14, 0, 0),
    "wildstep": ([81, 83, 66, 48, 85, 95, 49, 100, 20, 73, 99], .25, .2, .5, 14, 16, 16),
    "radchain": ([83, 66, 48, 85, 95, 49, 100], 2., 1.25, .6, 10, 0, 0),
}
CONDITIONS = ("reference", "no-maintenance", "mutation-default", "mutation-default-no-maintenance")


def manifest(organism, condition, seed, ticks, cadence):
    """Return a library regime (or Driftkin's frontend regime) with an intervention."""
    code, energy, mass, alpha, maintenance, preload_e, preload_m = ORGANISMS[organism]
    mutation = condition.startswith("mutation-default")
    name = f"{organism}-{condition}-seed-{seed}"
    return {
        "runner_schema_version": "0.2.0", "run_id": name,
        "simulation": {
            "width": 64, "height": 64, "seed": seed,
            "r_energy": energy, "r_mass": mass,
            "d_energy_log2": 7, "d_mass_log2": 7, "t_cap": 4.,
            "maintenance_rate_log2": None if condition.endswith("no-maintenance") else maintenance,
            "maintenance_exponent": 1., "local_action_exponent": alpha,
            "n_synth": 1, "inert_grace_ticks": 10, "p_spawn_log2": None,
            "mutation_base_log2": 16 if mutation else 63,
            "mutation_background_log2": 8 if mutation else 63,
        },
        "bootstrap": {"programs": [{"x": 32, "y": 32, "code": code,
                                       "free_energy": preload_e, "free_mass": preload_m}],
                      "environment": []},
        "limits": {"ticks": ticks}, "observation": {"every_n_ticks": cadence},
        "output_directory": f"unused-normal-runner/{name}",
    }


def summarize(directory, organism, condition, seed):
    """Extract explicit denominators and original-genome survival from a run."""
    summary = json.loads((directory / "summary.json").read_text())
    with (directory / "metrics.jsonl").open() as stream:
        samples = [json.loads(line) for line in stream]
    last = samples[-1]
    original = hashlib.sha256(bytes(ORGANISMS[organism][0])).hexdigest()
    counts = last["live_genome_counts"]
    totals = summary["genome_events"]
    population = last["live"]
    sizes = {}
    with (directory / "genomes.jsonl").open() as stream:
        for line in stream:
            record = json.loads(line)
            sizes[record["genome"]] = len(record["code"])
    return {
        "organism": organism, "condition": condition, "seed": seed,
        "ticks": summary["tick"], "live": population,
        "pending_constructions": summary["pending_constructions"],
        "live_original_genome": counts.get(original, 0), "unique_live_genomes": len(counts),
        "mean_live_size": sum(sizes[key] * value for key, value in counts.items()) / population if population else 0,
        **totals,
        "late_boots": totals["boots"] - samples[-2]["genome_events"]["boots"] if len(samples) > 1 else 0,
        "late_matching_boots": totals["matching_creator_at_creation_boots"] - samples[-2]["genome_events"]["matching_creator_at_creation_boots"] if len(samples) > 1 else 0,
        "late_boots_with_known_construction": totals["boots_with_known_construction"] - samples[-2]["genome_events"]["boots_with_known_construction"] if len(samples) > 1 else 0,
        "late_interval_start": samples[-2]["tick"] if len(samples) > 1 else 0,
        "late_clean_boots": totals["clean_single_contributor_boots"] - samples[-2]["genome_events"]["clean_single_contributor_boots"] if len(samples) > 1 else 0,
        "late_clean_matching_boots": totals["clean_matching_boots"] - samples[-2]["genome_events"]["clean_matching_boots"] if len(samples) > 1 else 0,
        "input_digest": summary["input_digest"],
    }


def archive_source(source_root, destination, expected_digest):
    """Archive Rust inputs and verify the build script's canonical source digest."""
    destination.mkdir()
    for name in ["Cargo.toml", "Cargo.lock", "build.rs"]:
        shutil.copy2(source_root / name, destination / name)
    shutil.copytree(source_root / "src", destination / "src")
    # Mirror src/build_support.rs: web sources do not enter headless provenance.
    selected = [destination / name for name in ["Cargo.toml", "Cargo.lock", "build.rs"]]
    for path in (destination / "src").rglob("*"):
        relative = path.relative_to(destination)
        if any(part.startswith(".") for part in relative.parts):
            continue
        if relative.parts[:2] == ("src", "web") or relative.as_posix() == "src/bin/proteus-server.rs":
            continue
        if path.is_file():
            selected.append(path)
    digest = hashlib.sha256()
    for path in sorted(selected, key=lambda p: p.relative_to(destination).as_posix().encode()):
        relative = path.relative_to(destination).as_posix().encode()
        content = path.read_bytes()
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        digest.update(len(content).to_bytes(8, "big"))
        digest.update(content)
    actual = "sha256:" + digest.hexdigest()
    if actual != expected_digest:
        raise ValueError(f"Rust source differs from the binary: {actual} != {expected_digest}; rebuild the binary")
    return actual


def main():
    """Execute an explicit matrix and write a machine-readable comparison table."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--seeds", type=int, nargs="+", default=[1, 2, 3])
    parser.add_argument("--ticks", type=int, default=2000)
    parser.add_argument("--cadence", type=int, default=100)
    parser.add_argument("--trace-start", type=int, default=0)
    parser.add_argument("--trace-end", type=int, default=0)
    parser.add_argument("--jobs", type=int, default=2)
    parser.add_argument("--organisms", nargs="+", choices=ORGANISMS, default=["squarestep", "wildstep", "radchain"])
    parser.add_argument("--conditions", nargs="+", choices=CONDITIONS, default=list(CONDITIONS))
    args = parser.parse_args()
    if min(args.ticks, args.cadence, args.jobs) < 1 or any(seed < 0 for seed in args.seeds):
        parser.error("ticks, cadence and jobs must be positive; seeds must be nonnegative")
    if not 0 <= args.trace_start <= args.trace_end:
        parser.error("trace interval must satisfy 0 <= start <= end")
    binary = args.binary.resolve(strict=True)
    args.output.mkdir(parents=True, exist_ok=False)
    pinned_binary = args.output.resolve() / "proteus-audit"
    shutil.copy2(binary, pinned_binary)
    binary = pinned_binary
    build_info = json.loads(subprocess.check_output([str(binary), "--build-info"], text=True))
    source_digest = archive_source(args.source.resolve(), args.output / "rust-source", build_info["build"]["source_digest"])
    matrix = [(organism, condition, seed) for organism in args.organisms
              for condition in args.conditions for seed in args.seeds]
    if len(set(matrix)) != len(matrix):
        parser.error("duplicate experiment cells")
    script_hashes = {}
    for name in ["reproduction_experiment.py", "analyze_genomes.py"]:
        source = Path(__file__).with_name(name)
        content = source.read_bytes()
        (args.output / name).write_bytes(content)
        script_hashes[name] = hashlib.sha256(content).hexdigest()
    (args.output / "experiment.json").write_text(json.dumps({
        "schema": "1.0.0", "ticks": args.ticks, "cadence": args.cadence,
        "source_digest": source_digest, "script_sha256": script_hashes, "engine_threads": 1,
        "matrix": matrix, "trace_start": args.trace_start, "trace_end": args.trace_end, "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
    }, indent=2) + "\n")

    def run(cell):
        """Run one exact manifest; propagate failures instead of dropping seeds."""
        organism, condition, seed = cell
        data = manifest(organism, condition, seed, args.ticks, args.cadence)
        name = data["run_id"]
        path = args.output / f"{name}.json"
        path.write_text(json.dumps(data, indent=2) + "\n")
        destination = args.output / name
        with (args.output / f"{name}.log").open("w") as log:
            subprocess.run([str(binary), "--manifest", str(path), "--output", str(destination),
                            "--trace-start", str(args.trace_start), "--trace-end", str(args.trace_end)],
                           stdout=log, stderr=subprocess.STDOUT, check=True,
                           env={**os.environ, "RAYON_NUM_THREADS": "1"})
        print(f"completed {name}", flush=True)
        return summarize(destination, organism, condition, seed)

    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        rows = list(pool.map(run, matrix))
    with (args.output / "results.csv").open("w", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    (args.output / "results.json").write_text(json.dumps(rows, indent=2) + "\n")


if __name__ == "__main__":
    main()
