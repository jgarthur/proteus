from __future__ import annotations

from dataclasses import dataclass
import hashlib
from typing import Any

import numpy as np

from proteus.models import (
    NonlocalAction,
    ProgramState,
    RadiationPacket,
    SeedSpec,
    SimulationConfig,
    SystemParams,
    clamp_nonnegative,
    to_builtin,
    wrap8,
    wrap16,
)
from proteus.random_service import RandomService
from proteus.seed_presets import SEED_PRESETS
from proteus.spec import (
    IMMEDIATE_KIND,
    LOCAL_KIND,
    NONLOCAL_KIND,
    OPCODE_BASE_COST,
    OPCODE_IS_NOOP,
    OPCODE_KIND,
    OPCODE_NAME,
    PROTECTION_REQUIRED,
    PUSH_LITERAL,
    assemble_program,
    decode_instruction,
    disassemble_program,
)

NOP_OPCODE = 0x50
CANONICAL_NOOP_HASH_OPCODE = 0xFF
ENGINE_BACKEND_NAME = "python"
DEFAULT_COUNTERS = {
    "append_adj_attempts": 0,
    "append_adj_successes": 0,
    "boot_attempts": 0,
    "boot_successes": 0,
    "boot_successes_active_construction": 0,
    "boot_successes_abandoned": 0,
    "inert_created": 0,
    "inert_write_events": 0,
    "inert_abandonments": 0,
    "inert_removed_preboot": 0,
    "inert_removed_while_abandoned": 0,
    "move_attempts": 0,
    "move_successes": 0,
}


def _normalize_program_for_hash_python(instructions: list[int], canonical_noop_opcode: int) -> bytes:
    normalized = []
    for value in instructions:
        opcode = int(value) & 0xFF
        normalized.append(canonical_noop_opcode if OPCODE_IS_NOOP[opcode] else opcode)
    return bytes(normalized)


def _scan_forward_opcode_python(instructions: list[int], start_ip: int, target_opcode: int) -> int:
    size = len(instructions)
    index = (start_ip + 1) % size
    for _ in range(size):
        if (int(instructions[index]) & 0xFF) == target_opcode:
            return index
        index = (index + 1) % size
    return -1


def _scan_backward_opcode_python(instructions: list[int], start_ip: int, target_opcode: int) -> int:
    size = len(instructions)
    index = (start_ip - 1) % size
    for _ in range(size):
        if (int(instructions[index]) & 0xFF) == target_opcode:
            return index
        index = (index - 1) % size
    return -1


try:
    from proteus._engine_cython_ops import (
        normalize_program_for_hash as _normalize_program_for_hash,
        scan_backward_opcode as _scan_backward_opcode,
        scan_forward_opcode as _scan_forward_opcode,
    )

    ENGINE_BACKEND_NAME = "cython-ops"
except ImportError:
    _normalize_program_for_hash = _normalize_program_for_hash_python
    _scan_forward_opcode = _scan_forward_opcode_python
    _scan_backward_opcode = _scan_backward_opcode_python


def compute_program_hash(instructions: list[int]) -> str:
    return hashlib.blake2b(
        _normalize_program_for_hash(instructions, CANONICAL_NOOP_HASH_OPCODE),
        digest_size=6,
    ).hexdigest()


def _new_counters() -> dict[str, int]:
    return dict(DEFAULT_COUNTERS)


def _restore_counters(payload: dict[str, Any] | None) -> dict[str, int]:
    counters = _new_counters()
    if payload is None:
        return counters
    for key in counters:
        if key in payload:
            counters[key] = int(payload[key])
    return counters


