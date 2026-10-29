# Research findings

Start here; use [the catalog](ARCHIVE.md) for detailed studies, historical plans
and implementation records. [SPEC.md](../SPEC.md) and
[SPEC-COMPANION.md](../SPEC-COMPANION.md) define the rules.

## Research aim

Open-ended evolution and emergence: reproducing organizations arising and
sustaining evolutionary innovation. Occupancy, sequence diversity and clever
hand-designed seeds are intermediate observations.

The working concern is **evolutionary accessibility**: can viable intermediate
forms lead to reproduction and further adaptation? Guardstep raises this concern
but does not prove the rules prohibit open-endedness. Its evolutionary origin
was not tested.

## Findings to carry forward

### Emergence: animation is not self-reproduction

Earlier sweeps observed spontaneous accretion and animation of multi-instruction
bodies, but did not demonstrate self-reproduction arising from them. Keep
persistence, construction, reproduction and sustained evolution separate.
Their numerical thresholds belong to older rules, not automatically to v0.4.0.
[Evidence and milestones](2026-08-13_mutation-load-and-emergence-milestones.md).

### Copying and Guardstep: chapter closed

Under v0.4.0, Squarestep and Wildstep lose their sequences even with zero
observed mutations and maintenance disabled. Failed appends, unconditional boots,
shared construction and live editing explain the loss.
[Mechanism study](2026-10-28_reproduction-fidelity.md).

The 71-byte Guardstep retries, verifies and provisions its child. Ten
single-founder 20,000-tick runs across five seeds produced 40,734 exact births,
with zero observed mutations and every surviving genome exact. Reproduction was
slower. It produced no exact descendants in five mixed worlds with Squarestep;
only one of five Wildstep worlds produced one. Neighbors can boot unfinished
children before verification.
[Comparison and counterexamples](2026-10-29_reliable-copying.md).

**Meaning:** colonization is weak evidence for heritable adaptive organization.
An engineered solution does not establish an evolutionary path. This chapter is
complete; boot-tolerant seed design is parked, not the default next task.

### Environment: separate causes and compare seeds

The ambient study found takeoff/stall outcomes across seeds. Background energy
also affects stressed mutation exposure. Its evidence supported retaining the
tested defaults, not maximizing occupancy as a proxy for emergence.
[Configuration-specific results](2026-08-21_ambient-rebalance-sweep.md).

### Performance: measured negatives supersede estimates

Inline code storage regressed the tested `tick_bench` fixtures; a stride diagnostic
did not justify a layout rewrite. Read these before repeating the roadmap's
optimistic proposals.
[Inline storage](2026-08-22_inline-program-storage-results.md),
[stride diagnostic](2026-08-22_tier-2a-stride-diagnostic.md).

## Keeping this useful

Use one report per question, led by **question, answer, significance, limits,
and disposition**. Promote durable conclusions here; catalog everything else.
A proposal or a closed report's final suggestion is not an active assignment.
[Authoring rules](../AGENTS.md#closing-experiments).

Keep compact evidence and rerun instructions. Full traces, genome dumps and
copied executables are scratch data; extract evidence for unresolved findings
before cleanup. The copying chapter retains about 1 MiB of evidence; bulk archives
are unavailable. Provenance hashes identify original files, not retained copies.
