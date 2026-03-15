from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "src"))

from cases import build_case
from proteus.analysis import evaluate_milestones


def _format_program_summary(label: str, payload: dict | None) -> list[str]:
    if payload is None:
        return [f"{label}: none"]
    preview = ", ".join(payload["disassembly"][:6])
    if len(payload["disassembly"]) == 6:
        preview = f"{preview}, ..."
    return [
        (
            f"{label}: size={payload['size']} age={payload['age']} live={payload['live']}"
            f" energy={payload['free_energy']} mass={payload['free_mass']}"
            f" bg={payload['background_radiation']} hash={payload['hash']}"
            f" at=({payload['x']},{payload['y']})"
        ),
        f"  code: {preview}",
    ]


def _print_text_report(results: dict) -> None:
    config = results["config"]
    print(
        f"backend={results['engine_backend']}"
        f" world={config['width']}x{config['height']}"
        f" rng_seed={config['rng_seed']}"
    )
    print(
        "system_params:"
        f" R_energy={config['system_params']['R_energy']}"
        f" R_mass={config['system_params']['R_mass']}"
        f" P_spawn={config['system_params']['P_spawn']}"
        f" D_energy={config['system_params']['D_energy']}"
        f" D_mass={config['system_params']['D_mass']}"
        f" M={config['system_params']['M']}"
    )
    if config["original_seed_hash"]:
        print(f"original_seed_hash={config['original_seed_hash']}")

    for snapshot in results["snapshots"]:
        summary = snapshot["summary"]
        population = snapshot["population"]
        resources = snapshot["resources"]
        size_stats = population["size_stats"]
        print()
        print(f"== tick {snapshot['tick']} ==")
        print(
            "summary:"
            f" occupied={summary['occupied_cells']}"
            f" live={summary['live_programs']}"
            f" inert={summary['inert_programs']}"
            f" instructions={summary['total_instructions']}"
            f" free_energy={summary['total_free_energy']}"
            f" free_mass={summary['total_free_mass']}"
            f" background={summary['total_background_radiation']}"
        )
        print(
            "population:"
            f" hashes={population['unique_hashes']}"
            f" singletons={population['singletons']}"
            f" original_seed={population['original_seed_hash_count']}"
            f" size_median={size_stats['median']}"
            f" size_p90={size_stats['p90']}"
            f" size_max={size_stats['max']}"
        )
        print(
            "resources:"
            f" occ_energy={resources['occupied_free_energy']}"
            f" empty_energy={resources['empty_free_energy']}"
            f" occ_mass={resources['occupied_free_mass']}"
            f" empty_mass={resources['empty_free_mass']}"
            f" occ_bg={resources['occupied_background']}"
            f" empty_bg={resources['empty_background']}"
        )
        top_hashes = snapshot["population"]["top_hashes"]
        if top_hashes:
            print("top_hashes:")
            for entry in top_hashes:
                preview = ", ".join(entry["example_disassembly"][:5])
                if len(entry["example_disassembly"]) == 5:
                    preview = f"{preview}, ..."
                print(
                    f"  {entry['hash']} count={entry['count']} live={entry['live']}"
                    f" inert={entry['inert']} size_range={entry['size_range'][0]}-{entry['size_range'][1]}"
                    f" preview={preview}"
                )
        for line in _format_program_summary("largest_program", snapshot["outliers"]["largest_program"]):
            print(line)
        for line in _format_program_summary("energy_rich_program", snapshot["outliers"]["energy_rich_program"]):
            print(line)
        for line in _format_program_summary("mass_rich_program", snapshot["outliers"]["mass_rich_program"]):
            print(line)


def main() -> None:
    parser = argparse.ArgumentParser(description="Evaluate Proteus runs at milestone ticks.")
    parser.add_argument("--case", choices=["default", "empty64"], default="default")
    parser.add_argument(
        "--ticks",
        type=int,
        nargs="+",
        default=[0, 100, 1000, 10000, 20000],
        help="Milestone ticks to snapshot.",
    )
    parser.add_argument("--top-hashes", type=int, default=5, help="How many dominant hashes to report per snapshot.")
    parser.add_argument(
        "--disassembly-limit",
        type=int,
        default=16,
        help="How many instructions to retain in outlier/hash previews.",
    )
    parser.add_argument("--json", action="store_true", help="Emit the full report as JSON.")
    args = parser.parse_args()

    config, _ = build_case(args.case)
    results = evaluate_milestones(
        config,
        args.ticks,
        top_hash_limit=max(1, args.top_hashes),
        disassembly_limit=max(1, args.disassembly_limit),
    )
    if args.json:
        print(json.dumps(results, indent=2))
        return
    _print_text_report(results)


if __name__ == "__main__":
    main()
