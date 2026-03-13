from __future__ import annotations

from proteus.defaults import build_default_config
from proteus.models import SimulationConfig, SystemParams


def build_case(case_name: str) -> tuple[SimulationConfig, int]:
    if case_name == "default":
        return build_default_config(), 1000
    if case_name == "empty64":
        return (
            SimulationConfig(
                width=64,
                height=64,
                rng_seed=1,
                system_params=SystemParams(
                    R_energy=0.25,
                    R_mass=0.05,
                    P_spawn=0.0,
                    D_energy=0.01,
                    D_mass=0.01,
                    M=0.0,
                ),
                seeds=[],
            ),
            2000,
        )
    raise ValueError(f"Unknown benchmark case: {case_name}")
