from __future__ import annotations

from collections import Counter, defaultdict
from math import log2
from typing import Any, Iterable, Sequence

import numpy as np

from proteus.engine import ENGINE_BACKEND_NAME, SimulationSession, compute_program_hash
from proteus.models import ProgramState, SimulationConfig, to_builtin
from proteus.spec import OPCODE_IS_NOOP, OPCODE_NAME, PUSH_LITERAL, disassemble_program

BUILDER_MNEMONICS = {
    "read",
    "readadj",
    "write",
    "writeadj",
    "appendadj",
    "del",
    "deladj",
    "boot",
    "synthesize",
    "givee",
    "givem",
}
REPLICATOR_COPY_MNEMONICS = {"read", "readadj"}
ABSORB_DOMINANCE_THRESHOLD = 0.75


def _opcode_token(raw_opcode: int) -> str:
    opcode = int(raw_opcode) & 0xFF
    if 0 <= opcode <= 0x0F:
        return f"push:{PUSH_LITERAL[opcode]}"
    if OPCODE_IS_NOOP[opcode]:
        return "noop"
    return OPCODE_NAME[opcode].lower()


def _collapsed_token_count(tokens: Sequence[str]) -> int:
    if not tokens:
        return 0
    collapsed = 1
    previous = tokens[0]
    for token in tokens[1:]:
        if token != previous:
            collapsed += 1
            previous = token
    return collapsed


def _program_structure(program: ProgramState) -> dict[str, Any]:
    tokens = [_opcode_token(opcode) for opcode in program.instructions]
    counts = Counter(tokens)
    total = len(tokens)
    absorb_count = counts["absorb"]
    entropy = 0.0
    concentration = 0.0
    if total > 0:
        probabilities = [count / total for count in counts.values()]
        entropy = -sum(probability * log2(probability) for probability in probabilities if probability > 0)
        concentration = max(probabilities)
    token_set = set(tokens)
    has_builder = bool(token_set & BUILDER_MNEMONICS)
    has_replicator = (
        "appendadj" in token_set
        and "boot" in token_set
        and bool(token_set & REPLICATOR_COPY_MNEMONICS)
    )
    return {
        "effective_size": _collapsed_token_count(tokens),
        "opcode_entropy": entropy,
        "opcode_concentration": concentration,
        "absorb_share": (absorb_count / total) if total > 0 else 0.0,
        "absorb_only": total > 0 and absorb_count == total,
        "absorb_dominated": total > 0 and (absorb_count / total) >= ABSORB_DOMINANCE_THRESHOLD,
        "nop_only": total > 0 and counts["nop"] == total,
        "builder": has_builder,
        "replicator": has_replicator,
    }


def _size_bucket_counts(sizes: Sequence[int]) -> dict[str, int]:
    return {
        "size_1": int(sum(1 for size in sizes if size == 1)),
        "size_2_to_4": int(sum(1 for size in sizes if 2 <= size <= 4)),
        "size_5_to_7": int(sum(1 for size in sizes if 5 <= size <= 7)),
        "size_8_plus": int(sum(1 for size in sizes if size >= 8)),
        "size_16_plus": int(sum(1 for size in sizes if size >= 16)),
    }


def _size_bucket_shares(size_counts: dict[str, int], total: int) -> dict[str, float]:
    if total <= 0:
        return {name: 0.0 for name in size_counts}
    return {name: count / total for name, count in size_counts.items()}


def _quantiles(values: Sequence[int]) -> dict[str, float | int | None]:
    if not values:
        return {
            "min": None,
            "p25": None,
            "median": None,
            "p75": None,
            "p90": None,
            "max": None,
            "mean": None,
        }
    array = np.array(values, dtype=np.int64)
    return {
        "min": int(array.min()),
        "p25": float(np.percentile(array, 25)),
        "median": float(np.percentile(array, 50)),
        "p75": float(np.percentile(array, 75)),
        "p90": float(np.percentile(array, 90)),
        "max": int(array.max()),
        "mean": float(array.mean()),
    }


def _top_cell(session: SimulationSession, field: str) -> dict[str, Any]:
    array = getattr(session.world, field)
    y, x = np.unravel_index(np.argmax(array), array.shape)
    program = session.world.get_program(int(x), int(y))
    return {
        "x": int(x),
        "y": int(y),
        "value": int(array[y, x]),
        "occupied": program is not None,
        "live": bool(program.live) if program else False,
        "size": len(program.instructions) if program else 0,
        "hash": compute_program_hash(program.instructions) if program else None,
    }


