from __future__ import annotations

import argparse
import cProfile
import io
from pathlib import Path
import pstats
import statistics
import sys
from time import perf_counter

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "src"))

from proteus.engine import ENGINE_BACKEND_NAME, SimulationSession
from cases import build_case


def run_benchmark(case_name: str, steps: int, repeat: int) -> dict[str, float]:
    samples: list[float] = []
    final_summary: dict | None = None
    for _ in range(repeat):
        config, _ = build_case(case_name)
        session = SimulationSession.from_config(config)
        start = perf_counter()
        session.advance(steps)
        elapsed = perf_counter() - start
        samples.append(steps / elapsed)
        final_summary = session.summary()
    assert final_summary is not None
    return {
        "mean_tps": statistics.mean(samples),
        "median_tps": statistics.median(samples),
        "min_tps": min(samples),
        "max_tps": max(samples),
        "summary": final_summary,
    }


def run_profile(case_name: str, steps: int, top: int) -> str:
    config, _ = build_case(case_name)
    session = SimulationSession.from_config(config)
    profiler = cProfile.Profile()
    profiler.enable()
    session.advance(steps)
    profiler.disable()
    stream = io.StringIO()
    pstats.Stats(profiler, stream=stream).sort_stats("cumtime").print_stats(top)
    return stream.getvalue()


def main() -> None:
    parser = argparse.ArgumentParser(description="Benchmark Proteus simulation throughput.")
    parser.add_argument("--case", choices=["default", "empty64"], default="default")
    parser.add_argument("--steps", type=int, default=None, help="Override the default step count for the chosen case.")
    parser.add_argument("--repeat", type=int, default=3, help="How many benchmark runs to average.")
    parser.add_argument("--profile", action="store_true", help="Print a cProfile report for one run.")
    parser.add_argument("--profile-top", type=int, default=20, help="Number of profile entries to print.")
    args = parser.parse_args()

    _, default_steps = build_case(args.case)
    steps = args.steps if args.steps is not None else default_steps
    results = run_benchmark(args.case, steps, args.repeat)
    print(f"case={args.case} steps={steps} repeat={args.repeat} backend={ENGINE_BACKEND_NAME}")
    print(
        "tps:"
        f" mean={results['mean_tps']:.2f}"
        f" median={results['median_tps']:.2f}"
        f" min={results['min_tps']:.2f}"
        f" max={results['max_tps']:.2f}"
    )
    summary = results["summary"]
    print(
        "summary:"
        f" tick={summary['tick']}"
        f" occupied={summary['occupied_cells']}"
        f" live={summary['live_programs']}"
        f" inert={summary['inert_programs']}"
        f" instructions={summary['total_instructions']}"
    )

    if args.profile:
        print()
        print(run_profile(args.case, min(steps, 200), args.profile_top))


if __name__ == "__main__":
    main()
