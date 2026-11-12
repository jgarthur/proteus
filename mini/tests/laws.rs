//! Each law of the world, pinned by a test.

use proteus_mini::code::{self, decode, Op};
use proteus_mini::{Body, Laws, Start, Tally, World};

/// Laws with nothing random or costly: no arrivals, decay, upkeep or mutation.
fn quiet() -> Laws {
    Laws {
        width: 8,
        height: 8,
        energy_rate: 0.0,
        mass_rate: 0.0,
        decay_log2: None,
        upkeep_log2: None,
        slip_log2: None,
        miscopy_log2: None,
        seed: 1,
    }
}

const HOME: usize = 4 * 8 + 4;
const EAST: usize = HOME + 1;
const SOUTH: usize = HOME + 8;

/// Builds a quiet world holding one program at `HOME`, facing east.
fn lone(laws: Laws, spelling: &str) -> World {
    let mut world = World::new(laws, Start::Seed(code::parse(spelling).unwrap())).unwrap();
    body(&mut world, HOME).dir = 0;
    world
}

fn body(world: &mut World, cell: usize) -> &mut Body {
    world.cell_mut(cell).body.as_mut().expect("occupied")
}

fn peek(world: &World, cell: usize) -> &Body {
    world.cells()[cell].body.as_ref().expect("occupied")
}

fn spelled(world: &World, cell: usize) -> String {
    code::spell(&peek(world, cell).code)
}

fn run(world: &mut World, ticks: u64) -> Tally {
    let mut tally = Tally::default();
    for _ in 0..ticks {
        tally += world.step();
    }
    tally
}

/// Gives the program at `cell` plenty to spend.
fn fund(world: &mut World, cell: usize) {
    body(world, cell).energy = 1000;
    body(world, cell).mass = 1000;
}

#[test]
fn half_of_all_instruction_values_are_no_ops() {
    let no_ops = (0..code::VALUES).filter(|&value| decode(value) == Op::Nop);
    assert_eq!(no_ops.count() * 2, usize::from(code::VALUES));
    assert_eq!(code::spell(&code::parse("ACKTLWDS.").unwrap()), "ACKTLWDS.");
    assert!(code::parse("AXK").is_err());
}

#[test]
fn absorb_and_collect_take_everything_loose_in_the_cell() {
    let mut world = lone(quiet(), "AC");
    world.cell_mut(HOME).energy = 7;
    world.cell_mut(HOME).mass = 5;
    world.step();
    assert_eq!(
        (peek(&world, HOME).energy, world.cells()[HOME].energy),
        (7, 0)
    );
    assert_eq!((peek(&world, HOME).mass, world.cells()[HOME].mass), (0, 5));
    world.step();
    assert_eq!((peek(&world, HOME).mass, world.cells()[HOME].mass), (5, 0));
}

#[test]
fn turn_goes_east_south_west_north() {
    let mut world = lone(quiet(), "T");
    let mut seen = Vec::new();
    for _ in 0..4 {
        seen.push(world.neighbour(HOME, peek(&world, HOME).dir));
        world.step();
    }
    assert_eq!(seen, [HOME + 1, HOME + 8, HOME - 1, HOME - 8]);
}

#[test]
fn look_reports_the_size_of_a_live_neighbour_only() {
    // Nothing there.
    let mut world = lone(quiet(), "L");
    body(&mut world, HOME).a = 9;
    world.step();
    assert_eq!(peek(&world, HOME).a, 0);

    // A live program of three instructions.
    world.place(EAST, code::parse("...").unwrap()).unwrap();
    world.step();
    assert_eq!(peek(&world, HOME).a, 3);

    // A child still under construction does not count.
    body(&mut world, EAST).live = false;
    world.step();
    assert_eq!(peek(&world, HOME).a, 0);

    // The register cannot hold more than its width allows.
    world.place(EAST, vec![8; 40]).unwrap();
    world.step();
    assert_eq!(peek(&world, HOME).a, code::A_MAX);
}

