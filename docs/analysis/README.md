# docs/analysis/

Non-authoritative working notes that support the spec.

- Dated files are point-in-time plans or synthesis notes.
- Named topic files are longer-lived critiques or theory notes.

Use this folder for context only. `SPEC.md` and `SPEC-COMPANION.md` remain the source of truth.

## Dated Notes

- `2026-03-16_simulation-observations.md` records exploratory v0.2.0 behavior, notable seeds, resource settings, and program-growth pathologies.
- `2026-03-24_spatiotemporal-ambient-noise-proposal.md` proposes spatially and temporally varying Poisson resource-arrival fields driven by layered simplex noise.
- `2026-08-11_rayon-performance-profile.md` records phase-level Rayon profiling and the A/B experiments behind the RAYON-PERFORMANCE backlog item.
- `2026-08-12_rayon-optimization-plan.md` is the executed step-by-step implementation plan for RAYON-PERFORMANCE.
- `2026-08-12_rayon-optimization-results.md` records the implementation, fixture and instruction-coverage audit, exact growing-web-scenario benchmarks, deep parity results, and line-profile findings.
- `2026-08-13_dyadic-sampler-plan.md` is the implementable hand-off plan for DYADIC-SAMPLERS: exact `2^-k` Bernoulli/binomial bit samplers, Poisson inversion, config validation, call-site map, and migration steps.
- `2026-08-21_dyadic-sampler-results.md` records the completed DYADIC-SAMPLERS implementation, draw-stream migration, verification digests, and benchmark deferral.
- `2026-08-13_performance-roadmap.md` records the tiered performance strategy after RAYON-PERFORMANCE: sampler work, SoA hot-field split, inline program storage behind data-justified caps, Pass 1 dispatch, and expected ceilings.
- `2026-08-13_mutation-load-and-emergence-milestones.md` preserves the August 2026 exploratory observations, defines the M0–M6 emergence milestone ladder (reserving *abiogenesis* for self-replication), measures the realized background-stressed mutation rate against the configured baseline, explains the boom-bust plateau as space saturation unwinding toward an attractor, derives why `move` and multi-cell `absorb` are required for viability, and shows that spontaneous accretion past size 1 is governed by mutations × occupancy rather than by `mutation_base_log2` alone.
