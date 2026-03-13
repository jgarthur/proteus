## Viability And Decay Notes

### Scope

These notes summarize the current theoretical picture behind Proteus v0.1 after the recent dense-world runs and the switch from inert auto-boot to inert grace-period maintenance.

The goal is not to replace simulation. It is to identify which pressures are actually strong under the current defaults, and which intuitions are misleading.

### Current Default Regime

- `R_energy = 0.25`
- `R_mass = 0.05`
- `P_spawn = 0`
- `D_energy = 0.01`
- `D_mass = 0.01`
- `T_cap = 4`
- `M = 1/128`
- `N_synth = 1`
- `inert_grace_ticks = 10`

### Main Conclusions

1. Energy is not the main limiting resource for small absorbers.
2. Mass is still the main bottleneck for replication.
3. The new inert-grace rule makes active construction cheap, but abandoned fragments can still persist for hundreds of ticks if they can scavenge any ambient energy.
4. Random movement is probably not the first-order fix. It may help spatial escape later, but it does not change the present energy-vs-mass asymmetry enough by itself.

### Background Energy Intake

If an absorber drains the same 5-cell cross every tick, expected energy intake is approximately:

`E_absorb_tick ~= 5 * R_energy * (1 - D_energy)`

Under the current defaults:

`E_absorb_tick ~= 5 * 0.25 * 0.99 = 1.2375 energy / tick`

This is extremely large relative to maintenance for small programs.

### Maintenance Break-Even For Stationary Absorbers

For a program of size `S` that repeatedly executes `absorb`, expected maintenance is:

`E_maint_tick = S * M`

With `M = 1/128`, the absorb-only break-even size is roughly:

`S_break_even ~= E_absorb_tick / M ~= 158`

That is far above the size of the dominant scavenger/plants we actually observe. This explains why absorb-heavy organisms are so hard to dislodge in a stationary environment: their local energy budget is overwhelmingly positive.

### Seed Replicator Energy Budget

For the 11-instruction seed with a 25-tick replication cycle:

`refill(25) = R_energy * (1 - (1 - D_energy)^25) / D_energy ~= 5.55`

Solo cross-neighborhood absorb yield:

`E_in_cycle ~= 5 * refill(25) ~= 27.77`

Per-cycle costs:

- maintenance: `25 * 11 * (1/128) ~= 2.11`
- base instruction costs: `1 * takeM + 11 * appendAdj = 12`

So:

`E_surplus_cycle ~= 27.77 - 2.11 - 12 ~= 13.62`

This means the seed is energetically comfortable. It is not primarily energy-limited.

### Seed Replicator Mass Budget

Per 25-tick cycle, the seed roughly gets:

- one `takeM` from an empty neighbor after refill: `~0.56 mass`
- ambient mass in its own cell: `25 * 0.05 = 1.25 mass`

Without synthesis:

`M_in_cycle ~= 1.81 mass`

but replication needs:

`M_out = 11 mass`

So the seed is strongly mass-limited.

With `N_synth = 1`, the seed can convert its energy surplus into:

`M_synth ~= E_surplus_cycle / (N_synth + 1) ~= 6.81 mass`

That gives total mass income of about:

`M_total_cycle ~= 0.56 + 1.25 + 6.81 ~= 8.62 mass`

Still short of 11 in one cycle, but plausibly enough over about 2 cycles. This matches the general simulation picture: replication should be viable, but not free.

### Decay And Storage

Storage threshold is:

`threshold = T_cap * S`

So tiny absorbers can bank nontrivial local energy before decay:

- size 1: threshold 4
- size 3: threshold 12

This is enough to ride out short local deficits, especially because maintenance is so low.

Any future environmental-variation experiment must therefore make cold phases strong or long enough that small absorbers cannot simply bank through them.

### Abandoned Inert Fragment Lifetime

Under the new rule, an inert fragment pays no maintenance while it is being actively written. Once abandoned, it pays normal maintenance.

Ignoring ambient resources entirely, a fragment of size `S` with no energy or free mass has a rough lower-bound expected lifetime:

`T_die_lower ~= sum(1 / (i * M) for i = 1..S)`

At `M = 1/128`, this gives:

- size 1: `~128 ticks`
- size 4: `~267 ticks`
- size 8: `~348 ticks`
- size 16: `~433 ticks`

These are lower bounds because any scavenged energy or mass extends lifetime further.

So the new grace-period rule removes accidental auto-booted scavengers, but it does not imply fast cleanup. Debris can still be ecologically relevant.

### Why Random Movement Is Probably Secondary

A seed altered to move randomly might avoid local crowding and empty-cell failure in some cases, but it has real costs:

- `move` costs a full 1-tick action and 1 energy
- movement can relocate the parent away from nearby mass it was about to exploit
- movement does not fix the basic fact that stationary absorbers already have a strongly positive energy budget

So random movement may help later as a seed design experiment, but it is unlikely to solve the current main problem by itself.

### Current Theoretical Read

The current defaults create a world where:

- small absorb-heavy organisms are energetically safe
- replication is feasible but still mass-constrained
- selection pressure for reproduction is weak unless local conditions become bad enough that stationary plants actually die

That is why dense worlds become busy and diverse without making replication stable. The economy supports life, but not yet a strong enough reason to spread.