#[test]
fn weigh_reports_held_mass_up_to_the_width_of_the_register() {
    let mut world = lone(quiet(), "W");
    body(&mut world, HOME).mass = 3;
    world.step();
    assert_eq!(peek(&world, HOME).a, 3);
    body(&mut world, HOME).mass = 500;
    world.step();
    assert_eq!(peek(&world, HOME).a, code::A_MAX);
}

#[test]
fn dec_counts_down_and_stops_at_zero() {
    let mut world = lone(quiet(), "D");
    body(&mut world, HOME).a = 2;
    let mut seen = Vec::new();
    for _ in 0..4 {
        world.step();
        seen.push(peek(&world, HOME).a);
    }
    assert_eq!(seen, [1, 0, 0, 0]);
}

#[test]
fn skip_passes_over_the_next_instruction_only_when_a_is_zero() {
    // A is 0: the turn is skipped.
    let mut world = lone(quiet(), "ST.");
    world.step();
    assert_eq!((peek(&world, HOME).ip, peek(&world, HOME).dir), (2, 0));

    // A is not 0: the turn runs.
    let mut world = lone(quiet(), "ST.");
    body(&mut world, HOME).a = 1;
    run(&mut world, 2);
    assert_eq!((peek(&world, HOME).ip, peek(&world, HOME).dir), (2, 1));

    // Code is a loop, so a skip at the end passes over the first instruction.
    let mut world = lone(quiet(), "T.S");
    run(&mut world, 3);
    assert_eq!((peek(&world, HOME).ip, peek(&world, HOME).dir), (1, 1));
}

#[test]
fn a_program_copies_itself_exactly_and_the_child_wakes_when_complete() {
    // `KLST` copies, looks ahead, and turns only if a live program is there.
    // While the child is unfinished the turn is skipped, so a lap is 3 ticks.
    let mut world = lone(quiet(), "KLST");
    fund(&mut world, HOME);
    let before = world.mass();

    run(&mut world, 9);
    let child = peek(&world, EAST);
    assert_eq!(
        (code::spell(&child.code), child.live, child.ip),
        ("KLS".into(), false, 0)
    );
    assert_eq!(peek(&world, HOME).child, Some(EAST));

    // The fourth copy completes it. It wakes for the following tick.
    let tally = run(&mut world, 1);
    let child = peek(&world, EAST);
    assert_eq!(
        (code::spell(&child.code), child.live),
        ("KLST".into(), true)
    );
    assert_eq!((child.generation, child.born), (1, 10));
    assert_eq!((child.energy, child.mass, child.a), (0, 0, 0));
    assert_eq!((tally.births, tally.copies), (1, 1));

    let parent = peek(&world, HOME);
    assert_eq!((parent.children, parent.child, parent.cursor), (1, None, 0));
    assert_eq!((parent.energy, parent.mass), (996, 996));
    assert_eq!(world.mass(), before);

    // Now the way east is blocked by a live program, so the parent turns.
    run(&mut world, 3);
    assert_eq!(peek(&world, HOME).dir, 1);
}

#[test]
fn a_copy_that_cannot_be_paid_for_waits_without_losing_its_place() {
    let mut world = lone(quiet(), "KK.T");
    body(&mut world, HOME).energy = 1000;
    body(&mut world, HOME).mass = 2;
    run(&mut world, 40);
    assert_eq!(spelled(&world, EAST), "KK");
    assert!(!peek(&world, EAST).live);

    // Out of energy is the same as out of mass.
    body(&mut world, HOME).energy = 0;
    body(&mut world, HOME).mass = 1000;
    run(&mut world, 40);
    assert_eq!(spelled(&world, EAST), "KK");

    body(&mut world, HOME).energy = 1000;
    run(&mut world, 8);
    assert_eq!(spelled(&world, EAST), "KK.T");
    assert!(peek(&world, EAST).live);
}