def _describe_program(
    session: SimulationSession,
    program: ProgramState | None,
    *,
    disassembly_limit: int,
) -> dict[str, Any] | None:
    if program is None:
        return None
    return {
        "x": program.x,
        "y": program.y,
        "live": program.live,
        "age": program.age,
        "size": len(program.instructions),
        "hash": compute_program_hash(program.instructions),
        "ip": program.ip,
        "free_energy": int(session.world.free_energy[program.y, program.x]),
        "free_mass": int(session.world.free_mass[program.y, program.x]),
        "background_radiation": int(session.world.bg_radiation[program.y, program.x]),
        "disassembly": disassemble_program(program.instructions)[:disassembly_limit],
    }


def snapshot_session(
    session: SimulationSession,
    *,
    original_seed_hash: str | None = None,
    top_hash_limit: int = 5,
    disassembly_limit: int = 16,
) -> dict[str, Any]:
    world = session.world
    summary = session.summary()
    occupied_mask = world.program_ids >= 0
    empty_mask = ~occupied_mask
    programs = list(world.programs.values())
    live_programs = [program for program in programs if program.live]
    inert_programs = [program for program in programs if not program.live]
    sizes = [len(program.instructions) for program in programs]
    live_sizes = [len(program.instructions) for program in live_programs]
    inert_sizes = [len(program.instructions) for program in inert_programs]
    inert_wait_ticks = [program.inert_ticks_without_write for program in inert_programs]
    grace_ticks = int(session.config.system_params.inert_grace_ticks)
    inert_abandoned_programs = [
        program for program in inert_programs
        if grace_ticks <= 0 or program.inert_ticks_without_write >= grace_ticks
    ]
    ages = [program.age for program in programs]
    structures = {program.program_id: _program_structure(program) for program in programs}
    effective_sizes = [int(structures[program.program_id]["effective_size"]) for program in programs]
    live_effective_sizes = [int(structures[program.program_id]["effective_size"]) for program in live_programs]
    live_entropies = [float(structures[program.program_id]["opcode_entropy"]) for program in live_programs]
    live_concentrations = [float(structures[program.program_id]["opcode_concentration"]) for program in live_programs]
    hash_counts = Counter(compute_program_hash(program.instructions) for program in programs)
    hash_to_programs: dict[str, list[ProgramState]] = defaultdict(list)
    for program in programs:
        hash_to_programs[compute_program_hash(program.instructions)].append(program)
    size_bucket_counts = _size_bucket_counts(sizes)
    dominant_hash_count = max(hash_counts.values(), default=0)
    live_total = len(live_programs)
    live_builder_count = sum(1 for program in live_programs if structures[program.program_id]["builder"])
    live_replicator_count = sum(1 for program in live_programs if structures[program.program_id]["replicator"])
    live_absorb_only_count = sum(1 for program in live_programs if structures[program.program_id]["absorb_only"])
    live_absorb_dominated_count = sum(
        1 for program in live_programs if structures[program.program_id]["absorb_dominated"]
    )
    live_nop_only_count = sum(1 for program in live_programs if structures[program.program_id]["nop_only"])

    top_hashes: list[dict[str, Any]] = []
    for program_hash, count in hash_counts.most_common(top_hash_limit):
        cohort = hash_to_programs[program_hash]
        example = cohort[0]
        cohort_sizes = [len(program.instructions) for program in cohort]
        top_hashes.append(
            {
                "hash": program_hash,
                "count": int(count),
                "live": int(sum(1 for program in cohort if program.live)),
                "inert": int(sum(1 for program in cohort if not program.live)),
                "size_mean": float(np.mean(cohort_sizes)),
                "size_range": [int(min(cohort_sizes)), int(max(cohort_sizes))],
                "age_max": int(max(program.age for program in cohort)),
                "example_ip": int(example.ip),
                "example_disassembly": disassemble_program(example.instructions)[:disassembly_limit],
            }
        )

    nonzero_empty_energy = int(np.count_nonzero(world.free_energy[empty_mask]))
    nonzero_empty_mass = int(np.count_nonzero(world.free_mass[empty_mask]))
    nonzero_empty_background = int(np.count_nonzero(world.bg_radiation[empty_mask]))

    largest_program = max(programs, key=lambda program: len(program.instructions), default=None)
    energy_rich_program = max(
        programs,
        key=lambda program: int(world.free_energy[program.y, program.x]),
        default=None,
    )
    mass_rich_program = max(
        programs,
        key=lambda program: int(world.free_mass[program.y, program.x]),
        default=None,
    )

    return {
        "tick": summary["tick"],
        "summary": summary,
        "population": {
            "program_count": len(programs),
            "live_count": len(live_programs),
            "inert_count": len(inert_programs),
            "unique_hashes": len(hash_counts),
            "singletons": int(sum(1 for count in hash_counts.values() if count == 1)),
            "original_seed_hash_count": int(hash_counts[original_seed_hash]) if original_seed_hash else None,
            "size_stats": _quantiles(sizes),
            "effective_size_stats": _quantiles(effective_sizes),
            "size_bucket_counts": size_bucket_counts,
            "size_bucket_shares": _size_bucket_shares(size_bucket_counts, len(programs)),
            "live_size_stats": _quantiles(live_sizes),
            "live_effective_size_stats": _quantiles(live_effective_sizes),
            "inert_size_stats": _quantiles(inert_sizes),
            "inert_wait_ticks_stats": _quantiles(inert_wait_ticks),
            "inert_abandoned_count": len(inert_abandoned_programs),
            "age_stats": _quantiles(ages),
            "dominant_hash_share": (dominant_hash_count / len(programs)) if programs else 0.0,
            "structure": {
                "live_builder_count": int(live_builder_count),
                "live_builder_share": (live_builder_count / live_total) if live_total else 0.0,
                "live_replicator_count": int(live_replicator_count),
                "live_replicator_share": (live_replicator_count / live_total) if live_total else 0.0,
                "live_absorb_only_count": int(live_absorb_only_count),
                "live_absorb_only_share": (live_absorb_only_count / live_total) if live_total else 0.0,
                "live_absorb_dominated_count": int(live_absorb_dominated_count),
                "live_absorb_dominated_share": (live_absorb_dominated_count / live_total) if live_total else 0.0,
                "live_nop_only_count": int(live_nop_only_count),
                "live_nop_only_share": (live_nop_only_count / live_total) if live_total else 0.0,
                "live_opcode_entropy_mean": float(np.mean(live_entropies)) if live_entropies else 0.0,
                "live_opcode_concentration_mean": float(np.mean(live_concentrations)) if live_concentrations else 0.0,
            },
            "top_hashes": top_hashes,
        },
        "counters": dict(summary.get("counters", {})),
        "resources": {
            "occupied_free_energy": int(world.free_energy[occupied_mask].sum()) if occupied_mask.any() else 0,
            "empty_free_energy": int(world.free_energy[empty_mask].sum()) if empty_mask.any() else 0,
            "occupied_free_mass": int(world.free_mass[occupied_mask].sum()) if occupied_mask.any() else 0,
            "empty_free_mass": int(world.free_mass[empty_mask].sum()) if empty_mask.any() else 0,
            "occupied_background": int(world.bg_radiation[occupied_mask].sum()) if occupied_mask.any() else 0,
            "empty_background": int(world.bg_radiation[empty_mask].sum()) if empty_mask.any() else 0,
            "nonzero_empty_energy_cells": nonzero_empty_energy,
            "nonzero_empty_mass_cells": nonzero_empty_mass,
            "nonzero_empty_background_cells": nonzero_empty_background,
            "max_free_energy_cell": _top_cell(session, "free_energy"),
            "max_free_mass_cell": _top_cell(session, "free_mass"),
            "max_background_cell": _top_cell(session, "bg_radiation"),
        },
        "outliers": {
            "largest_program": _describe_program(
                session,
                largest_program,
                disassembly_limit=disassembly_limit,
            ),
            "energy_rich_program": _describe_program(
                session,
                energy_rich_program,
                disassembly_limit=disassembly_limit,
            ),
            "mass_rich_program": _describe_program(
                session,
                mass_rich_program,
                disassembly_limit=disassembly_limit,
            ),
        },
    }


