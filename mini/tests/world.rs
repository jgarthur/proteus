//! Laws of the world: determinism, pairing, the speed of light, and mutation.

use std::collections::HashMap;

use proteus_mini::observe::{picture, Census};
use proteus_mini::{Head1, Rules, Schedule, World};

fn small(schedule: Schedule) -> Rules {
    Rules {
        width: 16,
        height: 16,
        schedule,
        ..Rules::default()
    }
}

fn world(rules: Rules) -> World {
    World::new(rules).unwrap()
}

fn advance(world: &mut World, ticks: u64) {
    for _ in 0..ticks {
        world.step();
    }
}

#[test]
fn a_world_is_a_pure_function_of_its_rules() {
    for schedule in [Schedule::Random, Schedule::Tiling] {
        let (mut a, mut b) = (world(small(schedule)), world(small(schedule)));
        let mut other = world(Rules {
            seed: 2,
            ..small(schedule)
        });
        advance(&mut a, 50);
        advance(&mut b, 50);
        advance(&mut other, 50);
        assert_eq!(a.digest(), b.digest());
        assert_ne!(a.digest(), other.digest());
    }
}

#[test]
fn random_pairs_are_disjoint_and_within_the_speed_of_light() {
    for light in [0, 1, 2, 3] {
        let mut w = world(Rules {
            light,
            steps: 0,
            ..small(Schedule::Random)
        });
        for _ in 0..20 {
            let pairs = w.pairs();
            let mut seen = vec![false; w.cells()];
            for &(a, b) in &pairs {
                assert!(!seen[a] && !seen[b] && a != b, "a cell paired twice");
                seen[a] = true;
                seen[b] = true;
                if light > 0 {
                    assert!(w.distance(a, b) <= light as usize);
                }
            }
            // A greedy random matching leaves some cells out, but not most.
            assert!(pairs.len() > w.cells() / 4);
            w.step();
        }
    }
}

#[test]
fn without_locality_distant_cells_meet() {
    let w = world(Rules {
        light: 0,
        ..small(Schedule::Random)
    });
    let furthest = w.pairs().iter().map(|&(a, b)| w.distance(a, b)).max();
    assert_eq!(furthest, Some(16));
}

#[test]
fn tiling_pairs_every_cell_with_each_neighbour_once_in_each_role() {
    let mut w = world(Rules {
        steps: 0,
        ..small(Schedule::Tiling)
    });
    let mut meetings: HashMap<(usize, usize), u32> = HashMap::new();
    for _ in 0..8 {
        let pairs = w.pairs();
        let mut seen = vec![false; w.cells()];
        for &(a, b) in &pairs {
            assert!(!seen[a] && !seen[b]);
            seen[a] = true;
            seen[b] = true;
            assert_eq!(w.distance(a, b), 1);
            *meetings.entry((a, b)).or_default() += 1;
        }
        assert!(seen.iter().all(|&paired| paired), "tiling left a cell out");
        w.step();
    }
    // Four neighbours, two roles: every ordered neighbour pair exactly once.
    assert_eq!(meetings.len(), w.cells() * 4);
    assert!(meetings.values().all(|&count| count == 1));
}

#[test]
fn tiling_rejects_worlds_it_cannot_tile() {
    for (width, light) in [(15, 1), (16, 2)] {
        let rules = Rules {
            width,
            light,
            ..small(Schedule::Tiling)
        };
        assert!(World::new(rules).is_err());
    }
}

/// Fills two worlds with tapes that halt at once, plants one replicator in
/// one of them, and checks that the difference spreads but never outruns light.
///
/// The quiet background matters. Head 0 starts on the first tape of a pair, so
/// a replicator that runs from the second tape overwrites itself with its
/// partner; a first tape that halts immediately never lets that happen.
#[test]
fn nothing_travels_faster_than_light() {
    for (schedule, light) in [
        (Schedule::Tiling, 1),
        (Schedule::Random, 1),
        (Schedule::Random, 2),
    ] {
        let rules = Rules {
            width: 32,
            height: 32,
            ops: "<>{}-+.,[]:".into(),
            head1: Head1::Second,
            light,
            schedule,
            ..Rules::default()
        };
        let (mut plain, mut seeded) = (world(rules.clone()), world(rules));
        for w in [&mut plain, &mut seeded] {
            for cell in 0..w.cells() {
                w.tape_mut(cell).fill(b'x');
                w.tape_mut(cell)[0] = b']';
            }
        }
        let origin = 16 * 32 + 16;
        seeded.tape_mut(origin)[..3].copy_from_slice(b"[:]");

        // Stop before the bound outgrows the torus and stops constraining anything.
        let mut furthest = vec![0];
        for tick in 1..=30 / light as usize {
            plain.step();
            seeded.step();
            let reached = (0..plain.cells())
                .filter(|&cell| plain.tape(cell) != seeded.tape(cell))
                .map(|cell| plain.distance(origin, cell))
                .max()
                .unwrap_or(0);
            assert!(reached <= tick * light as usize, "{schedule:?}: too fast");
            furthest.push(reached);
        }

        // The bound means nothing unless the difference actually spread, and
        // under the tiling it is tight: the first four ticks pair the origin
        // with a fresh neighbour each time, so the front moves at light speed.
        assert!(furthest.last() >= Some(&6), "{schedule:?}: no spread");
        if schedule == Schedule::Tiling {
            assert_eq!(furthest[..5], [0, 1, 2, 3, 4]);
        }
    }
}

