# Genome and lineage observation

## Run an audit

Build the diagnostic binary and pass it an ordinary headless-runner manifest:

```bash
cd rust
cargo build --release --bin proteus-audit
./target/release/proteus-audit \
  --manifest example-run.json \
  --output target/genome-audit \
  --trace-start 0 --trace-end 100
```

The output directory must be fresh. The manifest supplies the world, bootstrap,
tick limit and census cadence. `--output` selects the audit directory independently
of the manifest's ordinary runner output directory. `--trace-start` and
`--trace-end` select a half-open interval of executed ticks for detailed events;
both default to zero. All ticks contribute to totals and the birth log.

An event labeled tick 0 happened during the transition from world tick 0 to world
tick 1. Sampled lineage and metrics rows label the resulting world tick. Samples
include tick zero, every requested cadence, and the final tick even when it falls
between cadence boundaries.

| Artifact | Contents |
| --- | --- |
| `manifest.json` | Audit schema version, exact runner input, input digest, build identity, and trace interval |
| `metrics.jsonl` | Engine census, cumulative causal counters, and counts of each live genome |
| `lineage.jsonl` | One row per occupied cell at each sample: uid, parent, generation, birth tick, live state, location, size, genome digest |
| `genomes.jsonl` | SHA-256 genome digests and their exact byte arrays, emitted once per distinct sampled genome |
| `births.jsonl` | Every successful inert-to-live transition, including its exact genome, creator uid, successful boot actors, and construction comparisons |
| `actions.jsonl` | Queued edit, boot and move attempts, their captured values and success flags, within the trace interval |
| `events.jsonl` | Successful neighbor edits and moves, boot events, net local edits, maintenance losses, mutations and spawns within the trace interval |
| `summary.json` | Final engine metrics and cumulative causal counters; written after the other artifacts have been flushed |

`proteus-audit --build-info` prints executable and source digests without creating
artifacts. A failed or interrupted audit has no completed summary. Audit errors are reported
on stderr with a nonzero exit status. A fresh directory prevents mixing data from
multiple executions. Genome digests identify byte sequences, while program uids
identify individual bodies. Uids are scoped to the simulation; they cannot be
compared across runs.

## Interpret reproduction

A recorded parent is the program that created the body's first byte. It need
not be the only contributor or the program that boots it. A construction
contributor is an actor that successfully appends, writes or deletes a byte from
that body before its first boot. An identical-byte write still counts as a
contribution. `boot_actors` lists all successful booters; multi-boot produces one
birth record. The birth's `actor` is the first successful booter in queue order;
use `boot_actors` when the full set matters.

`matches_creator_at_creation` compares the newborn genome with the creator's
code immediately before the Pass 2 that created its first byte.
`matches_creator_at_boot` compares it with the same creator immediately before
the boot's Pass 2, and is absent if the creator no longer exists. These are exact
sequence comparisons, not proofs of autonomous or faithful self-replication.

`creator_changed_during_construction` records whether the creator changed or
disappeared at an observed pass boundary from first append
through the local phase of the boot tick. Creator edits during the creation pass
can affect later copies and are included; edits concurrent with boot are excluded.
Changes after the last append but before boot still exclude a body from the clean
comparison.

`creator_changes` separates local edits, neighbor edits, maintenance, baseline
mutation, background mutation, death, and edits outside the observed driver.
A body is inert before its first boot and cannot mutate; mutation can affect
pre-boot copying through its creator.

`construction_maintenance_deleted_bytes` counts bytes that maintenance removed
from the inert body. `contributors` counts its distinct editing actors. The
`clean_single_contributor_boots` denominator includes known constructions with
one contributor, a surviving creator with no observed code change, no body
maintenance loss, and no foreign boot actors. `boots_by_noncreator` counts known
constructions with any foreign booter; `boots_without_creator` counts those where
the creator did not also boot. `clean_matching_boots` counts exact matches within
that group.

All construction outcomes are counted: `constructions_started` equals
`boots_with_known_construction + constructions_died_unbooted + pending_constructions`.
The clean matching ratio describes a restricted copying mechanism; it conditions
away recorded creator changes, body maintenance and foreign contributions/boots.
For intervention comparisons, report the unrestricted matching count alongside
all starts, deaths and pending bodies. Matches among booted bodies alone omit
failed constructions; matches per start at a finite horizon include pending
bodies whose outcomes are not yet known.

Cancelling self-edits within one local pass are not visible to this boundary
observer.

`appends_to_live` counts successful extension of already-live programs.
`appends_by_nonparent` counts appends whose actor differs from the target's recorded
parent, including live targets and roots. It is distinct from the number of
bodies with multiple pre-boot contributors. Maintenance events include `was_live`
to distinguish live programs from inert bodies. Cumulative maintenance byte and
death counters also split into `live` and `inert` categories.

