from proteus.engine import SimulationSession, compute_program_hash
from proteus.models import SeedSpec, SimulationConfig, SystemParams


def build_config(seed_source: str, *, live: bool = True, initial_dir: int = 0, extra_seeds=None):
    seeds = [
        SeedSpec(
            assembly_source=seed_source,
            x=2,
            y=2,
            count=1,
            initial_dir=initial_dir,
            initial_free_energy=8,
            initial_free_mass=8,
            neighbor_free_energy=0,
            neighbor_free_mass=0,
            live=live,
        )
    ]
    if extra_seeds:
        seeds.extend(extra_seeds)
    return SimulationConfig(
        width=8,
        height=8,
        rng_seed=7,
        system_params=SystemParams(R_energy=0.0, R_mass=0.0, P_spawn=0.0, D_energy=0.0, D_mass=0.0, M=0.0),
        seeds=seeds,
    )


def test_seed_count_places_multiple_seed_programs():
    config = build_config("nop\n")
    config.seeds[0].count = 3

    session = SimulationSession.from_config(config)

    assert session.summary()["occupied_cells"] == 3
    hashes = {
        tuple(program.instructions)
        for program in session.world.programs.values()
    }
    assert len(hashes) == 1


def test_bootstrap_resources_only_touch_seed_cross():
    config = build_config("nop\n")
    config.seeds[0].initial_free_energy = 20
    config.seeds[0].initial_free_mass = 11
    config.seeds[0].neighbor_free_energy = 7
    config.seeds[0].neighbor_free_mass = 3

    session = SimulationSession.from_config(config)

    seeded_positions = set(session.world.plus_neighbors(2, 2))
    for y in range(session.world.height):
        for x in range(session.world.width):
            if (x, y) == (2, 2):
                assert int(session.world.free_energy[y, x]) == 20
                assert int(session.world.free_mass[y, x]) == 11
            elif (x, y) in seeded_positions:
                assert int(session.world.free_energy[y, x]) == 7
                assert int(session.world.free_mass[y, x]) == 3
            else:
                assert int(session.world.free_energy[y, x]) == 0
                assert int(session.world.free_mass[y, x]) == 0


def test_mutation_base_rate_is_configurable():
    config = build_config("nop\n")
    config.system_params.mutation_base_log2 = 0

    session = SimulationSession.from_config(config)
    original = list(session.world.programs.values())[0].instructions[0]

    session.advance(1)

    mutated = list(session.world.programs.values())[0].instructions[0]
    assert mutated != original


def test_program_hash_treats_all_unknown_noops_the_same():
    hash_a = compute_program_hash([0xAA, 0x50, 0xBB])
    hash_b = compute_program_hash([0xCC, 0x50, 0xDD])
    hash_c = compute_program_hash([0xAA, 0x51, 0xBB])

    assert hash_a == hash_b
    assert hash_a != hash_c


def test_background_mutation_scale_is_configurable():
    config = build_config("push 0\nemit\n")
    config.seeds[0].initial_free_energy = 0
    config.seeds[0].initial_free_mass = 0
    config.system_params.mutation_base_log2 = 60
    config.system_params.mutation_background_log2 = 0

    session = SimulationSession.from_config(config)
    session.world.bg_radiation[2, 2] = 1

    original = list(session.world.programs.values())[0].instructions[1]
    session.advance(1)
    mutated = list(session.world.programs.values())[0].instructions[1]

    assert mutated != original


def test_empty_world_mass_reaches_expected_scale_without_spawn():
    config = SimulationConfig(
        width=32,
        height=32,
        rng_seed=1,
        system_params=SystemParams(
            R_energy=0.0,
            R_mass=0.05,
            P_spawn=0.0,
            D_energy=0.0,
            D_mass=0.01,
            M=0.0,
        ),
        seeds=[],
    )

    session = SimulationSession.from_config(config)
    totals: list[int] = []
    for step in range(3000):
        session.advance(1)
        if step >= 2500:
            totals.append(session.summary()["total_free_mass"])

    average_total = sum(totals) / len(totals)
    assert 4800 <= average_total <= 5400


def test_append_adj_creates_inert_offspring():
    session = SimulationSession.from_config(build_config("push 0\nappendAdj\n"))

    session.advance(1)

    detail = session.cell_detail(3, 2)
    assert detail["program"] is not None
    assert detail["program"]["live"] is False
    assert detail["program"]["size"] == 1


