from __future__ import annotations

from proteus.models import SeedSpec, SimulationConfig, SystemParams, to_builtin
from proteus.seed_presets import BASIC_SEED, preset_payload


def build_default_config() -> SimulationConfig:
    return SimulationConfig(
        width=64,
        height=64,
        rng_seed=1,
        system_params=SystemParams(),
        seeds=[
            SeedSpec(
                assembly_source=BASIC_SEED,
                x=32,
                y=32,
                count=20,
                preset_key="basic",
                randomize_additional_seeds=False,
                initial_free_energy=20,
                initial_free_mass=11,
                neighbor_free_energy=20,
                neighbor_free_mass=11,
            )
        ],
    )


def build_defaults_payload() -> dict:
    config = build_default_config()
    return {
        "config": to_builtin(config),
        "seed_assembly": BASIC_SEED,
        "seed_presets": preset_payload(),
    }
