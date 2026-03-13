from __future__ import annotations

import argparse
from copy import deepcopy
import json
from pathlib import Path
import sys
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "src"))

from cases import build_case
from proteus.analysis import evaluate_milestones


REGIMES: dict[str, dict[str, float | int]] = {
    "default": {},
    "no_spawn": {
        "P_spawn": 0.0,
    },
    "no_spawn_low_maintenance": {
        "P_spawn": 0.0,
        "M": 1 / 128,
    },
    "no_spawn_high_buffer": {
        "P_spawn": 0.0,
        "T_cap": 4,
    },
    "no_spawn_builder_window": {
        "P_spawn": 0.0,
        "M": 1 / 128,
        "T_cap": 4,
        "N_synth": 1,
    },
    "lean_ambient": {
        "P_spawn": 0.0,
        "R_energy": 0.12,
        "D_energy": 0.02,
        "R_mass": 0.02,
        "D_mass": 0.04,
    },
    "builder_friendly": {
        "P_spawn": 0.0,
        "R_energy": 0.12,
        "D_energy": 0.02,
        "R_mass": 0.02,
        "D_mass": 0.04,
        "M": 1 / 128,
        "T_cap": 4,
        "N_synth": 1,
    },
    "builder_friendly_harsher": {
        "P_spawn": 0.0,
        "R_energy": 0.08,
        "D_energy": 0.03,
        "R_mass": 0.015,
        "D_mass": 0.04,
        "M": 1 / 128,
        "T_cap": 4,
        "N_synth": 1,
    },
}


def build_regime_config(case_name: str, regime_name: str):
    config, _ = build_case(case_name)
    config = deepcopy(config)
    overrides = REGIMES[regime_name]
    for field_name, value in overrides.items():
        setattr(config.system_params, field_name, value)
    return config


def score_snapshot(snapshot: dict[str, Any]) -> dict[str, float]:
    population = snapshot["population"]
    size_shares = population["size_bucket_shares"]
    size_stats = population["size_stats"]
    effective_size_stats = population["live_effective_size_stats"]
    structure = population["structure"]
    unique_hashes = float(population["unique_hashes"])
    program_count = float(max(1, population["program_count"]))
    dominant_hash_share = float(population["dominant_hash_share"])
    score = (
        3.0 * structure["live_builder_share"]
        + 4.0 * structure["live_replicator_share"]
        + 1.5 * min(float(effective_size_stats["median"] or 0.0) / 8.0, 1.0)
        + 1.0 * size_shares["size_8_plus"]
        + 0.75 * min(unique_hashes / 64.0, 1.0)
        + 0.5 * min(float(size_stats["max"] or 0) / 32.0, 1.0)
        - 2.0 * structure["live_absorb_only_share"]
        - 1.5 * structure["live_absorb_dominated_share"]
        - 1.5 * structure["live_nop_only_share"]
        - 0.75 * size_shares["size_1"]
        - 1.0 * dominant_hash_share
    )
    return {
        "anti_scavenger_score": score,
        "size_1_share": float(size_shares["size_1"]),
        "size_8_plus_share": float(size_shares["size_8_plus"]),
        "builder_share": float(structure["live_builder_share"]),
        "replicator_share": float(structure["live_replicator_share"]),
        "absorb_only_share": float(structure["live_absorb_only_share"]),
        "absorb_dominated_share": float(structure["live_absorb_dominated_share"]),
        "nop_only_share": float(structure["live_nop_only_share"]),
        "dominant_hash_share": dominant_hash_share,
        "unique_hashes_per_program": unique_hashes / program_count,
    }


def sweep_regimes(
    case_name: str,
    regimes: list[str],
    ticks: list[int],
    *,
    top_hash_limit: int,
    disassembly_limit: int,
) -> list[dict[str, Any]]:
    results: list[dict[str, Any]] = []
    for regime_name in regimes:
        config = build_regime_config(case_name, regime_name)
        evaluation = evaluate_milestones(
            config,
            ticks,
            top_hash_limit=top_hash_limit,
            disassembly_limit=disassembly_limit,
        )
        final_snapshot = evaluation["snapshots"][-1]
        results.append(
            {
                "regime": regime_name,
                "overrides": REGIMES[regime_name],
                "score": score_snapshot(final_snapshot),
                "final_snapshot": final_snapshot,
                "evaluation": evaluation,
            }
        )
    results.sort(key=lambda item: item["score"]["anti_scavenger_score"], reverse=True)
    return results