def test_inert_programs_skip_maintenance_until_boot():
    config = build_config("push 0\nappendAdj\n")
    config.system_params.M = 1.0
    config.system_params.inert_grace_ticks = 100
    session = SimulationSession.from_config(config)

    session.advance(1)

    detail = session.cell_detail(3, 2)
    assert detail["program"] is not None
    assert detail["program"]["live"] is False
    assert detail["program"]["size"] == 1


def test_abandoned_inert_program_dies_via_normal_maintenance():
    config = build_config("push 0\nappendAdj\nnop\nnop\n")
    config.system_params.M = 1.0
    config.system_params.inert_grace_ticks = 2
    session = SimulationSession.from_config(config)

    session.advance(1)
    session.advance(1)
    session.advance(1)
    detail = session.cell_detail(3, 2)
    assert detail["program"] is None
    assert session.summary()["counters"]["inert_abandonments"] == 1
    assert session.summary()["counters"]["inert_removed_preboot"] == 1


def test_boot_transitions_adjacent_inert_program_to_live():
    inert_seed = SeedSpec(
        assembly_source="nop\n",
        x=3,
        y=2,
        initial_dir=0,
        initial_free_energy=0,
        initial_free_mass=0,
        neighbor_free_energy=0,
        neighbor_free_mass=0,
        live=False,
    )
    session = SimulationSession.from_config(build_config("boot\n", extra_seeds=[inert_seed]))

    session.advance(1)

    detail = session.cell_detail(3, 2)
    assert detail["program"] is not None
    assert detail["program"]["live"] is True


def test_constructed_offspring_records_boot_metadata():
    config = build_config("push 0\nappendAdj\nboot\n")
    config.system_params.M = 0.0
    session = SimulationSession.from_config(config)

    session.advance(2)

    detail = session.cell_detail(3, 2)
    assert detail["program"] is not None
    assert detail["program"]["live"] is True
    assert detail["program"]["birth_kind"] == "constructed"
    assert detail["program"]["writes_received"] == 1
    assert detail["program"]["boot_size"] == 1
    assert detail["program"]["boot_writes_received"] == 1
    assert detail["program"]["boot_tick"] == 1


def test_move_attempts_and_successes_are_counted():
    session = SimulationSession.from_config(build_config("move\n"))

    session.advance(1)

    counters = session.summary()["counters"]
    assert counters["move_attempts"] == 1
    assert counters["move_successes"] == 1


def test_inert_program_stays_inert_after_grace_without_boot():
    config = build_config("nop\n", live=False)
    config.system_params.inert_grace_ticks = 2

    session = SimulationSession.from_config(config)
    session.advance(1)
    detail = session.cell_detail(2, 2)
    assert detail["program"] is not None
    assert detail["program"]["live"] is False
    assert detail["program"]["inert_ticks_without_write"] == 1

    session.advance(1)
    detail = session.cell_detail(2, 2)
    assert detail["program"] is not None
    assert detail["program"]["live"] is False
    assert detail["program"]["inert_ticks_without_write"] == 2


def test_inert_write_resets_abandonment_counter():
    config = build_config("push 0\nappendAdj\nnop\n")
    config.system_params.M = 1.0
    config.system_params.inert_grace_ticks = 2
    session = SimulationSession.from_config(config)

    session.advance(1)
    detail = session.cell_detail(3, 2)
    assert detail["program"] is not None
    assert detail["program"]["inert_ticks_without_write"] == 0

    session.advance(1)
    detail = session.cell_detail(3, 2)
    assert detail["program"] is not None
    assert detail["program"]["live"] is False
    assert detail["program"]["inert_ticks_without_write"] == 1

    session.advance(1)
    detail = session.cell_detail(3, 2)
    assert detail["program"] is not None
    assert detail["program"]["live"] is False
    assert detail["program"]["inert_ticks_without_write"] == 0
    assert detail["program"]["size"] == 2


def test_export_and_restore_preserve_tick_and_programs():
    config = build_config("push 0\nappendAdj\nnop\n")
    config.system_params.inert_grace_ticks = 10
    session = SimulationSession.from_config(config)
    session.advance(2)
    archive = session.export_archive()

    restored = SimulationSession.from_archive(archive)

    assert restored.summary() == session.summary()
    assert restored.cell_detail(3, 2) == session.cell_detail(3, 2)