#[test]
fn a_live_neighbour_cannot_be_copied_into() {
    let mut world = lone(quiet(), "K");
    fund(&mut world, HOME);
    world.place(EAST, code::parse("..").unwrap()).unwrap();
    run(&mut world, 10);
    assert_eq!(spelled(&world, EAST), "..");
    assert_eq!(
        (peek(&world, HOME).mass, peek(&world, HOME).child),
        (1000, None)
    );
}

#[test]
fn only_its_builder_can_extend_a_child() {
    // HOME faces east. A second program two cells east faces west, into the
    // same cell, and first copies on the fourth tick.
    let mut world = lone(quiet(), "KKKKK");
    fund(&mut world, HOME);
    world.place(EAST + 1, code::parse("...K").unwrap()).unwrap();
    fund(&mut world, EAST + 1);
    body(&mut world, EAST + 1).dir = 2;
    run(&mut world, 3);
    assert_eq!(spelled(&world, EAST), "KKK");

    // Tick 4: both copy. Only the builder's instruction lands.
    run(&mut world, 1);
    assert_eq!(spelled(&world, EAST), "KKKK");
    assert_eq!(peek(&world, EAST + 1).mass, 1000);
    assert_eq!(peek(&world, EAST + 1).child, None);
}

#[test]
fn an_empty_cell_goes_to_exactly_one_of_the_programs_that_want_it() {
    let mut winners = [0, 0];
    for seed in 0..40 {
        let mut world = lone(Laws { seed, ..quiet() }, "K.");
        fund(&mut world, HOME);
        world.place(EAST + 1, code::parse("KT").unwrap()).unwrap();
        fund(&mut world, EAST + 1);
        body(&mut world, EAST + 1).dir = 2;
        world.step();

        let child = spelled(&world, EAST);
        assert_eq!(child, "K");
        let paid = [HOME, EAST + 1].map(|cell| peek(&world, cell).mass == 999);
        assert_eq!(paid.iter().filter(|&&p| p).count(), 1, "exactly one pays");
        let builders = [HOME, EAST + 1].map(|cell| peek(&world, cell).child == Some(EAST));
        assert_eq!(builders, paid);
        winners[usize::from(paid[1])] += 1;
    }
    // The contest is a fair draw, so over forty worlds both sides win often.
    assert!(winners.iter().all(|&wins| wins >= 10), "{winners:?}");
}

#[test]
fn starting_a_child_elsewhere_abandons_the_unfinished_one() {
    let mut world = lone(quiet(), "KT...");
    fund(&mut world, HOME);
    run(&mut world, 1);
    assert_eq!(spelled(&world, EAST), "K");
    let before = world.mass();

    // One lap later the program faces south and copies there.
    run(&mut world, 5);
    assert!(world.cells()[EAST].body.is_none());
    assert_eq!(
        world.cells()[EAST].mass,
        1,
        "the abandoned body returns to its cell"
    );
    assert_eq!(spelled(&world, SOUTH), "K");
    assert_eq!(peek(&world, HOME).child, Some(SOUTH));
    assert_eq!(world.mass(), before);
}

#[test]
fn certain_upkeep_dissolves_an_unfed_program_and_returns_all_it_had() {
    let laws = Laws {
        upkeep_log2: Some(0),
        ..quiet()
    };
    let mut world = lone(laws, "T..");
    body(&mut world, HOME).energy = 7;
    body(&mut world, HOME).mass = 5;
    let before = world.mass();

    // Three instructions cost three energy a tick: two ticks are paid in full.
    run(&mut world, 2);
    assert_eq!(
        (spelled(&world, HOME), peek(&world, HOME).energy),
        ("T..".into(), 1)
    );

    // On the third tick one unit is paid and two instructions dissolve.
    run(&mut world, 1);
    assert_eq!(
        (spelled(&world, HOME), world.cells()[HOME].mass),
        ("T".into(), 2)
    );

    // On the fourth the last one goes, and the held mass goes with it.
    let tally = run(&mut world, 1);
    assert!(world.cells()[HOME].body.is_none());
    assert_eq!(world.cells()[HOME].mass, 3 + 5);
    assert_eq!((tally.deaths, tally.lifespans, tally.parents), (1, 3, 0));
    assert_eq!(world.mass(), before);
}

