# Reliable-copying organisms and evidence

Run Guardstep with the same environment as the paired comparison, from the
repository root:

```bash
cargo build --release --manifest-path rust/Cargo.toml --bin proteus-audit
rust/target/release/proteus-audit \
  --manifest docs/analysis/reliable-copying/guardstep.json \
  --output rust/runs/guardstep
```

`guardstep.json` starts a single 71-byte organism in a 64×64 world and runs
20,000 ticks. It uses maintenance k=14 and mutation k=63/63; the latter makes
mutation negligible, not impossible. Audit totals record actual mutations.

`genomes.json` contains the exact experimental byte sequences: Guardstep,
its zero-gift control, the unfunded prototype, Retrystep, Squarestep and Wildstep. The mechanistic Rust
tests load this file directly. The [analysis](../2026-10-29_reliable-copying.md)
explains the protocol, costs, and interference it can and cannot detect.

- `comparison.csv` and `comparison.json` contain the 60 single-founder outcomes.
- `mixed.csv` and `mixed.json` contain the 15 two-founder outcomes, with explicit
  creation-ancestry labels.
- `mixed-cases.json` preserves selected birth records from mixed populations.
- `trajectories.csv` contains sampled live and exact-genome counts;
  `trajectories.png` and `trajectories.svg` plot the paired comparisons.
- `manifests/` contains all 75 primary inputs for reproduction.
- `provenance.json` identifies the original source, executable and experiment inputs;
  `archive_retained: false` records that bulk archives are unavailable.


The tables, figures, selected cases and input manifests are the retained evidence.
Full run streams and copied binaries/sources are not retained. Rerunning the
commands uses the checkout's engine; historical identity is recorded in provenance.