def time_series_point(
    session: SimulationSession,
    *,
    original_seed_hash: str | None = None,
) -> dict[str, Any]:
    world = session.world
    summary = session.summary()
    programs = list(world.programs.values())
    live_programs = [program for program in programs if program.live]
    inert_programs = [program for program in programs if not program.live]
    grace_ticks = int(session.config.system_params.inert_grace_ticks)
    structures = {program.program_id: _program_structure(program) for program in programs}
    live_total = len(live_programs)
    hash_counts = Counter(compute_program_hash(program.instructions) for program in programs)
    size_bucket_counts = _size_bucket_counts([len(program.instructions) for program in programs])
    effective_median = float(
        np.percentile(
            [int(structures[program.program_id]["effective_size"]) for program in live_programs],
            50,
        )
    ) if live_programs else 0.0
    live_builder_count = sum(1 for program in live_programs if structures[program.program_id]["builder"])
    live_replicator_count = sum(1 for program in live_programs if structures[program.program_id]["replicator"])
    live_absorb_only_count = sum(1 for program in live_programs if structures[program.program_id]["absorb_only"])
    live_absorb_dominated_count = sum(
        1 for program in live_programs if structures[program.program_id]["absorb_dominated"]
    )
    live_nop_only_count = sum(1 for program in live_programs if structures[program.program_id]["nop_only"])
    return {
        "tick": summary["tick"],
        "engine_backend": ENGINE_BACKEND_NAME,
        "occupied_cells": summary["occupied_cells"],
        "live_programs": summary["live_programs"],
        "inert_programs": summary["inert_programs"],
        "total_instructions": summary["total_instructions"],
        "total_free_energy": summary["total_free_energy"],
        "total_free_mass": summary["total_free_mass"],
        "total_background_radiation": summary["total_background_radiation"],
        "unique_hashes": len(hash_counts),
        "original_seed_hash_count": int(hash_counts[original_seed_hash]) if original_seed_hash else None,
        "size_1_share": (size_bucket_counts["size_1"] / len(programs)) if programs else 0.0,
        "size_8_plus_share": (size_bucket_counts["size_8_plus"] / len(programs)) if programs else 0.0,
        "effective_median": effective_median,
        "dominant_hash_share": (max(hash_counts.values()) / len(programs)) if programs else 0.0,
        "builder_share": (live_builder_count / live_total) if live_total else 0.0,
        "replicator_share": (live_replicator_count / live_total) if live_total else 0.0,
        "absorb_only_share": (live_absorb_only_count / live_total) if live_total else 0.0,
        "absorb_dominated_share": (live_absorb_dominated_count / live_total) if live_total else 0.0,
        "nop_only_share": (live_nop_only_count / live_total) if live_total else 0.0,
        "max_inert_ticks_without_write": max((program.inert_ticks_without_write for program in inert_programs), default=0),
        "inert_waiting_programs": int(sum(1 for program in inert_programs if program.inert_ticks_without_write > 0)),
        "abandoned_inert_programs": int(
            sum(1 for program in inert_programs if grace_ticks <= 0 or program.inert_ticks_without_write >= grace_ticks)
        ),
        "inert_grace_ticks": grace_ticks,
        "counters": dict(summary.get("counters", {})),
    }