#[test]
fn chance_upkeep_wears_an_unfed_program_away_at_its_rate() {
    // Sixteen instructions at 2^-3 per tick: two dissolve per tick at first.
    let mut lost_in_first_tick = 0;
    let mut lifespans = 0;
    for seed in 0..200 {
        let laws = Laws {
            upkeep_log2: Some(3),
            seed,
            ..quiet()
        };
        let mut world = lone(laws, "................");
        world.step();
        lost_in_first_tick += 16 - peek(&world, HOME).code.len();
        lifespans += run(&mut world, 400).lifespans;
        assert!(world.cells()[HOME].body.is_none());
        assert_eq!(world.cells()[HOME].mass, 16);
    }
    assert!(
        (350..450).contains(&lost_in_first_tick),
        "{lost_in_first_tick}"
    );
    // The last of sixteen lasts 8 * (1 + 1/2 + ... + 1/16), about 27 ticks.
    assert!((4800..6000).contains(&lifespans), "{lifespans}");
}

#[test]
fn a_dying_builder_takes_its_unfinished_child_with_it() {
    let laws = Laws {
        upkeep_log2: Some(0),
        ..quiet()
    };
    let mut world = lone(laws, "K.");
    body(&mut world, HOME).energy = 3;
    body(&mut world, HOME).mass = 1;
    let before = world.mass();
    run(&mut world, 1);
    assert_eq!(spelled(&world, EAST), "K");
    run(&mut world, 1);
    assert!(world.cells()[HOME].body.is_none());
    assert!(world.cells()[EAST].body.is_none());
    assert_eq!((world.cells()[HOME].mass, world.cells()[EAST].mass), (2, 1));
    assert_eq!(world.mass(), before);
}

#[test]
fn a_shrinking_builder_finishes_with_what_it_has_left() {
    // Upkeep removes the last instruction while a child is half built. The
    // cursor then stands at the end of the code, and the next copy just
    // completes the child without writing anything.
    let mut world = lone(quiet(), "K..");
    fund(&mut world, HOME);
    run(&mut world, 4);
    assert_eq!(spelled(&world, EAST), "K.");
    body(&mut world, HOME).code.truncate(2);
    body(&mut world, HOME).ip = 0;
    run(&mut world, 1);
    assert_eq!(spelled(&world, EAST), "K.");
    assert!(peek(&world, EAST).live);
    assert_eq!(peek(&world, HOME).mass, 998);
}

#[test]
fn an_execution_slip_always_changes_the_instruction_it_hits() {
    let laws = Laws {
        slip_log2: Some(0),
        ..quiet()
    };
    let mut world = lone(laws, "TTTT");
    let tally = run(&mut world, 2);
    assert_eq!(tally.slips, 2);
    let code = &peek(&world, HOME).code;
    assert!(
        code[0] != 3 && code[1] != 3,
        "the two executed instructions changed"
    );
    assert_eq!(code[2..], [3, 3], "the others did not");
}

