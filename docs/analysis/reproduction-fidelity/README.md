# Reproduction-fidelity evidence

These files contain the compact evidence for the accompanying
[reproduction-fidelity analysis](../2026-10-28_reproduction-fidelity.md).

- `matrix.csv` and `matrix.json` preserve all 36 outcomes of the 5,000-tick
  mutation/maintenance comparison, including construction denominators.
- `long.csv` and `long.json` preserve the 20,000-tick reference and
  no-maintenance outcomes.
- `traces.csv` and `traces.json` summarize the six detailed early-construction runs.
- `driftkin.csv` and `driftkin.json` summarize the six Driftkin trace runs.
- `cases.json` retains the actual action/event/birth rows for incomplete copying,
  shared construction, live-program extension, and Driftkin offspring. Each case
  identifies its run, canonical input digest, executable digest and source digest.
- `early-copy-failures.json` identifies the first unmatched, single-creator boot
  in each mobile-organism trace, plus the number observed through tick 150.
- `provenance.json` identifies the original experiment definitions and tools; `archive_retained` records availability.

Only compact evidence is retained. Complete raw runs, executable copies and
source archives are unavailable. The report's commands regenerate the scenarios
with the checkout's engine; historical hashes identify the original build rather
than guaranteeing that a different checkout reproduces it exactly.

A boot is an inert-to-live transition. Matching counts compare byte sequences
with the creator at the stated observation boundary. See
[the tool guide](../../GENOME-OBSERVATION.md) for the exact definitions and the
difference between the restricted clean group and all construction outcomes.
