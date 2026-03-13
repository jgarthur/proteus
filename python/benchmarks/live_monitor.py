from __future__ import annotations

import argparse
from copy import deepcopy
import json
from pathlib import Path
import sys
import time
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "src"))

from cases import build_case
from proteus.analysis import time_series_point
from proteus.engine import SimulationSession, compute_program_hash
from proteus.models import SimulationConfig, to_builtin


def _apply_system_param_override(config: SimulationConfig, override: str) -> None:
    if "=" not in override:
        raise ValueError(f"Overrides must use NAME=VALUE, got {override!r}.")
    name, raw_value = override.split("=", 1)
    if not hasattr(config.system_params, name):
        raise ValueError(f"Unknown system parameter: {name}")
    current = getattr(config.system_params, name)
    if isinstance(current, bool):
        parsed = raw_value.strip().lower() in {"1", "true", "yes", "on"}
    elif isinstance(current, int) and not isinstance(current, bool):
        parsed = int(float(raw_value))
    else:
        parsed = float(raw_value)
    setattr(config.system_params, name, parsed)


def _print_header() -> None:
    print(
        "tick".rjust(7),
        "elapsed".rjust(9),
        "dt".rjust(7),
        "tps".rjust(9),
        "occ".rjust(6),
        "live".rjust(6),
        "inert".rjust(6),
        "instr".rjust(7),
        "uniq".rjust(6),
        "build".rjust(8),
        "repl".rjust(8),
        "abs1".rjust(8),
        "absdom".rjust(8),
        "effmed".rjust(8),
        "iabnd".rjust(7),
        "bact".rjust(7),
        "babn".rjust(7),
        "iprm".rjust(7),
        "iwait".rjust(7),
    )


def _print_point(point: dict[str, Any], *, elapsed_seconds: float, interval_seconds: float, interval_tps: float) -> None:
    print(
        str(point["tick"]).rjust(7),
        f"{elapsed_seconds:8.1f}s".rjust(9),
        f"{interval_seconds:6.2f}s".rjust(7),
        f"{interval_tps:8.1f}".rjust(9),
        str(point["occupied_cells"]).rjust(6),
        str(point["live_programs"]).rjust(6),
        str(point["inert_programs"]).rjust(6),
        str(point["total_instructions"]).rjust(7),
        str(point["unique_hashes"]).rjust(6),
        f"{point['builder_share']:.3f}".rjust(8),
        f"{point['replicator_share']:.3f}".rjust(8),
        f"{point['absorb_only_share']:.3f}".rjust(8),
        f"{point['absorb_dominated_share']:.3f}".rjust(8),
        f"{point['effective_median']:.1f}".rjust(8),
        str(point["counters"].get("inert_abandonments", 0)).rjust(7),
        str(point["counters"].get("boot_successes_active_construction", 0)).rjust(7),
        str(point["counters"].get("boot_successes_abandoned", 0)).rjust(7),
        str(point["counters"].get("inert_removed_preboot", 0)).rjust(7),
        str(point["max_inert_ticks_without_write"]).rjust(7),
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Run a Proteus simulation and print live ecology metrics over time.")
    parser.add_argument("--case", choices=["default", "empty64"], default="default")
    parser.add_argument("--ticks", type=int, default=50000)
    parser.add_argument("--sample-every", type=int, default=1000)
    parser.add_argument("--rng-seed", type=int, default=None)
    parser.add_argument("--width", type=int, default=None)
    parser.add_argument("--height", type=int, default=None)
    parser.add_argument("--seed-count", type=int, default=None)
    parser.add_argument(
        "--set",
        dest="overrides",
        action="append",
        default=[],
        help="Override a system parameter using NAME=VALUE. Can be repeated.",
    )
    parser.add_argument("--json", action="store_true", help="Print the collected series as JSON at the end.")
    parser.add_argument("--output", type=Path, default=None, help="Optional path to write the JSON payload.")
    parser.add_argument("--no-tick-zero", action="store_true", help="Skip the initial tick-0 sample.")
    args = parser.parse_args()

    config, _ = build_case(args.case)
    config = deepcopy(config)
    if args.rng_seed is not None:
        config.rng_seed = args.rng_seed
    if args.width is not None:
        config.width = args.width
    if args.height is not None:
        config.height = args.height
    if args.seed_count is not None:
        if not config.seeds:
            raise ValueError("This case has no seeds to resize.")
        config.seeds[0].count = args.seed_count
    for override in args.overrides:
        _apply_system_param_override(config, override)

    session = SimulationSession.from_config(config)
    first_program = next(iter(session.world.programs.values()), None)
    original_seed_hash = compute_program_hash(first_program.instructions) if first_program else None

    series: list[dict[str, Any]] = []
    start = time.perf_counter()
    previous_wall = start
    previous_tick = session.tick

    _print_header()
    if not args.no_tick_zero:
        point = time_series_point(session, original_seed_hash=original_seed_hash)
        point["elapsed_seconds"] = 0.0
        point["interval_seconds"] = 0.0
        point["interval_tps"] = 0.0
        series.append(point)
        _print_point(point, elapsed_seconds=0.0, interval_seconds=0.0, interval_tps=0.0)

    total_ticks = max(0, int(args.ticks))
    sample_every = max(1, int(args.sample_every))
    while session.tick < total_ticks:
        session.advance(min(sample_every, total_ticks - session.tick))
        now = time.perf_counter()
        elapsed = now - start
        interval = now - previous_wall
        interval_tick_delta = session.tick - previous_tick
        interval_tps = (interval_tick_delta / interval) if interval > 0 else 0.0
        point = time_series_point(session, original_seed_hash=original_seed_hash)
        point["elapsed_seconds"] = elapsed
        point["interval_seconds"] = interval
        point["interval_tps"] = interval_tps
        series.append(point)
        _print_point(point, elapsed_seconds=elapsed, interval_seconds=interval, interval_tps=interval_tps)
        previous_wall = now
        previous_tick = session.tick

    payload = {
        "engine_backend": session.summary()["engine_backend"],
        "config": {
            "width": config.width,
            "height": config.height,
            "rng_seed": config.rng_seed,
            "system_params": to_builtin(config.system_params),
            "seeds": to_builtin(config.seeds),
            "original_seed_hash": original_seed_hash,
        },
        "series": series,
    }
    if args.output is not None:
        args.output.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    if args.json:
        print(json.dumps(payload, indent=2))


if __name__ == "__main__":
    main()