#[test]
fn execution_slips_follow_their_rate_and_land_everywhere() {
    let laws = Laws {
        slip_log2: Some(4),
        ..quiet()
    };
    let mut world = World::new(laws, Start::Soup(8)).unwrap();
    let original: Vec<Vec<u8>> = world
        .cells()
        .iter()
        .map(|c| c.body.clone().unwrap().code)
        .collect();
    // 64 programs, 800 ticks: 51,200 executions at 1 in 16.
    let tally = run(&mut world, 800);
    assert!((2900..3500).contains(&tally.slips), "{}", tally.slips);

    let mut produced = [false; 16];
    for (cell, was) in world.cells().iter().zip(&original) {
        let now = &cell.body.as_ref().unwrap().code;
        for (a, b) in now.iter().zip(was) {
            produced[usize::from(*a)] |= a != b;
        }
    }
    assert!(
        produced.iter().all(|&seen| seen),
        "a slip can produce any value"
    );
}

#[test]
fn without_mutation_every_program_is_an_exact_copy() {
    let laws = Laws {
        width: 24,
        height: 24,
        slip_log2: None,
        miscopy_log2: None,
        ..Laws::default()
    };
    let seed = code::parse("ACKLST").unwrap();
    let mut world = World::new(laws, Start::Seed(seed.clone())).unwrap();
    let tally = run(&mut world, 4000);
    assert_eq!((tally.slips, tally.miscopies), (0, 0));
    assert!(tally.births > 200, "{}", tally.births);
    for body in world.cells().iter().filter_map(|cell| cell.body.as_ref()) {
        // Upkeep may have shortened a starving program, and a child may be
        // half built, but no instruction is ever out of place.
        assert!(seed.starts_with(&body.code), "{}", code::spell(&body.code));
    }
}

#[test]
fn every_copy_can_go_wrong_in_three_ways() {
    let laws = Laws {
        miscopy_log2: Some(0),
        ..quiet()
    };
    let (mut shorter, mut longer, mut changed) = (0, 0, 0);
    for seed in 0..60 {
        let mut world = lone(
            Laws {
                seed,
                ..laws.clone()
            },
            "KLSDWW",
        );
        fund(&mut world, HOME);
        let before = world.mass();
        let mut tally = Tally::default();
        while tally.births == 0 {
            tally += world.step();
        }
        let child = &peek(&world, EAST).code;
        shorter += usize::from(child.len() < 6);
        longer += usize::from(child.len() > 6);
        changed += usize::from(child.len() == 6 && code::spell(child) != "KLSDWW");
        // Every successful copy erred, and each wrote at most one instruction.
        assert!(tally.miscopies >= child.len() as u64);
        assert_eq!(1000 - peek(&world, HOME).mass, child.len() as u32);
        assert_eq!(world.mass(), before);
    }
    assert!(
        shorter > 5 && longer > 5 && changed > 5,
        "{shorter} {longer} {changed}"
    );
}

#[test]
fn copy_errors_follow_their_rate() {
    let laws = Laws {
        width: 24,
        height: 24,
        slip_log2: None,
        miscopy_log2: Some(3),
        ..Laws::default()
    };
    let start = Start::Seed(code::parse("ACKLST").unwrap());
    let mut world = World::new(laws, start).unwrap();
    let tally = run(&mut world, 4000);
    assert!(tally.copies > 2000, "{}", tally.copies);
    let expected = tally.copies as f64 / 8.0;
    assert!((tally.miscopies as f64 - expected).abs() < 4.0 * expected.sqrt());
}

#[test]
fn loose_energy_and_mass_dissipate_wherever_they_are() {
    let laws = Laws {
        decay_log2: Some(2),
        ..quiet()
    };
    let mut world = lone(laws, ".");
    world.cell_mut(HOME).energy = 4000;
    world.cell_mut(HOME).mass = 4000;
    body(&mut world, HOME).energy = 4000;
    body(&mut world, HOME).mass = 4000;
    let before = world.mass();
    let tally = world.step();
    let left = [
        world.cells()[HOME].energy,
        world.cells()[HOME].mass,
        peek(&world, HOME).energy,
        peek(&world, HOME).mass,
    ];
    // A quarter of 4000 goes, give or take four standard deviations.
    assert!(
        left.iter().all(|&amount| (2890..3110).contains(&amount)),
        "{left:?}"
    );
    assert_eq!(before - world.mass(), tally.mass_out);
    assert_eq!(
        peek(&world, HOME).code.len(),
        1,
        "instructions do not dissipate"
    );
}

