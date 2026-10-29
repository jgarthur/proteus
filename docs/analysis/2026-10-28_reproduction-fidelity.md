# Reproduction and genome fidelity in Proteus v0.4.0

- **Question and scope:** Why do mobile seed genomes change? Proteus v0.4.0 audits of Squarestep, Wildstep and Radchain in their stated 64×64 regimes, plus targeted action traces.
- **Answer:** Failed appends followed by unconditional boots produce incomplete offspring; multiple programs can contribute to or boot one body; live programs can receive further appends. These mechanisms occur with zero observed mutations and maintenance disabled.
- **Meaning:** Construction, persistence and exact sequence copying are distinct. A successful boot does not establish reproduction; a mixed-contributor body can nevertheless match its creator exactly.
- **Limits:** Cross-organism comparisons include environmental differences. These experiments identify copying failures, not the minimum complexity or evolutionary accessibility of reliable reproduction.
- **Disposition:** Closed; the [Guardstep follow-up](2026-10-29_reliable-copying.md) tested an engineered remedy. [Compact evidence](reproduction-fidelity/README.md) is retained; original bulk archives are unavailable. Reopen for a new causal claim, not to reconstruct discarded runs.

[Chapter synthesis](README.md#copying-and-guardstep-chapter-closed).

## Method and evidence

`proteus-audit` runs the same engine driver as ordinary execution. Read-only
callbacks separate local edits, nonlocal edits, maintenance and mutation. The
recorder preserves every boot's exact genome and construction history, sampled
lineage/genomes, and selected detailed action/event traces. It checks birth,
death and mutation totals against the engine and verifies a genome-byte ledger
on every tick. The [tool guide](../GENOME-OBSERVATION.md) defines the artifacts
and observation boundaries.

All worlds are 64×64 and start with one organism at `(32,32)`. World seeds are
1, 2 and 3. The three organisms use their respective verified regimes from the
[seed-organism study](2026-08-23_seed-organisms.md):

| Organism | Bytes | Energy / mass arrival rates | Local exponent | Maintenance exponent k | Root free energy / mass |
| --- | ---: | --- | ---: | ---: | --- |
| Squarestep | 9 | 0.5 / 0.3 | 0.55 | 14 | 0 / 0 |
| Wildstep | 11 | 0.25 / 0.2 | 0.5 | 14 | 16 / 16 |
| Radchain | 7 | 2 / 1.25 | 0.6 | 10 | 0 / 0 |

All use decay exponents 7, storage cap 4, maintenance size exponent 1,
synthesis surcharge 1, inert grace 10, and no spontaneous spawning.
Cross-organism comparisons therefore also include environmental differences.

The factorial matrix changes two inputs independently: the stated maintenance
rate versus `null`, and mutation exponents `63/63` versus `16/8`. Exponents
`63/63` make mutation negligible, not impossible; the recorded counts establish
which runs actually had zero mutations. Each condition uses the same three
world seeds. Parameter interventions preserve the engine's seed/tick/cell noise
field, while changed state can affect later execution and sampling decisions.

The matrix runs for 5,000 ticks with 250-tick samples. Reference and
no-maintenance conditions also run to 20,000 ticks with 1,000-tick samples.
Detailed mobile-organism traces cover ticks `[0,150)` without maintenance.
Driftkin traces cover `[0,1000)` under its frontend regime. All outcomes, including
unfinished constructions, remain in the [data tables](reproduction-fidelity/README.md).

## Incomplete copying precedes mutation and maintenance

In the Squarestep seed-1 trace, creator uid `385159` is itself booted as an exact
nine-byte Squarestep at tick 32. It attempts to build a child at cell 2145:

| Executed tick | Attempt | Outcome and relevant state |
| ---: | --- | --- |
| 34 | Append `83` (`collect`) | Fails: target uid `24711` is protected. That target moves away later in the same Pass 2. The source has 7 free mass. |
| 35–41 | Append `66,48,85,95,49,100,64` | All seven succeed, creating and extending child uid `582023`. |
| 42 | Append `99` (`move`) | Fails: the inert target is open, but the source has zero free mass. |
| 43 | Boot | Succeeds with genome `[66,48,85,95,49,100,64]`. |

The creator has not changed, one actor supplied the body, no maintenance was
charged, and no mutation occurred. Its counted copy loop advances after failed
appends and does not retry the omitted bytes. `boot` accepts the incomplete inert
body. These outcomes follow SPEC-COMPANION §7's append, movement and boot rules;
they are not observer-induced execution changes.

The same restricted class of unmatched birth occurs in all six early traces:

| Organism | Seed 1: first tick / count in 150 ticks | Seed 2 | Seed 3 |
| --- | --- | --- | --- |
| Squarestep | 43 / 21 | 66 / 22 | 67 / 16 |
| Wildstep | 79 / 11 | 66 / 9 | 66 / 8 |

Here the restriction means one contributor, only the creator booting, no observed
creator changes, and no body maintenance loss. It isolates copying failures that
remain after excluding those other mechanisms. The [selected rows](reproduction-fidelity/cases.json)
and [per-seed identifiers](reproduction-fidelity/early-copy-failures.json) make the
cases directly inspectable.

## Shared construction can preserve or change the sequence

In that same zero-mutation, no-maintenance Squarestep trace, body uid `794491`
receives bytes from actors `614271` and `418171` over ticks 48–56. The resulting
nine bytes exactly match Squarestep. Both actors boot it at tick 57. Counting
contributors alone would incorrectly classify this exact sequence as a corrupted
copy.

Body uid `941699` shows the other outcome. Actors `761479` and `385159` interleave
appends over ticks 57–66. Actor `385159`, which did not create the first byte,
boots it at tick 67. Its sequence is
`[83,66,48,85,85,95,49,64,64,99]`: ten bytes assembled from two programs.
A creation-parent tree alone would hide the second contributor and the different
booter. Both cases are retained in [cases.json](reproduction-fidelity/cases.json).

## Live programs remain editable

At tick 124, actor `598399` appends byte `49` to already-live uid `24711`, extending
it from nine to ten bytes. This changes the genome after its birth comparison.
Consequently, an exact genome at boot and persistence of that genome later in
life are different measurements.

The long-run table below quantifies this pathway alongside mutation and
maintenance. Because local instructions cannot append to their own code, every
recorded size increase comes from a successful neighbor append or one-byte spawn;
spawning is disabled in these experiments. Maintenance and deletion can remove
bytes, and mutation can alter existing bytes.

All 18 long runs record zero mutations. Ranges below span seeds 1–3;
comma-separated counts preserve seed order. The late birth window is world
ticks 19,000–20,000.

| Organism | Maintenance | Live programs | Exact starting genomes (seeds 1,2,3) | Unique live genomes | Mean live size, bytes | Boots in final 1,000 ticks (seeds 1,2,3) |
| --- | --- | ---: | --- | ---: | ---: | --- |
| Squarestep | k=14 | 4,095–4,096 | 0, 0, 1 | 3,862–3,913 | 765.2–804.0 | 2, 0, 2 |
| Squarestep | off | 4,095–4,096 | 0, 0, 1 | 3,862–3,911 | 902.1–944.1 | 0, 0, 0 |
| Wildstep | k=14 | 4,095–4,096 | 0, 0, 1 | 3,935–3,949 | 609.0–624.9 | 2, 2, 1 |
| Wildstep | off | 4,096 | 0, 1, 0 | 3,932–3,943 | 833.1–857.1 | 0, 0, 0 |
| Radchain | k=10 | 4–7 | 7, 4, 6 | 1 | 7.0 | 0, 0, 0 |
| Radchain | off | 4–7 | 7, 4, 6 | 1 | 7.0 | 0, 0, 0 |

Squarestep seed 1 records **3,710,655 appends to live programs** and
395,400 bytes removed by maintenance over 20,000 ticks, with zero mutations.
Its final 3,893 unique live genomes and mean live size of
803.978027 bytes reproduce the earlier seed-organism result. Disabling
maintenance also removes almost every starting sequence and produces larger mean
genomes in every paired mobile-organism seed. Maintenance trims genomes and
creates vacancies; it is not necessary for the loss of copying fidelity observed
during growth.

At full occupancy, no-maintenance worlds have no empty cells for new construction;
any further boot must use an already-present inert body. The late birth count is
therefore constrained by space, and is not a standalone test of reproductive
competence. The early traces establish copying errors during growth. The long
runs show continued genome editing after colonization. See the complete
[per-seed results](reproduction-fidelity/long.csv).

## Mutation and maintenance interventions

The 5,000-tick results below are totals across seeds 1–3, with one seeded organism per
world. “Matches” means the booted genome exactly matches the creator at the
first append. Deaths in this table are deaths **before boot**, not all program
deaths. Every row satisfies starts = boots + deaths + pending.

| Organism | Mutation k (base/bg) | Maintenance | Starts | Boots | Deaths before boot | Pending | Matches |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |
| Squarestep | 63/63 | k=14 | 12,331 | 12,330 | 0 | 1 | 8,055 |
| Squarestep | 63/63 | off | 12,285 | 12,284 | 0 | 1 | 8,058 |
| Squarestep | 16/8 | k=14 | 12,364 | 12,364 | 0 | 0 | 7,413 |
| Squarestep | 16/8 | off | 12,282 | 12,282 | 0 | 0 | 7,413 |
| Wildstep | 63/63 | k=14 | 12,337 | 12,335 | 0 | 2 | 7,799 |
| Wildstep | 63/63 | off | 12,285 | 12,285 | 0 | 0 | 7,790 |
| Wildstep | 16/8 | k=14 | 12,332 | 12,330 | 1 | 1 | 7,814 |
| Wildstep | 16/8 | off | 12,284 | 12,282 | 0 | 2 | 7,811 |
| Radchain | 63/63 | k=10 | 14 | 14 | 0 | 0 | 14 |
| Radchain | 63/63 | off | 14 | 14 | 0 | 0 | 14 |
| Radchain | 16/8 | k=10 | 25 | 23 | 0 | 2 | 14 |
| Radchain | 16/8 | off | 17 | 17 | 0 | 0 | 14 |

At 5,000 ticks, every Squarestep and Wildstep run in the negligible-mutation
conditions has at most one remaining exact starting genome, whether maintenance
is enabled or disabled. Their live populations range from 4,094 to 4,096.
All 12 of those runs record zero mutations. Radchain retains its exact starting
genome in all surviving programs under those conditions: populations are 7, 4
and 6, with maintenance enabled and disabled alike.

With mutation exponents 16/8, all three organisms lose every exact starting
genome by tick 5,000 in these seeds. The mobile populations still nearly fill
the grid. Occupancy therefore misses both copying differences and the loss of
the starting sequence.

The [per-seed matrix](reproduction-fidelity/matrix.csv) includes creator-change
counts, live versus inert maintenance deaths, mixed construction and foreign
boot counts, and the actual final sampling interval. Individual birth records
separate the creator-change mechanisms.

Construction starts are first appends into empty cells. Every start ends in a
known boot, death before boot, or a still-pending body at the horizon. The
unrestricted matching count compares the newborn with its creator immediately
before the creation pass. Mutation can change a creator during construction;
maintenance can trim either the creator or an abandoned body. Those exposures
are recorded separately, and multiple causes can apply to one construction.

The restricted clean ratio conditions those exposures away. It answers whether
the copy protocol can still fail without them, as the early traces demonstrate.
It is not an intervention-effect estimate. The tables preserve all construction
outcomes for comparisons across conditions, including pending bodies whose
outcomes are not yet known.

## Driftkin: byte 39 is confirmed; the first body has two bytes

The [earlier seed-1 analysis](2026-08-13_mutation-load-and-emergence-milestones.md)
predicted that the evolved 39-byte Driftkin would append literal `39`, attempt
boot before its copy machinery, and produce a one-byte offspring.

Fresh traces under the frontend regime (`r_energy=0.25`, `r_mass=1`, maintenance
k=7, local exponent 1, root preload 20E/12M) confirm the byte and instruction
ordering but correct the first offspring's size. In seeds 1–3, under both mutation
settings, the root:

1. Attempts boot at tick 0, which fails on an empty target.
2. Moves at ticks 1 and 2.
3. Successfully appends `39` at both ticks 3 and 4.
4. First successfully boots that body at tick 23, with genome `[39,39]`.

In seed 1, maintenance shortens that child at tick 74 and removes it at tick 159.
The two-byte genome was recorded at boot, before any maintenance loss. Across
1,000 ticks, the three default-mutation runs produce 37, 34 and 34 boots; none
matches the creator, and all three runs record zero mutations. The first child's
full recorded lifecycle is in [cases.json](reproduction-fidelity/cases.json).
These are controlled fresh starts with the observed genome, rather than a saved
snapshot of the historical evolved world.

## Reproduce and inspect

From the repository root:

```bash
cargo build --release --manifest-path rust/Cargo.toml --bin proteus-audit

python3 rust/scripts/reproduction_experiment.py \
  --binary rust/target/release/proteus-audit \
  --output rust/runs/reproduction-repeat/matrix \
  --ticks 5000 --cadence 250 --seeds 1 2 3 --jobs 2

python3 rust/scripts/reproduction_experiment.py \
  --binary rust/target/release/proteus-audit \
  --output rust/runs/reproduction-repeat/long \
  --conditions reference no-maintenance \
  --ticks 20000 --cadence 1000 --seeds 1 2 3 --jobs 2

python3 rust/scripts/reproduction_experiment.py \
  --binary rust/target/release/proteus-audit \
  --output rust/runs/reproduction-repeat/traces \
  --organisms squarestep wildstep --conditions no-maintenance \
  --ticks 150 --cadence 25 --trace-end 150 --seeds 1 2 3 --jobs 2

python3 rust/scripts/reproduction_experiment.py \
  --binary rust/target/release/proteus-audit \
  --output rust/runs/reproduction-repeat/driftkin \
  --organisms driftkin --conditions reference mutation-default \
  --ticks 1000 --cadence 100 --trace-end 1000 --seeds 1 2 3 --jobs 2
```

The compact tables and selected action/birth records are retained in
[the evidence directory](reproduction-fidelity/README.md). Original raw runs,
pinned executables and source copies were discarded at chapter closure.
The [provenance record](reproduction-fidelity/provenance.json) preserves their
identities, not downloadable archives. The commands above regenerate the
experiment using the checkout's engine; exact historical reproduction requires
matching the recorded source digest.

After regenerating the trace batch above, inspect a specific body:

```bash
python3 rust/scripts/analyze_genomes.py \
  rust/runs/reproduction-repeat/traces/squarestep-no-maintenance-seed-1 \
  --uid 582023
```

## Disposition: closed

The copying-protocol follow-up was completed in the [Guardstep study](2026-10-29_reliable-copying.md). The chapter's synthesis and its implications for emergence are in [Research findings](README.md).
