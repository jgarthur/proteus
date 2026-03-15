from __future__ import annotations

from archive_utils import canonicalize_archive_snapshot
from proteus import engine as active_engine
from proteus import engine_reference
from proteus.models import SeedSpec, SimulationConfig, SystemParams


def build_reference_parity_config() -> SimulationConfig:
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
            M=0.0,
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


def test_active_hash_normalization_matches_python_reference():
    instructions = [0xAA, 0x50, 0x02, 0xBB, 0x31, 0xCC]

    assert engine_reference._normalize_program_for_hash(
        instructions,
        engine_reference.CANONICAL_NOOP_HASH_OPCODE,
    ) == engine_reference._normalize_program_for_hash_python(
        instructions,
        engine_reference.CANONICAL_NOOP_HASH_OPCODE,
    )


def test_active_scan_helpers_match_python_reference():
    instructions = [0x02, 0x30, 0x50, 0x31, 0x00, 0x30]

    for start_ip in range(len(instructions)):
        for target_opcode in (0x30, 0x31, 0x50, 0xFF):
            assert engine_reference._scan_forward_opcode(
                instructions,
                start_ip,
                target_opcode,
            ) == engine_reference._scan_forward_opcode_python(
                instructions,
                start_ip,
                target_opcode,
            )
            assert engine_reference._scan_backward_opcode(
                instructions,
                start_ip,
                target_opcode,
            ) == engine_reference._scan_backward_opcode_python(
                instructions,
                start_ip,
                target_opcode,
            )


def test_active_engine_matches_reference_archive():
    active_session = active_engine.SimulationSession.from_config(build_reference_parity_config())
    reference_session = engine_reference.SimulationSession.from_config(build_reference_parity_config())

    active_session.advance(7)
    reference_session.advance(7)

    assert canonicalize_archive_snapshot(active_session.export_archive()) == canonicalize_archive_snapshot(
        reference_session.export_archive()
    )