@dataclass(slots=True)
class WorldState:
    width: int
    height: int
    free_energy: np.ndarray
    free_mass: np.ndarray
    bg_radiation: np.ndarray
    program_ids: np.ndarray
    programs: dict[int, ProgramState]
    radiation: dict[tuple[int, int], list[RadiationPacket]]
    open_mask: np.ndarray
    next_program_id: int = 1

    @classmethod
    def empty(cls, width: int, height: int) -> WorldState:
        shape = (height, width)
        return cls(
            width=width,
            height=height,
            free_energy=np.zeros(shape, dtype=np.int64),
            free_mass=np.zeros(shape, dtype=np.int64),
            bg_radiation=np.zeros(shape, dtype=np.int64),
            program_ids=np.full(shape, -1, dtype=np.int64),
            programs={},
            radiation={},
            open_mask=np.ones(shape, dtype=np.bool_),
            next_program_id=1,
        )

    def clone(self) -> WorldState:
        return WorldState(
            width=self.width,
            height=self.height,
            free_energy=self.free_energy.copy(),
            free_mass=self.free_mass.copy(),
            bg_radiation=self.bg_radiation.copy(),
            program_ids=self.program_ids.copy(),
            programs={program_id: program.clone() for program_id, program in self.programs.items()},
            radiation={
                position: [packet.clone() for packet in packets]
                for position, packets in self.radiation.items()
            },
            open_mask=self.open_mask.copy(),
            next_program_id=self.next_program_id,
        )

    def wrap(self, x: int, y: int) -> tuple[int, int]:
        return x % self.width, y % self.height

    def neighbor(self, x: int, y: int, direction: int) -> tuple[int, int]:
        if direction % 4 == 0:
            return self.wrap(x + 1, y)
        if direction % 4 == 1:
            return self.wrap(x, y - 1)
        if direction % 4 == 2:
            return self.wrap(x - 1, y)
        return self.wrap(x, y + 1)

    def plus_neighbors(self, x: int, y: int) -> list[tuple[int, int]]:
        return [
            self.wrap(x, y),
            self.neighbor(x, y, 0),
            self.neighbor(x, y, 1),
            self.neighbor(x, y, 2),
            self.neighbor(x, y, 3),
        ]

    def get_program(self, x: int, y: int) -> ProgramState | None:
        program_id = int(self.program_ids[y, x])
        if program_id < 0:
            return None
        return self.programs.get(program_id)

    def place_program(self, program: ProgramState) -> None:
        self.programs[program.program_id] = program
        self.program_ids[program.y, program.x] = program.program_id
        self.next_program_id = max(self.next_program_id, program.program_id + 1)

    def remove_program(self, program_id: int) -> None:
        program = self.programs.pop(program_id, None)
        if program is None:
            return
        self.program_ids[program.y, program.x] = -1

    def new_program(
        self,
        x: int,
        y: int,
        instructions: list[int],
        rng: RandomService,
        *,
        live: bool,
        initial_dir: int | None = None,
        initial_id: int | None = None,
        birth_kind: str = "seed",
        created_tick: int = 0,
        creator_program_id: int | None = None,
        creator_hash: str | None = None,
    ) -> ProgramState:
        x, y = self.wrap(x, y)
        program = ProgramState(
            program_id=self.next_program_id,
            x=x,
            y=y,
            instructions=list(instructions),
            dir=(initial_dir if initial_dir is not None else rng.integers(0, 4)) % 4,
            id_reg=wrap8(initial_id if initial_id is not None else rng.integers(0, 256)),
            live=bool(live),
            age=0,
            birth_kind=birth_kind,
            created_tick=int(created_tick),
            creator_program_id=creator_program_id,
            creator_hash=creator_hash,
        )
        self.place_program(program)
        return program

    def recompute_open_mask(self) -> None:
        self.open_mask[:] = self.program_ids < 0
        for program in self.programs.values():
            if not program.live:
                self.open_mask[program.y, program.x] = True

    def strength_at(self, x: int, y: int) -> int:
        program = self.get_program(x, y)
        if program is None:
            return 0
        return min(len(program.instructions), int(self.free_energy[y, x]))

    def serialize(self) -> dict[str, Any]:
        return {
            "width": self.width,
            "height": self.height,
            "free_energy": to_builtin(self.free_energy),
            "free_mass": to_builtin(self.free_mass),
            "bg_radiation": to_builtin(self.bg_radiation),
            "program_ids": to_builtin(self.program_ids),
            "open_mask": to_builtin(self.open_mask.astype(np.int8)),
            "next_program_id": self.next_program_id,
            "programs": [to_builtin(program) for program in self.programs.values()],
            "radiation": [
                {
                    "x": x,
                    "y": y,
                    "packets": [to_builtin(packet) for packet in packets],
                }
                for (x, y), packets in self.radiation.items()
            ],
        }

    @classmethod
    def from_archive(cls, payload: dict[str, Any]) -> WorldState:
        width = int(payload["width"])
        height = int(payload["height"])
        world = cls(
            width=width,
            height=height,
            free_energy=np.array(payload["free_energy"], dtype=np.int64),
            free_mass=np.array(payload["free_mass"], dtype=np.int64),
            bg_radiation=np.array(payload["bg_radiation"], dtype=np.int64),
            program_ids=np.array(payload["program_ids"], dtype=np.int64),
            programs={},
            radiation={},
            open_mask=np.array(payload["open_mask"], dtype=np.bool_),
            next_program_id=int(payload["next_program_id"]),
        )
        for raw_program in payload["programs"]:
            program = ProgramState(
                program_id=int(raw_program["program_id"]),
                x=int(raw_program["x"]),
                y=int(raw_program["y"]),
                instructions=[int(value) for value in raw_program["instructions"]],
                ip=int(raw_program["ip"]),
                dir=int(raw_program["dir"]),
                src=int(raw_program["src"]),
                dst=int(raw_program["dst"]),
                flag=int(raw_program["flag"]),
                msg=int(raw_program["msg"]),
                id_reg=int(raw_program["id_reg"]),
                lc=int(raw_program["lc"]),
                stack=[int(value) for value in raw_program.get("stack", [])],
                live=bool(raw_program["live"]),
                age=int(raw_program["age"]),
                inert_ticks_without_write=int(raw_program.get("inert_ticks_without_write", 0)),
                birth_kind=str(raw_program.get("birth_kind", "seed")),
                created_tick=int(raw_program.get("created_tick", 0)),
                creator_program_id=(
                    None if raw_program.get("creator_program_id") is None else int(raw_program["creator_program_id"])
                ),
                creator_hash=raw_program.get("creator_hash"),
                last_writer_program_id=(
                    None if raw_program.get("last_writer_program_id") is None else int(raw_program["last_writer_program_id"])
                ),
                last_writer_hash=raw_program.get("last_writer_hash"),
                writes_received=int(raw_program.get("writes_received", 0)),
                boot_tick=None if raw_program.get("boot_tick") is None else int(raw_program["boot_tick"]),
                boot_size=None if raw_program.get("boot_size") is None else int(raw_program["boot_size"]),
                boot_writes_received=(
                    None if raw_program.get("boot_writes_received") is None else int(raw_program["boot_writes_received"])
                ),
                booted_by_program_id=(
                    None if raw_program.get("booted_by_program_id") is None else int(raw_program["booted_by_program_id"])
                ),
                booted_by_hash=raw_program.get("booted_by_hash"),
                booted_while_under_grace=(
                    None
                    if raw_program.get("booted_while_under_grace") is None
                    else bool(raw_program["booted_while_under_grace"])
                ),
            )
            world.programs[program.program_id] = program
        for entry in payload.get("radiation", []):
            world.radiation[(int(entry["x"]), int(entry["y"]))] = [
                RadiationPacket(
                    x=int(packet["x"]),
                    y=int(packet["y"]),
                    direction=int(packet["direction"]),
                    value=int(packet["value"]),
                )
                for packet in entry.get("packets", [])
            ]
        return world


def system_params_from_dict(payload: dict[str, Any]) -> SystemParams:
    return SystemParams(
        R_energy=float(payload["R_energy"]),
        R_mass=float(payload["R_mass"]),
        P_spawn=float(payload["P_spawn"]),
        D_energy=float(payload["D_energy"]),
        D_mass=float(payload["D_mass"]),
        T_cap=int(payload["T_cap"]),
        M=float(payload["M"]),
        inert_grace_ticks=int(payload.get("inert_grace_ticks", payload.get("inert_auto_boot_ticks", 10))),
        N_synth=int(payload["N_synth"]),
        mutation_base_log2=int(payload.get("mutation_base_log2", 16)),
        mutation_background_log2=int(payload.get("mutation_background_log2", 8)),
    )


def seed_spec_from_dict(payload: dict[str, Any]) -> SeedSpec:
    return SeedSpec(
        assembly_source=str(payload["assembly_source"]),
        x=int(payload["x"]),
        y=int(payload["y"]),
        count=int(payload.get("count", 1)),
        preset_key=payload.get("preset_key"),
        randomize_additional_seeds=bool(payload.get("randomize_additional_seeds", False)),
        initial_dir=None if payload.get("initial_dir") is None else int(payload["initial_dir"]),
        initial_id=None if payload.get("initial_id") is None else int(payload["initial_id"]),
        initial_free_energy=int(payload.get("initial_free_energy", 20)),
        initial_free_mass=int(payload.get("initial_free_mass", 11)),
        neighbor_free_energy=int(payload.get("neighbor_free_energy", 20)),
        neighbor_free_mass=int(payload.get("neighbor_free_mass", 11)),
        live=bool(payload.get("live", True)),
    )


