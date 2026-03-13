from __future__ import annotations

from proteus.analysis import evaluate_milestones, sample_time_series
from proteus.models import SeedSpec, SimulationConfig, SystemParams


def test_evaluate_milestones_reports_size_buckets():
    config = SimulationConfig(
        width=6,
        height=6,
        rng_seed=1,
        system_params=SystemParams(R_energy=0.0, R_mass=0.0, P_spawn=0.0, D_energy=0.0, D_mass=0.0, M=0.0),
        seeds=[
            SeedSpec(
                assembly_source="nop\n",
                x=2,
                y=2,
                initial_free_energy=0,
                initial_free_mass=0,
                neighbor_free_energy=0,
                neighbor_free_mass=0,
            )
        ],
    )

    results = evaluate_milestones(config, [0])
    population = results["snapshots"][0]["population"]

    assert population["size_bucket_counts"]["size_1"] == 1
    assert population["size_bucket_shares"]["size_1"] == 1.0
    assert population["dominant_hash_share"] == 1.0


def test_evaluate_milestones_reports_structure_metrics():
    config = SimulationConfig(
        width=8,
        height=8,
        rng_seed=1,
        system_params=SystemParams(R_energy=0.0, R_mass=0.0, P_spawn=0.0, D_energy=0.0, D_mass=0.0, M=0.0),
        seeds=[
            SeedSpec(
                assembly_source="absorb\nabsorb\nabsorb\n",
                x=1,
                y=1,
                initial_free_energy=0,
                initial_free_mass=0,
                neighbor_free_energy=0,
                neighbor_free_mass=0,
            ),
            SeedSpec(
                assembly_source="read\nappendAdj\nboot\n",
                x=5,
                y=5,
                initial_free_energy=0,
                initial_free_mass=0,
                neighbor_free_energy=0,
                neighbor_free_mass=0,
            ),
        ],
    )

    results = evaluate_milestones(config, [0])
    population = results["snapshots"][0]["population"]
    structure = population["structure"]

    assert population["live_effective_size_stats"]["median"] == 2.0
    assert structure["live_builder_share"] == 0.5
    assert structure["live_replicator_motif_share"] == 0.5
    assert structure["live_replicator_share"] == 0.0
    assert structure["live_absorb_only_share"] == 0.5
    assert structure["live_absorb_dominated_share"] == 0.5
    assert structure["live_nop_only_share"] == 0.0


def test_sample_time_series_distinguishes_fragment_and_nontrivial_boots():
    config = SimulationConfig(
        width=8,
        height=8,
        rng_seed=1,
        system_params=SystemParams(R_energy=0.0, R_mass=0.0, P_spawn=0.0, D_energy=0.0, D_mass=0.0, M=0.0),
        seeds=[
            SeedSpec(
                assembly_source="read\nappendAdj\nread\nappendAdj\nread\nappendAdj\nread\nappendAdj\nboot\n",
                x=2,
                y=2,
                initial_free_energy=16,
                initial_free_mass=16,
                neighbor_free_energy=0,
                neighbor_free_mass=0,
            )
        ],
    )

    results = sample_time_series(config, total_ticks=20, sample_every=20)
    point = results["series"][-1]

    assert point["replicator_motif_share"] > 0.0
    assert point["boot_nontrivial_survivor_share_10"] > 0.0
    assert point["boot_fragment_survivor_share_10"] == 0.0
    assert point["constructed_live_share"] > 0.0
    assert point["booted_live_share"] > 0.0


def test_sample_time_series_reports_inert_waiting_and_abandonment():
    config = SimulationConfig(
        width=6,
        height=6,
        rng_seed=1,
        system_params=SystemParams(
            R_energy=0.0,
            R_mass=0.0,
            P_spawn=0.0,
            D_energy=0.0,
            D_mass=0.0,
            M=0.0,
            inert_grace_ticks=2,
        ),
        seeds=[
            SeedSpec(
                assembly_source="nop\n",
                x=2,
                y=2,
                initial_free_energy=0,
                initial_free_mass=0,
                neighbor_free_energy=0,
                neighbor_free_mass=0,
                live=False,
            )
        ],
    )

    results = sample_time_series(config, total_ticks=2, sample_every=1)
    series = results["series"]

    assert [point["tick"] for point in series] == [0, 1, 2]
    assert series[0]["inert_programs"] == 1
    assert series[1]["max_inert_ticks_without_write"] == 1
    assert series[1]["inert_waiting_programs"] == 1
    assert series[2]["live_programs"] == 0
    assert series[2]["inert_programs"] == 1
    assert series[2]["abandoned_inert_programs"] == 1
    assert series[2]["counters"]["inert_abandonments"] == 1
