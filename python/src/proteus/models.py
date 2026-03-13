from __future__ import annotations

from dataclasses import dataclass, field, is_dataclass
from typing import Any

import numpy as np


def wrap16(value: int) -> int:
    value = int(value)
    return ((value + 0x8000) % 0x10000) - 0x8000


def wrap8(value: int) -> int:
    return int(value) & 0xFF


def clamp_nonnegative(value: int) -> int:
    return max(0, int(value))


def to_builtin(value: Any) -> Any:
    if is_dataclass(value):
        return {key: to_builtin(getattr(value, key)) for key in value.__dataclass_fields__}
    if isinstance(value, np.ndarray):
        return value.tolist()
    if isinstance(value, np.integer):
        return int(value)
    if isinstance(value, np.floating):
        return float(value)
    if isinstance(value, dict):
        return {str(key): to_builtin(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [to_builtin(item) for item in value]
    return value


@dataclass(slots=True)
class SystemParams:
    R_energy: float = 0.25
    R_mass: float = 0.05
    P_spawn: float = 0.0
    D_energy: float = 0.01
    D_mass: float = 0.01
    T_cap: int = 4
    M: float = 1 / 128
    inert_grace_ticks: int = 10
    N_synth: int = 1
    mutation_base_log2: int = 16
    mutation_background_log2: int = 8

    def validate(self) -> None:
        for name in ("R_energy", "R_mass", "P_spawn", "D_energy", "D_mass", "M"):
            value = float(getattr(self, name))
            if not 0.0 <= value <= 1.0:
                raise ValueError(f"{name} must be between 0 and 1.")
        if self.T_cap < 0:
            raise ValueError("T_cap must be nonnegative.")
        if self.inert_grace_ticks < 0:
            raise ValueError("inert_grace_ticks must be nonnegative.")
        if self.N_synth < 0:
            raise ValueError("N_synth must be nonnegative.")
        if self.mutation_base_log2 < 0:
            raise ValueError("mutation_base_log2 must be nonnegative.")
        if self.mutation_background_log2 < 0:
            raise ValueError("mutation_background_log2 must be nonnegative.")


@dataclass(slots=True)
class SeedSpec:
    assembly_source: str
    x: int
    y: int
    count: int = 1
    preset_key: str | None = None
    randomize_additional_seeds: bool = False
    initial_dir: int | None = None
    initial_id: int | None = None
    initial_free_energy: int = 20
    initial_free_mass: int = 11
    neighbor_free_energy: int = 20
    neighbor_free_mass: int = 11
    live: bool = True

    def validate(self) -> None:
        if not self.assembly_source.strip():
            raise ValueError("Seed assembly_source cannot be empty.")
        if self.count <= 0:
            raise ValueError("count must be positive.")
        for name in (
            "initial_free_energy",
            "initial_free_mass",
            "neighbor_free_energy",
            "neighbor_free_mass",
        ):
            if getattr(self, name) < 0:
                raise ValueError(f"{name} must be nonnegative.")
        if self.initial_dir is not None and self.initial_dir not in (0, 1, 2, 3):
            raise ValueError("initial_dir must be 0..3 when provided.")
        if self.initial_id is not None and not 0 <= self.initial_id <= 255:
            raise ValueError("initial_id must be 0..255 when provided.")


@dataclass(slots=True)
class SimulationConfig:
    width: int = 64
    height: int = 64
    rng_seed: int = 1
    system_params: SystemParams = field(default_factory=SystemParams)
    seeds: list[SeedSpec] = field(default_factory=list)

    def validate(self) -> None:
        if self.width <= 0 or self.height <= 0:
            raise ValueError("width and height must be positive.")
        if self.width > 1024 or self.height > 1024:
            raise ValueError("width and height must be 1024 or smaller in v1.")
        self.system_params.validate()
        total_seed_count = 0
        seen_positions: set[tuple[int, int]] = set()
        for seed in self.seeds:
            seed.validate()
            total_seed_count += seed.count
            position = (seed.x % self.width, seed.y % self.height)
            if position in seen_positions:
                raise ValueError("Seed positions must be unique.")
            seen_positions.add(position)
        if total_seed_count > self.width * self.height:
            raise ValueError("Total seed count cannot exceed the number of cells in the world.")


@dataclass(slots=True)
class ProgramState:
    program_id: int
    x: int
    y: int
    instructions: list[int]
    ip: int = 0
    dir: int = 0
    src: int = 0
    dst: int = 0
    flag: int = 0
    msg: int = 0
    id_reg: int = 0
    lc: int = 0
    stack: list[int] = field(default_factory=list)
    live: bool = True
    age: int = 0
    inert_ticks_without_write: int = 0

    @property
    def size(self) -> int:
        return len(self.instructions)

    @property
    def strength(self) -> int:
        return 0

    def clone(self) -> ProgramState:
        return ProgramState(
            program_id=self.program_id,
            x=self.x,
            y=self.y,
            instructions=list(self.instructions),
            ip=self.ip,
            dir=self.dir,
            src=self.src,
            dst=self.dst,
            flag=self.flag,
            msg=self.msg,
            id_reg=self.id_reg,
            lc=self.lc,
            stack=list(self.stack),
            live=self.live,
            age=self.age,
            inert_ticks_without_write=self.inert_ticks_without_write,
        )


@dataclass(slots=True)
class RadiationPacket:
    x: int
    y: int
    direction: int
    value: int

    def clone(self) -> RadiationPacket:
        return RadiationPacket(
            x=self.x,
            y=self.y,
            direction=self.direction,
            value=self.value,
        )


@dataclass(slots=True)
class NonlocalAction:
    source_program_id: int
    source_x: int
    source_y: int
    target_x: int
    target_y: int
    opcode: int
    mnemonic: str
    executed_index: int
    paid_with_background: bool
    background_payment_amount: int
    source_strength: int
    source_src: int
    source_dst: int
    stack_top: int | None = None


@dataclass(slots=True)
class ControlResult:
    status: str
    target_tps: float | None
    summary: dict[str, Any]
