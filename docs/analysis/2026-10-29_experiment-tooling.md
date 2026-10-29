# Tools for studying and designing copying organisms

> **Scope:** Parked proposals from the closed copying chapter. These priorities concern that investigation, not the selected next research agenda. See [Research findings](README.md) for the current synthesis.

This is a proposal for developers doing organism experiments. It reflects the
[reproduction-fidelity study](2026-10-28_reproduction-fidelity.md), where the
hardest work was relating individual actions to population outcomes and keeping
experimental evidence consistent while the observer was being developed.
The tools below are proposed, not implemented by this note.

## First priority: a bytecode workbench

An organism designer needs to see why the VM took a branch or advanced a read
head. Add a headless, deterministic single-program debugger with an assembler
that supports named labels and constants, plus disassembly annotated with byte
offsets. Assembly must produce ordinary genome bytes; names must not become
runtime metadata or change the VM.

The trace should expose the executed instruction, instruction and read heads,
stack, loop counter/return position, flag, local budget, energy/mass, and queued
nonlocal action. Separate local execution from resolution of the nonlocal action.
An append can be queued successfully and later fail because the target is
protected, resources are inadequate, or another actor resolved first.

The most useful first demonstration would step through one failed append and
its retry. It should reveal whether the copy head advances twice, whether the
success flag survives until the branch, and whether a failed branch quietly
consumes the candidate byte. A compact saved trace is more useful here than
another aggregate metric.

## Second priority: controlled interference fixtures

Provide a small scenario format for a candidate copier plus a fixed interfering
program, initial registers/resources, and observation horizon. Use ordinary
engine instructions for interference so tests exercise supported execution.
Cases should cover resource starvation and recovery, a protected target, a
foreign append between verification and boot, a foreign boot, and a target that
moves away. Include uncontested reproduction as a positive control.

Report starts, exact and inexact births, unfinished bodies, parent survival,
time/resources per exact birth, and subsequent reproduction by exact offspring.
Keep incomplete constructions in the denominator: never booting is not reliable
reproduction. Separate a protocol guarantee under stated assumptions from an
observed success rate under finite stochastic interference.

A fixture should also demonstrate the baseline copier failing for the reason
the candidate is meant to address. This prevents a friendly environment from
making a retry loop appear robust without ever exercising its recovery path.

## Third priority: a construction timeline

Build a query over the audit artifacts that starts from a body uid and shows
all contributing actors, append/write/delete/boot attempts, target openness,
resource state and genome changes on one timeline. Include creator changes and
post-boot edits. A creation-parent tree alone hides shared construction.

The current artifacts answer many of these questions, but finding a minimal
case requires joining several JSONL files manually. Index rows by uid and tick,
retain explicit missing-history markers, and allow export of a small evidence
bundle containing the manifest, relevant rows and source/executable digests.
Bounded traces must never be presented as complete lifetimes.

## Fourth priority: an experiment ledger and resumable batches

The experiment launcher already pins binaries, checks archived source digests,
preserves manifests and controls thread count. Extend that foundation with a
machine-readable run state: planned, running, completed or failed, with a clear
completion marker and validated output inventory. Resume only when the entire
input and executable identity match. Preserve failed attempts separately.

Have one command validate a batch's source/tool hashes, intended matrix,
termination ticks, event/census identities, construction denominators and
genome hashes. Emit comparison tables from those validated rows. This would
replace repeated ad hoc checks and prevent an obsolete exploratory run from
entering a final table because its directory name looks similar.

For long runs, estimate remaining time from completed comparable runs and expose
a graceful deadline that stops scheduling and preserves partial results. Keep
partial horizons visible rather than mixing them into final-horizon summaries.

## Later: checkpoints, replay and offspring assays

A versioned complete-state checkpoint would allow replay around a rare failure
and transplanting actual offspring into a standardized environment. It must
preserve every execution-relevant field, world tick and RNG derivation inputs;
a sampled genome and lineage row cannot reconstruct a world. This belongs with
the existing SNAPSHOT-BOUNDARY work and needs separate design and parity proof.

Offspring assays would distinguish exact sequence inheritance from reproductive
competence. Run a recorded newborn genome in a declared common environment,
retain the source birth and any reset state, and follow multiple generations.
Compare isolated competence with competition in the original ecology; neither
measurement substitutes for the other.

## Scope for the immediate organism investigation

Use the current audit tool, targeted engine tests and explicit manifests first.
Write down the candidate's invariants before selecting a genome: what advances
its copy position, how it validates a target, and which interleavings it can
tolerate. The workbench and interference fixtures offer the clearest next tooling
investment if repeated manual traces become the limiting step. A general visual
dashboard or an automated genome search is less useful until those contracts
and failure measurements are clear.
