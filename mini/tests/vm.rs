//! Interpreter semantics, checked against BFF's published behaviour.

use proteus_mini::vm::{run, OpTable, Run, BFF};

const ALL: &str = "<>{}-+.,[]:";

/// Runs `tape` under `ops` and returns the final tape with the run's counts.
fn exec(ops: &str, tape: &[u8], steps: u32, head1: usize) -> (Vec<u8>, Run) {
    let mut tape = tape.to_vec();
    let counts = run(&mut tape, &OpTable::parse(ops).unwrap(), steps, head1);
    (tape, counts)
}

#[test]
fn each_instruction_does_what_its_name_says() {
    // (program, where head 1 starts, tape afterwards)
    let cases: [(&[u8], usize, &[u8]); 8] = [
        (b">+", 0, b">,"),   // head 0 right, then '+' (43) becomes ',' (44)
        (b">-", 0, b">,"),   // head 0 right, then '-' (45) becomes ',' (44)
        (b"<-", 0, b"<,"),   // head 0 wraps left onto the last byte
        (b"}.", 0, b"}}"),   // head 1 right, then put head 0's byte there
        (b"{.x", 0, b"{.{"), // head 1 wraps left onto the last byte
        (b">,", 0, b">>"),   // head 0 right, then get head 1's byte
        (b".x", 1, b".."),   // head 1 can start elsewhere
        (b":ab", 1, b":::"), // copy advances both heads, so it chases itself
    ];
    for (program, head1, expected) in cases {
        let (tape, _) = exec(ALL, program, 100, head1);
        assert_eq!(
            tape,
            expected,
            "program {:?}",
            String::from_utf8_lossy(program)
        );
    }
}

#[test]
fn loop_runs_until_the_byte_under_head_0_is_zero() {
    let (tape, counts) = exec(BFF, b"<[-]\x03", 100, 0);
    assert_eq!(tape, b"<[-]\x00");
    // '<', '[', three rounds of '-' and ']', then the data byte.
    assert_eq!(counts, Run { reads: 9, ops: 8 });
}

#[test]
fn loop_is_skipped_when_the_byte_under_head_0_is_zero() {
    let (tape, counts) = exec(BFF, b"<[-]+\x00", 100, 0);
    assert_eq!(tape, b"<[-]+\x01");
    // Skipping reads '<', '[', '+', and the data byte; entering would read more.
    assert_eq!(counts.reads, 4);
}

#[test]
fn brackets_nest() {
    let (tape, _) = exec(BFF, b"<[[-]]\x02", 100, 0);
    assert_eq!(tape, b"<[[-]]\x00");

    // A skip must land past the outer ']', not the inner one.
    let (tape, counts) = exec(BFF, b"<[[-]]+\x00", 100, 0);
    assert_eq!(tape, b"<[[-]]+\x01");
    assert_eq!(counts.reads, 4);
}

#[test]
fn a_bracket_without_a_partner_halts_the_run() {
    // ']' with a nonzero byte under head 0 and no '[' behind it.
    let (tape, counts) = exec(BFF, b"+]+", 100, 0);
    assert_eq!(tape, b",]+");
    assert_eq!(counts.reads, 2);

    // '[' with a zero byte under head 0 and no ']' ahead of it.
    let (tape, counts) = exec(BFF, b"<[+\x00", 100, 0);
    assert_eq!(tape, b"<[+\x00");
    assert_eq!(counts.reads, 2);
}

#[test]
fn a_bracket_that_does_not_jump_needs_no_partner() {
    let (tape, counts) = exec(BFF, b"<]+\x00", 100, 0);
    assert_eq!(tape, b"<]+\x01");
    assert_eq!(counts.reads, 4);
}

#[test]
fn the_step_limit_counts_inert_bytes_too() {
    let (_, counts) = exec(BFF, &[0; 8], 3, 0);
    assert_eq!(counts, Run { reads: 3, ops: 0 });

    let (_, counts) = exec(BFF, b"[]", 1000, 0);
    assert_eq!(
        counts,
        Run {
            reads: 1000,
            ops: 1000
        }
    );
}

#[test]
fn a_run_ends_when_the_pointer_leaves_the_tape() {
    let (_, counts) = exec(BFF, b"xx>x", 1000, 0);
    assert_eq!(counts, Run { reads: 4, ops: 1 });
}

#[test]
fn instructions_outside_the_set_are_inert() {
    let (tape, counts) = exec("+", b"[+]", 100, 0);
    assert_eq!(tape, b"\\+]");
    assert_eq!(counts, Run { reads: 3, ops: 1 });

    assert!(OpTable::parse("<>?").is_err());
}

/// The replicator in figure 4 of the BFF paper: it writes itself backwards
/// into the other tape, and reads the same in both directions.
fn bff_replicator() -> Vec<u8> {
    let mut tape = b"[[{.>]-]".to_vec();
    tape.resize(56, b' ');
    tape.extend(b"]-]>.{[[");
    tape
}

#[test]
fn the_published_bff_replicator_copies_itself_onto_its_partner() {
    let replicator = bff_replicator();
    let other: Vec<u8> = (0..64).collect();
    let (tape, _) = exec(BFF, &[replicator.clone(), other].concat(), 1 << 13, 0);
    assert_eq!(&tape[..64], replicator);
    assert_eq!(&tape[64..], replicator);
}

#[test]
fn three_bytes_replicate_when_copy_exists_and_head_1_starts_on_the_second_tape() {
    let mut replicator = b"[:]".to_vec();
    replicator.resize(64, b'x');
    let other: Vec<u8> = (0..64).collect();
    let (tape, _) = exec(ALL, &[replicator.clone(), other].concat(), 1 << 13, 64);
    assert_eq!(&tape[..64], replicator);
    assert_eq!(&tape[64..], replicator);
}