def sample_time_series(
    config: SimulationConfig,
    *,
    total_ticks: int,
    sample_every: int,
    include_tick_zero: bool = True,
) -> dict[str, Any]:
    session = SimulationSession.from_config(config)
    first_program = next(iter(session.world.programs.values()), None)
    original_seed_hash = compute_program_hash(first_program.instructions) if first_program else None
    total_ticks = max(0, int(total_ticks))
    sample_every = max(1, int(sample_every))
    points: list[dict[str, Any]] = []
    if include_tick_zero:
        points.append(time_series_point(session, original_seed_hash=original_seed_hash))
    while session.tick < total_ticks:
        session.advance(min(sample_every, total_ticks - session.tick))
        points.append(time_series_point(session, original_seed_hash=original_seed_hash))
    return {
        "engine_backend": ENGINE_BACKEND_NAME,
        "config": {
            "width": config.width,
            "height": config.height,
            "rng_seed": config.rng_seed,
            "system_params": to_builtin(config.system_params),
            "seeds": to_builtin(config.seeds),
            "original_seed_hash": original_seed_hash,
        },
        "series": points,
    }


def evaluate_milestones(
    config: SimulationConfig,
    ticks: Iterable[int],
    *,
    top_hash_limit: int = 5,
    disassembly_limit: int = 16,
) -> dict[str, Any]:
    session = SimulationSession.from_config(config)
    first_program = next(iter(session.world.programs.values()), None)
    original_seed_hash = compute_program_hash(first_program.instructions) if first_program else None
    original_seed_disassembly = disassemble_program(first_program.instructions) if first_program else None
    milestones = sorted({max(0, int(tick)) for tick in ticks})

    results = {
        "engine_backend": ENGINE_BACKEND_NAME,
        "config": {
            "width": config.width,
            "height": config.height,
            "rng_seed": config.rng_seed,
            "system_params": to_builtin(config.system_params),
            "seeds": to_builtin(config.seeds),
            "original_seed_hash": original_seed_hash,
            "original_seed_disassembly": original_seed_disassembly,
        },
        "snapshots": [],
    }

    for target_tick in milestones:
        if session.tick < target_tick:
            session.advance(target_tick - session.tick)
        results["snapshots"].append(
            snapshot_session(
                session,
                original_seed_hash=original_seed_hash,
                top_hash_limit=top_hash_limit,
                disassembly_limit=disassembly_limit,
            )
        )

    return results