def simulation_config_from_dict(payload: dict[str, Any]) -> SimulationConfig:
    return SimulationConfig(
        width=int(payload["width"]),
        height=int(payload["height"]),
        rng_seed=int(payload["rng_seed"]),
        system_params=system_params_from_dict(payload["system_params"]),
        seeds=[seed_spec_from_dict(seed) for seed in payload.get("seeds", [])],
    )


class SimulationSession:
    def __init__(self, config: SimulationConfig, world: WorldState, rng: RandomService, tick: int = 0) -> None:
        self.config = config
        self.world = world
        self.rng = rng
        self.tick = int(tick)
        self.counters = _new_counters()
        self._initial_archive: dict[str, Any] | None = None

    @classmethod
    def from_config(cls, config: SimulationConfig) -> SimulationSession:
        config.validate()
        rng = RandomService(seed=config.rng_seed)
        world = WorldState.empty(config.width, config.height)
        session = cls(config=config, world=world, rng=rng, tick=0)
        reserved_positions = {world.wrap(seed.x, seed.y) for seed in config.seeds}
        for seed in config.seeds:
            positions = session._resolve_seed_positions(seed, reserved_positions)
            program_sources = session._resolve_seed_program_sources(seed)
            for (x, y), source in zip(positions, program_sources):
                bytecode = assemble_program(source)
                world.free_energy[y, x] += seed.initial_free_energy
                world.free_mass[y, x] += seed.initial_free_mass
                for nx, ny in (
                    world.neighbor(x, y, 0),
                    world.neighbor(x, y, 1),
                    world.neighbor(x, y, 2),
                    world.neighbor(x, y, 3),
                ):
                    world.free_energy[ny, nx] += seed.neighbor_free_energy
                    world.free_mass[ny, nx] += seed.neighbor_free_mass
                world.new_program(
                    x,
                    y,
                    bytecode,
                    rng,
                    live=seed.live,
                    initial_dir=seed.initial_dir,
                    initial_id=seed.initial_id,
                    birth_kind="seed",
                    created_tick=0,
                )
        world.recompute_open_mask()
        session._initial_archive = session.export_archive()
        return session

    def _resolve_seed_positions(
        self,
        seed: SeedSpec,
        reserved_positions: set[tuple[int, int]],
    ) -> list[tuple[int, int]]:
        positions: list[tuple[int, int]] = []
        anchor = self.world.wrap(seed.x, seed.y)
        if self.world.get_program(*anchor) is not None:
            raise ValueError("Seed anchor collides with an existing program.")
        positions.append(anchor)
        while len(positions) < seed.count:
            candidate = (
                self.rng.integers(0, self.world.width),
                self.rng.integers(0, self.world.height),
            )
            if candidate in positions:
                continue
            if candidate in reserved_positions:
                continue
            if self.world.get_program(*candidate) is not None:
                continue
            positions.append(candidate)
        return positions

    @classmethod
    def from_archive(cls, payload: dict[str, Any]) -> SimulationSession:
        config = simulation_config_from_dict(payload["config"])
        world = WorldState.from_archive(payload["world"])
        rng = RandomService(state=payload["rng_state"])
        session = cls(config=config, world=world, rng=rng, tick=int(payload["tick"]))
        session.counters = _restore_counters(payload.get("counters"))
        session._initial_archive = payload.get("reset_archive")
        if session._initial_archive is None:
            session._initial_archive = session.export_archive()
        return session

    def clone_reset_archive(self) -> dict[str, Any]:
        if self._initial_archive is None:
            self._initial_archive = self.export_archive()
        return self._initial_archive

    def _resolve_seed_program_sources(self, seed: SeedSpec) -> list[str]:
        if not seed.randomize_additional_seeds or seed.count <= 1:
            return [seed.assembly_source] * seed.count
        sources = [seed.assembly_source]
        preset_catalog = list(SEED_PRESETS)
        while len(sources) < seed.count:
            preset = preset_catalog[self.rng.integers(0, len(preset_catalog))]
            sources.append(preset.assembly_source)
        return sources

    def reset(self) -> None:
        archive = self.clone_reset_archive()
        restored = SimulationSession.from_archive(archive)
        self.config = restored.config
        self.world = restored.world
        self.rng = restored.rng
        self.tick = restored.tick
        self.counters = restored.counters
        self._initial_archive = archive

    def _increment_counter(self, name: str, amount: int = 1) -> None:
        self.counters[name] = int(self.counters.get(name, 0)) + int(amount)

    def _inert_is_under_grace(self, program: ProgramState) -> bool:
        grace_ticks = int(self.config.system_params.inert_grace_ticks)
        return (not program.live) and grace_ticks > 0 and program.inert_ticks_without_write < grace_ticks

    def _remove_program(self, program_id: int) -> None:
        program = self.world.programs.get(program_id)
        if program is not None and not program.live:
            self._increment_counter("inert_removed_preboot")
            if not self._inert_is_under_grace(program):
                self._increment_counter("inert_removed_while_abandoned")
        self.world.remove_program(program_id)

    def export_archive(self) -> dict[str, Any]:
        archive = {
            "config": to_builtin(self.config),
            "tick": self.tick,
            "rng_state": to_builtin(self.rng.state),
            "world": self.world.serialize(),
            "counters": to_builtin(self.counters),
        }
        if self._initial_archive is not None:
            archive["reset_archive"] = self._initial_archive
        return archive

    def summary(self) -> dict[str, Any]:
        occupied = int(np.count_nonzero(self.world.program_ids >= 0))
        live = sum(1 for program in self.world.programs.values() if program.live)
        inert = occupied - live
        total_instructions = sum(program.size for program in self.world.programs.values())
        return {
            "tick": self.tick,
            "engine_backend": ENGINE_BACKEND_NAME,
            "dimensions": {"width": self.world.width, "height": self.world.height},
            "occupied_cells": occupied,
            "live_programs": live,
            "inert_programs": inert,
            "total_instructions": total_instructions,
            "total_free_energy": int(self.world.free_energy.sum()),
            "total_free_mass": int(self.world.free_mass.sum()),
            "total_background_radiation": int(self.world.bg_radiation.sum()),
            "counters": to_builtin(self.counters),
        }

    def viewport(self, origin_x: int, origin_y: int, width: int, height: int, overlay: str = "occupancy") -> dict[str, Any]:
        cells: list[list[dict[str, Any]]] = []
        width = max(1, min(width, self.world.width))
        height = max(1, min(height, self.world.height))
        for row in range(height):
            line: list[dict[str, Any]] = []
            for column in range(width):
                x, y = self.world.wrap(origin_x + column, origin_y + row)
                program = self.world.get_program(x, y)
                line.append(
                    {
                        "x": x,
                        "y": y,
                        "occupied": program is not None,
                        "live": bool(program.live) if program else False,
                        "open": bool(self.world.open_mask[y, x]),
                        "free_energy": int(self.world.free_energy[y, x]),
                        "free_mass": int(self.world.free_mass[y, x]),
                        "background_radiation": int(self.world.bg_radiation[y, x]),
                        "size": program.size if program else 0,
                        "program_id": program.program_id if program else None,
                        "program_hash": compute_program_hash(program.instructions) if program else None,
                    }
                )
            cells.append(line)
        return {
            "tick": self.tick,
            "origin_x": origin_x % self.world.width,
            "origin_y": origin_y % self.world.height,
            "width": width,
            "height": height,
            "overlay": overlay,
            "cells": cells,
        }

    def cell_detail(self, x: int, y: int) -> dict[str, Any]:
        x, y = self.world.wrap(x, y)
        program = self.world.get_program(x, y)
        detail = {
            "x": x,
            "y": y,
            "open": bool(self.world.open_mask[y, x]),
            "free_energy": int(self.world.free_energy[y, x]),
            "free_mass": int(self.world.free_mass[y, x]),
            "background_radiation": int(self.world.bg_radiation[y, x]),
            "program": None,
        }
        if program is None:
            return detail
        detail["program"] = {
            "program_id": program.program_id,
            "live": program.live,
            "age": program.age,
            "size": program.size,
            "program_hash": compute_program_hash(program.instructions),
            "strength": self.world.strength_at(x, y),
            "registers": {
                "IP": program.ip,
                "Dir": program.dir,
                "Src": program.src,
                "Dst": program.dst,
                "Flag": program.flag,
                "Msg": program.msg,
                "ID": program.id_reg,
                "LC": program.lc,
            },
            "inert_ticks_without_write": program.inert_ticks_without_write,
            "birth_kind": program.birth_kind,
            "writes_received": program.writes_received,
            "boot_tick": program.boot_tick,
            "boot_size": program.boot_size,
            "boot_writes_received": program.boot_writes_received,
            "stack": list(program.stack),
            "bytecode": list(program.instructions),
            "disassembly": disassemble_program(program.instructions),
        }
        return detail

    def advance(self, steps: int = 1) -> None:
        for _ in range(max(0, int(steps))):
            self._tick_once()

    def _run_pass1_for_program(
        self,
        program: ProgramState,
        absorbers: set[tuple[int, int]],
        nonlocal_actions: list[NonlocalAction],
    ) -> None:
        if not program.instructions:
            return
        immediate_limit = len(program.instructions)
        immediate_count = 0

        while True:
            if program.program_id not in self.world.programs or not program.instructions:
                return
            size = len(program.instructions)
            opcode = int(program.instructions[program.ip % size]) & 0xFF
            kind = OPCODE_KIND[opcode]
            if kind in (LOCAL_KIND, NONLOCAL_KIND):
                paid, used_background, background_amount = self._pay_base_cost(program, OPCODE_BASE_COST[opcode])
                if not paid:
                    program.flag = 1
                    self.world.open_mask[program.y, program.x] = True
                    return
                program.flag = 0
                executed_index = program.ip
                if kind == LOCAL_KIND:
                    self._execute_local(program, opcode, absorbers)
                else:
                    target_x, target_y = self.world.neighbor(program.x, program.y, program.dir)
                    mnemonic = OPCODE_NAME[opcode]
                    if mnemonic == "appendAdj":
                        self._increment_counter("append_adj_attempts")
                    elif mnemonic == "boot":
                        self._increment_counter("boot_attempts")
                    elif mnemonic == "move":
                        self._increment_counter("move_attempts")
                    nonlocal_actions.append(
                        NonlocalAction(
                            source_program_id=program.program_id,
                            source_x=program.x,
                            source_y=program.y,
                            target_x=target_x,
                            target_y=target_y,
                            opcode=opcode,
                            mnemonic=mnemonic,
                            executed_index=executed_index,
                            paid_with_background=used_background,
                            background_payment_amount=background_amount,
                            source_strength=self.world.strength_at(program.x, program.y),
                            source_src=program.src,
                            source_dst=program.dst,
                        )
                    )
                if program.program_id in self.world.programs and program.instructions:
                    program.ip = (program.ip + 1) % len(program.instructions)
                    self._maybe_mutate(program, executed_index, used_background, background_amount)
                return

            immediate_count += 1
            if immediate_count > immediate_limit:
                self.world.open_mask[program.y, program.x] = True
                return
            self._execute_immediate(program, opcode)

    def _pay_base_cost(self, program: ProgramState, cost: int) -> tuple[bool, bool, int]:
        if cost <= 0:
            return True, False, int(self.world.bg_radiation[program.y, program.x])
        if int(self.world.free_energy[program.y, program.x]) >= cost:
            self.world.free_energy[program.y, program.x] -= cost
            return True, False, int(self.world.bg_radiation[program.y, program.x])
        if int(self.world.free_energy[program.y, program.x]) == 0 and int(self.world.bg_radiation[program.y, program.x]) >= cost:
            background_amount = int(self.world.bg_radiation[program.y, program.x])
            self.world.bg_radiation[program.y, program.x] -= cost
            return True, True, background_amount
        return False, False, int(self.world.bg_radiation[program.y, program.x])

    def _maybe_mutate(
        self,
        program: ProgramState,
        executed_index: int,
        paid_with_background: bool,
        background_amount: int,
    ) -> None:
        if program.program_id not in self.world.programs or not 0 <= executed_index < len(program.instructions):
            return
        if paid_with_background:
            denominator = float(2 ** self.config.system_params.mutation_background_log2)
            probability = min(background_amount / denominator, 1.0)
        else:
            probability = 2.0 ** (-self.config.system_params.mutation_base_log2)
        if bool(self.rng.bernoulli(probability)):
            bit = self.rng.integers(0, 8)
            program.instructions[executed_index] ^= 1 << bit

    def _push(self, program: ProgramState, value: int) -> None:
        if len(program.stack) >= 0x7FFF:
            program.flag = 1
            return
        program.stack.append(wrap16(value))

    def _pop(self, program: ProgramState) -> int | None:
        if not program.stack:
            program.flag = 1
            return None
        return program.stack.pop()

    def _pop_binary(self, program: ProgramState) -> tuple[int, int] | None:
        if len(program.stack) < 2:
            while program.stack:
                program.stack.pop()
            program.flag = 1
            return None
        top = program.stack.pop()
        second = program.stack.pop()
        return second, top

    def _execute_immediate(self, program: ProgramState, opcode: int) -> None:
        size = len(program.instructions)
        next_ip = (program.ip + 1) % size
        if 0x00 <= opcode <= 0x0F:
            self._push(program, PUSH_LITERAL[opcode])
            program.ip = next_ip
            return
        if OPCODE_IS_NOOP[opcode]:
            program.ip = next_ip
            return
        if opcode == 0x10:
            if not program.stack:
                program.flag = 1
            else:
                self._push(program, program.stack[-1])
            program.ip = next_ip
            return
        if opcode == 0x11:
            if self._pop(program) is None:
                program.flag = 1
            program.ip = next_ip
            return
        if opcode == 0x12:
            if len(program.stack) < 2:
                program.flag = 1
            else:
                program.stack[-1], program.stack[-2] = program.stack[-2], program.stack[-1]
            program.ip = next_ip
            return
        if opcode == 0x13:
            if len(program.stack) < 2:
                program.flag = 1
            else:
                self._push(program, program.stack[-2])
            program.ip = next_ip
            return
        if opcode == 0x14:
            self._push(program, self.rng.integers(0, 256))
            program.ip = next_ip
            return
        if opcode in {0x20, 0x21, 0x23, 0x24, 0x25, 0x27, 0x28}:
            operands = self._pop_binary(program)
            if operands is not None:
                second, top = operands
                if opcode == 0x20:
                    self._push(program, second + top)
                elif opcode == 0x21:
                    self._push(program, second - top)
                elif opcode == 0x23:
                    self._push(program, 1 if second == top else 0)
                elif opcode == 0x24:
                    self._push(program, 1 if second < top else 0)
                elif opcode == 0x25:
                    self._push(program, 1 if second > top else 0)
                elif opcode == 0x27:
                    self._push(program, 1 if second != 0 and top != 0 else 0)
                else:
                    self._push(program, 1 if second != 0 or top != 0 else 0)
            program.ip = next_ip
            return
        if opcode in {0x22, 0x26}:
            value = self._pop(program)
            if value is not None:
                self._push(program, -value if opcode == 0x22 else (1 if value == 0 else 0))
            program.ip = next_ip
            return
        if opcode == 0x30:
            count = self._pop(program)
            if count is None:
                program.ip = next_ip
                return
            program.lc = wrap16(count)
            if program.lc <= 0:
                match = self._scan_forward(program, 0x31)
                if match is None:
                    program.flag = 1
                    program.ip = next_ip
                else:
                    program.ip = (match + 1) % size
                return
            program.ip = next_ip
            return
        if opcode == 0x31:
            if program.lc == 0:
                program.ip = next_ip
                return
            program.lc = wrap16(program.lc - 1)
            if program.lc > 0:
                match = self._scan_backward(program, 0x30)
                if match is None:
                    program.ip = next_ip
                else:
                    program.ip = (match + 1) % size
                return
            program.ip = next_ip
            return
        if opcode == 0x32:
            offset = self._pop(program)
            if offset is not None:
                program.ip = (program.ip + offset) % size
            else:
                program.ip = next_ip
            return
        if opcode in {0x33, 0x34}:
            if len(program.stack) < 2:
                program.flag = 1
                program.ip = next_ip
                return
            value = program.stack.pop()
            offset = program.stack.pop()
            should_jump = value != 0 if opcode == 0x33 else value == 0
            program.ip = (program.ip + offset) % size if should_jump else next_ip
            return
        if opcode == 0x40:
            program.dir = (program.dir + 1) % 4
            program.ip = next_ip
            return
        if opcode == 0x41:
            program.dir = (program.dir - 1) % 4
            program.ip = next_ip
            return
        if opcode == 0x42:
            self._push(program, size)
            program.ip = next_ip
            return
        if opcode == 0x43:
            self._push(program, program.ip)
            program.ip = next_ip
            return
        if opcode == 0x44:
            self._push(program, program.flag)
            program.ip = next_ip
            return
        if opcode == 0x45:
            self._push(program, program.msg)
            program.ip = next_ip
            return
        if opcode == 0x46:
            self._push(program, program.id_reg)
            program.ip = next_ip
            return
        if opcode == 0x4D:
            self._push(program, int(self.world.free_energy[program.y, program.x]))
            program.ip = next_ip
            return
        if opcode == 0x4E:
            self._push(program, int(self.world.free_mass[program.y, program.x]))
            program.ip = next_ip
            return
        if opcode == 0x47:
            self._push(program, program.src)
            program.ip = next_ip
            return
        if opcode == 0x48:
            self._push(program, program.dst)
            program.ip = next_ip
            return
        if opcode in {0x49, 0x4A, 0x4B, 0x4C}:
            value = self._pop(program)
            if value is not None:
                if opcode == 0x49:
                    program.dir = value % 4
                elif opcode == 0x4A:
                    program.src = wrap16(value)
                elif opcode == 0x4B:
                    program.dst = wrap16(value)
                else:
                    program.id_reg = wrap8(value)
            program.ip = next_ip

    def _scan_forward(self, program: ProgramState, target_opcode: int) -> int | None:
        match = _scan_forward_opcode(program.instructions, program.ip, target_opcode)
        return None if match < 0 else match

    def _scan_backward(self, program: ProgramState, target_opcode: int) -> int | None:
        match = _scan_backward_opcode(program.instructions, program.ip, target_opcode)
        return None if match < 0 else match

    def _execute_local(self, program: ProgramState, opcode: int, absorbers: set[tuple[int, int]]) -> None:
        x, y = program.x, program.y
        if opcode == 0x50:
            self.world.open_mask[y, x] = True
            return
        if opcode == 0x51:
            self.world.open_mask[y, x] = True
            absorbers.add((x, y))
            packets = self.world.radiation.pop((x, y), [])
            if packets:
                total = sum(packet.value for packet in packets)
                self.world.free_energy[y, x] = clamp_nonnegative(int(self.world.free_energy[y, x]) + total)
                last_packet = packets[-1]
                program.msg = wrap16(last_packet.value)
                program.dir = last_packet.direction % 4
                program.flag = 1
            return
        if opcode == 0x52:
            value = self._pop(program)
            if value is None:
                return
            self.world.radiation.setdefault((x, y), []).append(
                RadiationPacket(x=x, y=y, direction=program.dir, value=wrap16(value))
            )
            return
        if opcode == 0x53:
            self._push(program, program.instructions[program.src % len(program.instructions)])
            program.src = wrap16(program.src + 1)
            return
        if opcode == 0x54:
            value = self._pop(program)
            if value is None:
                return
            program.instructions[program.dst % len(program.instructions)] = wrap8(value)
            program.dst = wrap16(program.dst + 1)
            return
        if opcode == 0x55:
            target_index = program.dst % len(program.instructions)
            deleted = self._delete_instruction(program, target_index)
            if deleted:
                self.world.free_mass[y, x] += 1
            return
        if opcode == 0x56:
            if int(self.world.free_energy[y, x]) < self.config.system_params.N_synth:
                program.flag = 1
                return
            self.world.free_energy[y, x] -= self.config.system_params.N_synth
            self.world.free_mass[y, x] += 1

    def _delete_instruction(self, program: ProgramState, index: int) -> bool:
        if not program.instructions:
            return False
        if program.ip > index:
            program.ip -= 1
        del program.instructions[index]
        if not program.instructions:
            self._remove_program(program.program_id)
            return True
        size = len(program.instructions)
        if program.ip >= size:
            program.ip %= size
        return True

    def _tick_once(self) -> None:
        absorbers: set[tuple[int, int]] = set()
        newly_live_programs: set[int] = set()
        written_inert_programs: set[int] = set()
        self.world.recompute_open_mask()
        nonlocal_actions: list[NonlocalAction] = []

        for program_id in sorted(list(self.world.programs)):
            program = self.world.programs.get(program_id)
            if program is None or not program.live:
                continue
            self._run_pass1_for_program(program, absorbers, nonlocal_actions)

        snapshot = self.world.clone()
        winners = self._prepare_nonlocal_winners(snapshot, nonlocal_actions)
        self._apply_nonlocal_winners(snapshot, winners, newly_live_programs, written_inert_programs)
        self._update_inert_programs(written_inert_programs)
        self._run_physics(absorbers, newly_live_programs)
        self.world.recompute_open_mask()
        self.tick += 1

    def _distribute_absorbed_background(self, absorbers: set[tuple[int, int]]) -> None:
        if not absorbers:
            return
        absorber_mask = np.zeros_like(self.world.open_mask, dtype=np.int64)
        for x, y in absorbers:
            absorber_mask[y, x] = 1
        absorber_count = (
            absorber_mask
            + np.roll(absorber_mask, 1, axis=0)
            + np.roll(absorber_mask, -1, axis=0)
            + np.roll(absorber_mask, 1, axis=1)
            + np.roll(absorber_mask, -1, axis=1)
        )
        share = np.zeros_like(self.world.bg_radiation, dtype=np.int64)
        np.floor_divide(
            self.world.bg_radiation,
            absorber_count,
            out=share,
            where=absorber_count > 0,
        )
        self.world.bg_radiation -= share * absorber_count
        absorbed_energy = (
            share
            + np.roll(share, 1, axis=0)
            + np.roll(share, -1, axis=0)
            + np.roll(share, 1, axis=1)
            + np.roll(share, -1, axis=1)
        )
        self.world.free_energy += absorbed_energy * absorber_mask

    def _prepare_nonlocal_winners(self, snapshot: WorldState, actions: list[NonlocalAction]) -> list[NonlocalAction]:
        eligible: list[NonlocalAction] = []
        for action in actions:
            source_program = self.world.programs.get(action.source_program_id)
            source_snapshot = snapshot.programs.get(action.source_program_id)
            if source_program is None or source_snapshot is None:
                continue
            target_snapshot = snapshot.get_program(action.target_x, action.target_y)
            if action.mnemonic in {"writeAdj", "appendAdj", "giveE", "giveM"}:
                if not source_snapshot.stack:
                    source_program.flag = 1
                    continue
                action.stack_top = source_snapshot.stack[-1]
            additional_energy = 0
            additional_mass = 0
            if action.mnemonic == "appendAdj":
                additional_mass = 1
            elif action.mnemonic in {"delAdj", "takeE"}:
                additional_energy = snapshot.strength_at(action.target_x, action.target_y)
            if additional_energy > int(self.world.free_energy[action.source_y, action.source_x]):
                source_program.flag = 1
                continue
            if additional_mass > int(self.world.free_mass[action.source_y, action.source_x]):
                source_program.flag = 1
                continue
            self.world.free_energy[action.source_y, action.source_x] -= additional_energy
            self.world.free_mass[action.source_y, action.source_x] -= additional_mass
            if action.mnemonic == "boot":
                if target_snapshot is None or target_snapshot.live:
                    source_program.flag = 1
                    continue
            if action.mnemonic == "move":
                if target_snapshot is not None:
                    source_program.flag = 1
                    continue
            if action.mnemonic == "writeAdj" and target_snapshot is None:
                source_program.flag = 1
                continue
            if action.mnemonic == "appendAdj" and target_snapshot is not None and bool(snapshot.open_mask[action.target_y, action.target_x]) is False:
                source_program.flag = 1
                continue
            if action.mnemonic in PROTECTION_REQUIRED and target_snapshot is not None and bool(snapshot.open_mask[action.target_y, action.target_x]) is False:
                source_program.flag = 1
                continue
            eligible.append(action)

        grouped: dict[tuple[int, int], list[NonlocalAction]] = {}
        for action in eligible:
            grouped.setdefault((action.target_x, action.target_y), []).append(action)

        winners: list[NonlocalAction] = []
        for group in grouped.values():
            if len(group) == 1:
                winners.append(group[0])
                continue
            best_strength = max(action.source_strength for action in group)
            tied = [action for action in group if action.source_strength == best_strength]
            if len(tied) == 1:
                winner = tied[0]
            else:
                weights = []
                for action in tied:
                    program = snapshot.programs.get(action.source_program_id)
                    weights.append(len(program.instructions) if program is not None else 1)
                winner = tied[self.rng.weighted_choice_index(weights)]
            winners.append(winner)
            for loser in group:
                if loser is winner:
                    continue
                source_program = self.world.programs.get(loser.source_program_id)
                if source_program is not None:
                    source_program.flag = 1
        return winners

    def _apply_nonlocal_winners(
        self,
        snapshot: WorldState,
        winners: list[NonlocalAction],
        newly_live_programs: set[int],
        written_inert_programs: set[int],
    ) -> None:
        for action in winners:
            self._apply_nonlocal_target_effect(snapshot, action, newly_live_programs, written_inert_programs)
        for action in winners:
            self._apply_nonlocal_source_effect(snapshot, action)
        for action in winners:
            source_program = self.world.programs.get(action.source_program_id)
            if source_program is not None and source_program.flag != 1:
                source_program.flag = 0

    def _record_inert_write(
        self,
        program: ProgramState,
        writer_program: ProgramState | None,
        written_inert_programs: set[int],
    ) -> None:
        if program.live:
            return
        program.inert_ticks_without_write = 0
        program.writes_received += 1
        if writer_program is not None:
            program.last_writer_program_id = writer_program.program_id
            program.last_writer_hash = compute_program_hash(writer_program.instructions)
        written_inert_programs.add(program.program_id)
        self._increment_counter("inert_write_events")

    def _apply_nonlocal_target_effect(
        self,
        snapshot: WorldState,
        action: NonlocalAction,
        newly_live_programs: set[int],
        written_inert_programs: set[int],
    ) -> None:
        target_snapshot = snapshot.get_program(action.target_x, action.target_y)
        current_target = self.world.get_program(action.target_x, action.target_y)
        source_program = self.world.programs.get(action.source_program_id)
        if action.mnemonic == "writeAdj":
            if target_snapshot is None or current_target is None or action.stack_top is None:
                if source_program is not None:
                    source_program.flag = 1
                return
            index = action.source_dst % len(target_snapshot.instructions)
            current_target.instructions[index] = wrap8(action.stack_top)
            self._record_inert_write(current_target, source_program, written_inert_programs)
            return
        if action.mnemonic == "appendAdj":
            if action.stack_top is None:
                return
            self._increment_counter("append_adj_successes")
            if target_snapshot is None:
                if current_target is not None:
                    return
                self.world.new_program(
                    action.target_x,
                    action.target_y,
                    [wrap8(action.stack_top)],
                    self.rng,
                    live=False,
                    birth_kind="constructed",
                    created_tick=self.tick,
                    creator_program_id=None if source_program is None else source_program.program_id,
                    creator_hash=None if source_program is None else compute_program_hash(source_program.instructions),
                )
                self._increment_counter("inert_created")
                current_target = self.world.get_program(action.target_x, action.target_y)
                if current_target is not None:
                    self._record_inert_write(current_target, source_program, written_inert_programs)
            elif current_target is not None:
                current_target.instructions.append(wrap8(action.stack_top))
                self._record_inert_write(current_target, source_program, written_inert_programs)
            return
        if action.mnemonic == "delAdj":
            if target_snapshot is None or current_target is None:
                source_program = self.world.programs.get(action.source_program_id)
                if source_program is not None:
                    source_program.flag = 1
                return
            index = action.source_dst % len(target_snapshot.instructions)
            self._delete_instruction(current_target, index)
            return
        if action.mnemonic == "takeE":
            amount = (int(snapshot.free_energy[action.target_y, action.target_x]) + 1) // 2
            self.world.free_energy[action.target_y, action.target_x] = max(
                0,
                int(self.world.free_energy[action.target_y, action.target_x]) - amount,
            )
            return
        if action.mnemonic == "takeM":
            amount = (int(snapshot.free_mass[action.target_y, action.target_x]) + 1) // 2
            self.world.free_mass[action.target_y, action.target_x] = max(
                0,
                int(self.world.free_mass[action.target_y, action.target_x]) - amount,
            )
            return
        if action.mnemonic == "boot" and current_target is not None and not current_target.live:
            was_under_grace = self._inert_is_under_grace(current_target)
            current_target.live = True
            current_target.age = 0
            current_target.inert_ticks_without_write = 0
            current_target.boot_tick = self.tick
            current_target.boot_size = len(current_target.instructions)
            current_target.boot_writes_received = current_target.writes_received
            current_target.booted_by_program_id = None if source_program is None else source_program.program_id
            current_target.booted_by_hash = None if source_program is None else compute_program_hash(source_program.instructions)
            current_target.booted_while_under_grace = was_under_grace
            newly_live_programs.add(current_target.program_id)
            self._increment_counter("boot_successes")
            self._increment_counter(
                "boot_successes_active_construction" if was_under_grace else "boot_successes_abandoned"
            )

    def _update_inert_programs(
        self,
        written_inert_programs: set[int],
    ) -> None:
        grace_ticks = int(self.config.system_params.inert_grace_ticks)
        for program in self.world.programs.values():
            if program.live:
                program.inert_ticks_without_write = 0
                continue
            if program.program_id in written_inert_programs:
                program.inert_ticks_without_write = 0
                continue
            program.inert_ticks_without_write += 1
            if grace_ticks > 0 and program.inert_ticks_without_write == grace_ticks:
                self._increment_counter("inert_abandonments")

    def _apply_nonlocal_source_effect(self, snapshot: WorldState, action: NonlocalAction) -> None:
        source_program = self.world.programs.get(action.source_program_id)
        source_snapshot = snapshot.programs.get(action.source_program_id)
        if source_program is None or source_snapshot is None:
            return
        target_snapshot = snapshot.get_program(action.target_x, action.target_y)
        if action.mnemonic == "readAdj":
            if target_snapshot is None:
                self._push(source_program, 0)
                source_program.flag = 1
            else:
                self._push(source_program, target_snapshot.instructions[action.source_src % len(target_snapshot.instructions)])
            source_program.src = wrap16(source_program.src + 1)
            return
        if action.mnemonic == "writeAdj":
            if self._pop(source_program) is None:
                source_program.flag = 1
                return
            source_program.dst = wrap16(source_program.dst + 1)
            return
        if action.mnemonic == "appendAdj":
            if self._pop(source_program) is None:
                source_program.flag = 1
            return
        if action.mnemonic == "delAdj":
            self.world.free_mass[source_program.y, source_program.x] += 1
            return
        if action.mnemonic == "senseSize":
            self._push(source_program, len(target_snapshot.instructions) if target_snapshot is not None else 0)
            return
        if action.mnemonic == "senseE":
            self._push(source_program, int(snapshot.free_energy[action.target_y, action.target_x]))
            return
        if action.mnemonic == "senseM":
            self._push(source_program, int(snapshot.free_mass[action.target_y, action.target_x]))
            return
        if action.mnemonic == "senseID":
            if target_snapshot is None:
                self._push(source_program, 0)
                source_program.flag = 1
            else:
                self._push(source_program, target_snapshot.id_reg)
            return
        if action.mnemonic == "giveE":
            amount = self._pop(source_program)
            if amount is None:
                source_program.flag = 1
                return
            if amount > 0:
                transfer = min(amount, int(self.world.free_energy[source_program.y, source_program.x]))
                self.world.free_energy[source_program.y, source_program.x] -= transfer
                self.world.free_energy[action.target_y, action.target_x] += transfer
            return
        if action.mnemonic == "giveM":
            amount = self._pop(source_program)
            if amount is None:
                source_program.flag = 1
                return
            if amount > 0:
                transfer = min(amount, int(self.world.free_mass[source_program.y, source_program.x]))
                self.world.free_mass[source_program.y, source_program.x] -= transfer
                self.world.free_mass[action.target_y, action.target_x] += transfer
            return
        if action.mnemonic == "takeE":
            amount = (int(snapshot.free_energy[action.target_y, action.target_x]) + 1) // 2
            self.world.free_energy[source_program.y, source_program.x] += amount
            return
        if action.mnemonic == "takeM":
            amount = (int(snapshot.free_mass[action.target_y, action.target_x]) + 1) // 2
            self.world.free_mass[source_program.y, source_program.x] += amount
            return
        if action.mnemonic == "move":
            if self.world.get_program(action.target_x, action.target_y) is not None:
                source_program.flag = 1
                return
            source_x, source_y = source_program.x, source_program.y
            target_x, target_y = action.target_x, action.target_y
            self.world.program_ids[source_y, source_x] = -1
            self.world.program_ids[target_y, target_x] = source_program.program_id
            source_program.x = target_x
            source_program.y = target_y
            self.world.free_energy[target_y, target_x] += int(self.world.free_energy[source_y, source_x])
            self.world.free_mass[target_y, target_x] += int(self.world.free_mass[source_y, source_x])
            self.world.free_energy[source_y, source_x] = 0
            self.world.free_mass[source_y, source_x] = 0
            self._increment_counter("move_successes")

    def _run_physics(
        self,
        absorbers: set[tuple[int, int]],
        newly_live_programs: set[int],
    ) -> None:
        arrivals: dict[tuple[int, int], list[RadiationPacket]] = {}
        for packets in self.world.radiation.values():
            for packet in packets:
                target_x, target_y = self.world.neighbor(packet.x, packet.y, packet.direction)
                arrivals.setdefault((target_x, target_y), []).append(
                    RadiationPacket(
                        x=target_x,
                        y=target_y,
                        direction=packet.direction,
                        value=packet.value,
                    )
                )
        self.world.radiation = {}
        for (x, y), packets in arrivals.items():
            if len(packets) == 1:
                self.world.radiation[(x, y)] = packets
            else:
                total = sum(packet.value for packet in packets)
                self.world.free_energy[y, x] = clamp_nonnegative(int(self.world.free_energy[y, x]) + total)

        energy_arrivals = self.rng.bernoulli(self.config.system_params.R_energy, size=self.world.bg_radiation.shape)
        self.world.bg_radiation += energy_arrivals.astype(np.int64)
        energy_decay = self.rng.binomial(self.world.bg_radiation, self.config.system_params.D_energy)
        self.world.bg_radiation -= energy_decay.astype(np.int64)
        self._distribute_absorbed_background(absorbers)

        mass_arrivals = self.rng.bernoulli(self.config.system_params.R_mass, size=self.world.free_mass.shape)
        spawn_mask = np.zeros_like(mass_arrivals, dtype=np.bool_)
        if self.config.system_params.P_spawn > 0:
            empty_arrivals = mass_arrivals & (self.world.program_ids < 0)
            if np.any(empty_arrivals):
                spawn_mask = empty_arrivals & self.rng.bernoulli(
                    self.config.system_params.P_spawn,
                    size=self.world.free_mass.shape,
                )
                ys, xs = np.nonzero(spawn_mask)
                for y, x in zip(ys.tolist(), xs.tolist()):
                    if self.world.get_program(x, y) is None:
                        program = self.world.new_program(
                            x,
                            y,
                            [NOP_OPCODE],
                            self.rng,
                            live=True,
                            birth_kind="spawn",
                            created_tick=self.tick,
                        )
                        newly_live_programs.add(program.program_id)
        self.world.free_mass += (mass_arrivals & ~spawn_mask).astype(np.int64)

        for program_id in list(self.world.programs):
            program = self.world.programs.get(program_id)
            if program is None or not program.instructions:
                continue
            size = len(program.instructions)
            maintenance_rate = self.config.system_params.M
            if not program.live and self._inert_is_under_grace(program):
                maintenance_rate = 0.0
            cost = int(self.rng.binomial(size, maintenance_rate))
            if cost <= 0:
                continue
            x, y = program.x, program.y
            from_energy = min(cost, int(self.world.free_energy[y, x]))
            self.world.free_energy[y, x] -= from_energy
            remaining = cost - from_energy
            if remaining > 0:
                from_mass = min(remaining, int(self.world.free_mass[y, x]))
                self.world.free_mass[y, x] -= from_mass
                remaining -= from_mass
            if remaining > 0:
                if remaining >= size:
                    self._remove_program(program.program_id)
                    continue
                del program.instructions[-remaining:]
                if not program.instructions:
                    self._remove_program(program.program_id)
                    continue
                size = len(program.instructions)
                if program.ip >= size:
                    program.ip %= size

        threshold = np.zeros_like(self.world.free_energy)
        for program in self.world.programs.values():
            threshold[program.y, program.x] = self.config.system_params.T_cap * len(program.instructions)
        excess_energy = np.maximum(0, self.world.free_energy - threshold)
        excess_mass = np.maximum(0, self.world.free_mass - threshold)
        self.world.free_energy -= self.rng.binomial(excess_energy, self.config.system_params.D_energy).astype(np.int64)
        self.world.free_mass -= self.rng.binomial(excess_mass, self.config.system_params.D_mass).astype(np.int64)

        for program in self.world.programs.values():
            if program.live and program.program_id not in newly_live_programs:
                program.age += 1