The complete birth log preserves booted bodies that die between samples.
Sampled lineage alone can omit short-lived bodies and ancestors; a missing
parent row is not evidence that a program was spontaneously created. Spawn
counts appear in metrics and spawn events in the selected trace window. Starting
the audit at bootstrap ensures that all ordinary constructed bodies have their
construction history available.

Inspect a run summary or a particular program with:

```bash
python3 rust/scripts/analyze_genomes.py rust/target/genome-audit
python3 rust/scripts/analyze_genomes.py rust/target/genome-audit --uid 8321
```

The program view follows creation ancestry, lists its sampled states and genome
sequences, and selects retained events where it is the target, actor or a booter.
Action attempts include the source's working resources and target openness before
Pass 2. These are inputs to resolution; same-pass transfers can change the pools
before an additional cost is paid.
An ancestor without an observed identity is marked explicitly. This is a creation
tree; contributor relationships are carried separately in the event trace.

## Run paired experiments

The experiment script defines the verified Squarestep, Wildstep and Radchain
regimes from the seed-organism analysis. Each organism is tested in its own
regime; cross-organism differences also include those environmental differences.

```bash
python3 rust/scripts/reproduction_experiment.py \
  --binary rust/target/release/proteus-audit \
  --output rust/target/reproduction-experiment \
  --seeds 1 2 3 --ticks 2000 --cadence 100 --jobs 2
```

The four conditions form a factorial comparison of the verified maintenance
rate versus no maintenance, and negligible mutation (`63/63`) versus the spec
mutation exponents (`16/8`). The latter change only the mutation exponents;
other organism-specific parameters stay fixed. The negligible-mutation condition
still has nonzero probabilities, so consult the observed mutation counters.
Runs with the same organism and seed share the engine's deterministic noise
field. Their trajectories can diverge after an intervention changes state.

Each execution preserves its exact manifest and audit artifacts. The experiment
pins a copy of the binary for the whole matrix, preserves the launcher and
analysis scripts with SHA-256 digests, archives the Rust source after checking it
against the binary's source digest, fixes each child at one engine thread,
and writes `results.csv` and `results.json`
with per-seed outcomes and denominators. Failed runs abort the comparison rather
than silently disappearing. Use `--organisms` and `--conditions` to select a
subset. Use a fresh output directory for each matrix. To rerun an archived launcher,
pass `--source previous-experiment/rust-source` and use its pinned binary; the
source hash check rejects a stale or unrelated build.

For the historical Driftkin
prediction, select `--organisms driftkin --conditions mutation-default` and use
`--trace-start 0 --trace-end 1000 --ticks 1000`. Driftkin uses the frontend regime
and is a trace specimen, not a verified reproducer.

## Developer contract

`GenomeObserver` receives immutable grid views from the same driver used by
`run_tick_report`. Its callbacks run serially after preparation, local execution,
nonlocal execution, the lifecycle/maintenance/spawn tail, and mutation. The
ordinary `()` implementation disables callbacks at compile time. No observer
receives an RNG or mutable simulation state.

These boundaries follow SPEC's Tick Structure, Maintenance and Mutation sections,
and SPEC-COMPANION §§7–9. Only local writes/deletions can change code in Pass 1.
Pass 2 captures one nonlocal action per actor, and its final Flag identifies
success before packet listening can overwrite it. Edits occur before deferred
moves, so the observer follows target uids to their final locations. Maintenance
can only shorten or remove a genome in the tail; mutation preserves its length
and flips one bit afterward.

`GenomeAudit` requires unique, nonzero uids, as assigned by engine bootstrap.
Callers that manually replace programs through `grid_mut()` must preserve that
invariant. It clones code at the boundaries and retains construction state until
boot or death. Drain `events`, `attempts` and `births` after each tick. This is an opt-in
diagnostic with memory and runtime costs proportional to the observed genomes;
it is not part of ordinary web observation.

The audit artifact contract is versioned independently as `1.0.0`. Its input is
the runner's `0.2.0` manifest; it does not change canonical runner inputs or engine
semantics. The CLI independently compares cumulative boot, spawn, death and
mutation-cause counts against `TickReport` on every tick, failing on disagreement. It also checks the construction outcome balance and the
genome byte ledger: initial bytes + appends + spawns − local deletions − neighbor
deletions − maintenance losses must equal current code bytes.

Run the observation checks with `cargo test --test genome_observation --test audit_cli`
from `rust/`, and `python3 -m unittest discover -s scripts -p 'test_genome_tools.py'`.
`scripts/check-genome-audit-parity.sh` compares every deterministic data artifact
between serial execution and Rayon at one and four threads on a growing,
mutating fixture. Build metadata is excluded from that byte comparison because
it identifies the differing binaries and thread counts.
