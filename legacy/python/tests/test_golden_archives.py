from __future__ import annotations

import pytest

from archive_utils import canonicalize_archive_snapshot, load_archive_fixture
from proteus.engine import SimulationSession
from proteus.models import SeedSpec, SimulationConfig, SystemParams


def build_control_flow_bootstrap_config() -> SimulationConfig:
    return SimulationConfig(
        width=6,
        height=5,
        rng_seed=23,
        system_params=SystemParams(
            R_energy=0.0,
            R_mass=0.0,
            P_spawn=0.0,
            D_energy=0.0,
            D_mass=0.0,
            T_cap=2,
            M=0.0,
            N_synth=3,
        ),
        seeds=[
            SeedSpec(
                assembly_source="push 2\nfor\nnop\nnext\npush 0\nappendAdj\nboot\n",
                x=1,
                y=2,
                count=1,
                initial_dir=0,
                initial_free_energy=12,
                initial_free_mass=3,
                neighbor_free_energy=1,
                neighbor_free_mass=0,
            )
        ],
    )


@pytest.mark.parametrize(
    ("fixture_name", "steps"),
    [
        ("control_flow_bootstrap_tick0", 0),
        ("control_flow_bootstrap_tick7", 7),
    ],
)
def test_control_flow_bootstrap_archive_matches_golden_fixture(fixture_name: str, steps: int):
    session = SimulationSession.from_config(build_control_flow_bootstrap_config())
    session.advance(steps)

    assert canonicalize_archive_snapshot(session.export_archive()) == load_archive_fixture(fixture_name)