#[test]
fn energy_and_mass_arrive_at_their_rates() {
    let laws = Laws {
        energy_rate: 0.5,
        mass_rate: 2.0,
        ..quiet()
    };
    let mut world = lone(laws, ".");
    let tally = run(&mut world, 500);
    let cells = world.cells();
    let energy: u32 = cells.iter().map(|cell| cell.energy).sum();
    let mass: u32 = cells.iter().map(|cell| cell.mass).sum();
    // 64 cells for 500 ticks: 16,000 and 64,000 expected.
    assert!((15_500..16_500).contains(&energy), "{energy}");
    assert!((63_000..65_000).contains(&mass), "{mass}");
    assert_eq!(u64::from(mass), tally.mass_in);
}

#[test]
fn a_fresh_world_starts_at_the_level_its_cells_settle_to() {
    let world = World::new(Laws::default(), Start::Seed(vec![0])).unwrap();
    let empty = &world.cells()[0];
    assert_eq!((empty.energy, empty.mass), (32, 6));
}

#[test]
fn mass_changes_only_by_arrival_and_decay() {
    for start in [Start::Seed(code::parse("ACKLST").unwrap()), Start::Soup(6)] {
        let laws = Laws {
            width: 16,
            height: 16,
            mass_rate: 0.25,
            slip_log2: Some(9),
            miscopy_log2: Some(4),
            seed: 2,
            ..Laws::default()
        };
        let mut world = World::new(laws, start).unwrap();
        let mut activity = Tally::default();
        for _ in 0..6000 {
            let before = world.mass();
            let tally = world.step();
            assert_eq!(world.mass(), before + tally.mass_in - tally.mass_out);
            activity += tally;
        }
        // The check means little unless the world was busy while it ran.
        assert!(
            activity.copies > 500 && activity.deaths > 100,
            "{activity:?}"
        );
        assert!(
            activity.slips > 100 && activity.miscopies > 50,
            "{activity:?}"
        );
    }
}

#[test]
fn a_world_is_a_pure_function_of_its_laws() {
    let start = || Start::Seed(code::parse("ACKLST").unwrap());
    let laws = Laws {
        width: 16,
        height: 16,
        ..Laws::default()
    };
    let mut a = World::new(laws.clone(), start()).unwrap();
    let mut b = World::new(laws.clone(), start()).unwrap();
    let mut other = World::new(Laws { seed: 2, ..laws }, start()).unwrap();
    run(&mut a, 3000);
    run(&mut b, 3000);
    run(&mut other, 3000);
    assert_eq!(a.digest(), b.digest());
    assert_ne!(a.digest(), other.digest());
}

/// Runs two worlds that differ only in whether a seed program exists, and
/// checks that the difference spreads but never by more than a cell per tick.
#[test]
fn nothing_travels_faster_than_light() {
    let laws = Laws {
        width: 32,
        height: 32,
        ..Laws::default()
    };
    let start = Start::Seed(code::parse("ACKLST").unwrap());
    let mut seeded = World::new(laws.clone(), start.clone()).unwrap();
    let mut empty = World::new(laws, start).unwrap();
    let origin = 16 * 32 + 16;
    empty.cell_mut(origin).body = None;

    let mut furthest = 0;
    for tick in 1..=30 {
        seeded.step();
        empty.step();
        let reached = (0..seeded.cells().len())
            .filter(|&cell| seeded.cells()[cell] != empty.cells()[cell])
            .map(|cell| seeded.distance(origin, cell))
            .max()
            .unwrap_or(0);
        assert!(reached <= tick, "tick {tick}: reached {reached}");
        furthest = reached;
    }
    // The first child is being built next door by now.
    assert!(furthest >= 1);
}
