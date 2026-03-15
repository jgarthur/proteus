from __future__ import annotations

from typing import Any, Literal

from pydantic import BaseModel, ConfigDict, Field


class SystemParamsModel(BaseModel):
    model_config = ConfigDict(extra="forbid")

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


class SeedSpecModel(BaseModel):
    model_config = ConfigDict(extra="forbid")

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


class SimulationConfigModel(BaseModel):
    model_config = ConfigDict(extra="forbid")

    width: int = 64
    height: int = 64
    rng_seed: int = 1
    system_params: SystemParamsModel = Field(default_factory=SystemParamsModel)
    seeds: list[SeedSpecModel] = Field(default_factory=list)


class AssembleRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")

    source: str


class ControlRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")

    action: Literal["play", "pause", "step", "reset", "set_speed"]
    steps: int = 1
    target_tps: float | None = None


class ImportRunRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")

    archive: dict[str, Any]