def _print_table(results: list[dict[str, Any]]) -> None:
    print(
        "regime".ljust(26),
        "score".rjust(8),
        "occ".rjust(6),
        "live".rjust(6),
        "uniq".rjust(6),
        "build".rjust(8),
        "repl".rjust(8),
        "abs1".rjust(8),
        "absdom".rjust(8),
        "dom".rjust(8),
        "effmed".rjust(8),
        "max".rjust(6),
    )
    for result in results:
        snapshot = result["final_snapshot"]
        population = snapshot["population"]
        structure = population["structure"]
        effective_size_stats = population["live_effective_size_stats"]
        size_stats = population["size_stats"]
        print(
            result["regime"].ljust(26),
            f"{result['score']['anti_scavenger_score']:.3f}".rjust(8),
            str(snapshot["summary"]["occupied_cells"]).rjust(6),
            str(snapshot["summary"]["live_programs"]).rjust(6),
            str(population["unique_hashes"]).rjust(6),
            f"{structure['live_builder_share']:.3f}".rjust(8),
            f"{structure['live_replicator_share']:.3f}".rjust(8),
            f"{structure['live_absorb_only_share']:.3f}".rjust(8),
            f"{structure['live_absorb_dominated_share']:.3f}".rjust(8),
            f"{population['dominant_hash_share']:.3f}".rjust(8),
            str(int(effective_size_stats["median"] or 0)).rjust(8),
            str(int(size_stats["max"] or 0)).rjust(6),
        )


def _print_details(results: list[dict[str, Any]]) -> None:
    for result in results:
        snapshot = result["final_snapshot"]
        population = snapshot["population"]
        resources = snapshot["resources"]
        top_hash = population["top_hashes"][0] if population["top_hashes"] else None
        print()
        print(f"== {result['regime']} ==")
        print(f"overrides={result['overrides']}")
        print(f"score={result['score']['anti_scavenger_score']:.3f}")
        print(
            "final:"
            f" occupied={snapshot['summary']['occupied_cells']}"
            f" live={snapshot['summary']['live_programs']}"
            f" inert={snapshot['summary']['inert_programs']}"
            f" instructions={snapshot['summary']['total_instructions']}"
            f" unique_hashes={population['unique_hashes']}"
            f" dominant_hash_share={population['dominant_hash_share']:.3f}"
        )
        print(
            "sizes:"
            f" size_1={population['size_bucket_shares']['size_1']:.3f}"
            f" size_8_plus={population['size_bucket_shares']['size_8_plus']:.3f}"
            f" size_16_plus={population['size_bucket_shares']['size_16_plus']:.3f}"
            f" median={population['size_stats']['median']}"
            f" effective_median={population['live_effective_size_stats']['median']}"
            f" p90={population['size_stats']['p90']}"
            f" max={population['size_stats']['max']}"
        )
        print(
            "structure:"
            f" builder={population['structure']['live_builder_share']:.3f}"
            f" replicator={population['structure']['live_replicator_share']:.3f}"
            f" absorb_only={population['structure']['live_absorb_only_share']:.3f}"
            f" absorb_dominated={population['structure']['live_absorb_dominated_share']:.3f}"
            f" nop_only={population['structure']['live_nop_only_share']:.3f}"
            f" entropy={population['structure']['live_opcode_entropy_mean']:.3f}"
            f" concentration={population['structure']['live_opcode_concentration_mean']:.3f}"
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
        if top_hash:
            preview = ", ".join(top_hash["example_disassembly"][:6])
            print(
                "top_hash:"
                f" hash={top_hash['hash']}"
                f" count={top_hash['count']}"
                f" preview={preview}"
            )


def main() -> None:
    parser = argparse.ArgumentParser(description="Sweep Proteus parameter regimes for anti-scavenger behavior.")
    parser.add_argument("--case", choices=["default", "empty64"], default="default")
    parser.add_argument(
        "--regimes",
        nargs="+",
        default=list(REGIMES),
        choices=sorted(REGIMES),
        help="Which preset regimes to evaluate.",
    )
    parser.add_argument(
        "--ticks",
        type=int,
        nargs="+",
        default=[0, 1000, 10000, 20000],
        help="Milestone ticks to evaluate for each regime.",
    )
    parser.add_argument("--top-hashes", type=int, default=3)
    parser.add_argument("--disassembly-limit", type=int, default=12)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--details", action="store_true", help="Print per-regime details after the summary table.")
    args = parser.parse_args()

    results = sweep_regimes(
        args.case,
        args.regimes,
        args.ticks,
        top_hash_limit=max(1, args.top_hashes),
        disassembly_limit=max(1, args.disassembly_limit),
    )
    if args.json:
        print(json.dumps(results, indent=2))
        return
    _print_table(results)
    if args.details:
        _print_details(results)


if __name__ == "__main__":
    main()