fn changed_bytes(rules: Rules) -> (usize, usize) {
    let mut w = world(Rules { steps: 0, ..rules });
    let before = w.bytes().to_vec();
    w.step();
    let changed = before.iter().zip(w.bytes()).filter(|(a, b)| a != b).count();
    (changed, before.len())
}

#[test]
fn mutation_off_changes_nothing() {
    let (changed, _) = changed_bytes(Rules {
        mutation_log2: None,
        ..small(Schedule::Random)
    });
    assert_eq!(changed, 0);
}

#[test]
fn mutation_at_probability_one_redraws_every_byte() {
    // Tape lengths on both sides of the 64-byte mask word.
    for tape in [10, 64, 100] {
        let (changed, total) = changed_bytes(Rules {
            tape,
            mutation_log2: Some(0),
            ..small(Schedule::Random)
        });
        // A redrawn byte keeps its old value one time in 256.
        let kept = (total - changed) as f64 / total as f64;
        assert!(
            (kept - 1.0 / 256.0).abs() < 0.004,
            "tape {tape}: kept {kept}"
        );
    }
}

#[test]
fn mutation_follows_its_dyadic_rate() {
    for (k, tape) in [(2, 64), (4, 64), (4, 100), (6, 10)] {
        let (changed, total) = changed_bytes(Rules {
            tape,
            mutation_log2: Some(k),
            ..small(Schedule::Random)
        });
        let expected = total as f64 / f64::from(1 << k) * 255.0 / 256.0;
        let tolerance = 5.0 * expected.sqrt();
        assert!(
            (changed as f64 - expected).abs() < tolerance,
            "k = {k}, tape = {tape}: {changed} changed, expected about {expected:.0}"
        );
    }
}

#[test]
fn mutation_draws_fresh_noise_every_tick() {
    let mut w = world(Rules {
        steps: 0,
        mutation_log2: Some(4),
        ..small(Schedule::Random)
    });
    let before = w.bytes().to_vec();
    let changed = |w: &World| before.iter().zip(w.bytes()).filter(|(a, b)| a != b).count();
    w.step();
    let after_one = changed(&w);
    advance(&mut w, 7);
    // Eight independent ticks touch 1 - (15/16)^8 of the bytes, 6.4 times one
    // tick's share. Noise that repeated itself would touch nothing new.
    let ratio = changed(&w) as f64 / after_one as f64;
    assert!((ratio - 6.4).abs() < 0.5, "ratio {ratio}");
}

#[test]
fn mutation_reaches_every_position_on_a_long_tape() {
    let mut w = world(Rules {
        tape: 100,
        steps: 0,
        mutation_log2: Some(3),
        ..small(Schedule::Random)
    });
    let before = w.bytes().to_vec();
    advance(&mut w, 40);
    let mut touched = [false; 100];
    for (i, (a, b)) in before.iter().zip(w.bytes()).enumerate() {
        touched[i % 100] |= a != b;
    }
    assert!(touched.iter().all(|&hit| hit));
}

#[test]
fn a_fresh_soup_has_no_structure_and_a_monoculture_is_all_structure() {
    let mut w = world(Rules::default());
    let soup = Census::of(&w);
    assert!(soup.shannon > 7.99);
    assert!(soup.high_order().abs() < 0.05);
    assert!((soup.op_density - 10.0 / 256.0).abs() < 0.005);

    let program: Vec<u8> = (0..64).map(|i| b"[[{.>]-]"[i % 8]).collect();
    for cell in 0..w.cells() {
        w.tape_mut(cell).copy_from_slice(&program);
    }
    let monoculture = Census::of(&w);
    assert!(monoculture.high_order() > 2.0);
    assert_eq!(monoculture.kinds.len(), 1);
    assert_eq!(monoculture.top_share(), 1.0);
}

#[test]
fn a_picture_is_a_png_of_the_requested_size() {
    let w = world(small(Schedule::Random));
    let png = picture(&w, 3);
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(&png[16..24], [0, 0, 0, 48, 0, 0, 0, 48]);
    assert_eq!(&png[png.len() - 8..png.len() - 4], b"IEND");
}
