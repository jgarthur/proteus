# Spatiotemporal Ambient Noise Proposal

**Status**: Non-authoritative design proposal recorded March 24, 2026. This document does not change the simulator specification.

## Overview

The scalar arrival-rate parameters `R_energy` and `R_mass` are promoted to spatiotemporal fields. Each cell's per-tick Poisson arrival rate varies smoothly in space and time, driven by layered 3D simplex noise. This produces a dynamic resource landscape: energy and mass arrive in drifting, multi-scale patches rather than uniformly.

The two fields use **independent** noise instances (different seeds, independent parameter sets). All parameters below are prefixed `energy_` or `mass_` accordingly (e.g., `energy_sigma`, `mass_L_space`). Where the description is identical for both, the prefix is omitted for brevity.

## Noise Model

For each cell at position `(x, y)` on tick `t`, define the raw noise value:

```
η(x, y, t) = (1 / Z) × Σ_{i=0}^{N-1} p^i × simplex(x / (L_space × 2^i),
                                                        y / (L_space × 2^i),
                                                        t / (L_time  × 2^i))
```

where:

- `simplex(·)` is 3D simplex noise with output approximately in `[-1, 1]`
- `N` is the number of octaves
- `p ∈ (0, 1]` is the spatial persistence (shared by x and y axes)
- `L_space` is the base spatial wavelength in cells
- `L_time` is the base temporal wavelength in ticks
- Lacunarity is fixed at 2 (each octave doubles the frequency)
- `Z = sqrt(Σ_{i=0}^{N-1} p^{2i})` is the variance-normalizing factor, so `η` has approximately unit variance regardless of `N` and `p`

The per-cell arrival rate is:

```
λ(x, y, t) = exp(μ + σ × η(x, y, t))
```

where `μ` is chosen so that the **expected arrival rate equals the configured `R`**:

```
μ = log(R) − σ² / 2
```

This follows from `E[exp(σ η)] = exp(σ² / 2)` when `η` is approximately standard normal (valid by central-limit effects across octaves). The parameter `R` retains its original interpretation as the mean arrival rate per cell per tick; `σ` controls how much spatial and temporal variation exists around that mean.

## Poisson Sampling

Each tick, each cell draws:

```
arrivals(x, y, t) ~ Poisson(λ(x, y, t))
```

These arrivals enter the background radiation pool (for energy) or background mass pool (for mass) exactly as in the uniform-rate model. All downstream mechanics — decay, collection, absorption, storage thresholds — are unchanged.

## Stationary Distribution

Under constant `λ` and decay rate `D > 0`, a single cell's background pool has stationary distribution `Poisson(λ / D)`. With spatiotemporal noise, the **conditional** stationary distribution given a locally stable `λ` is still `Poisson(λ / D)`, but the **marginal** (averaged over the noise field) is a Poisson–log-normal mixture. The marginal mean background level remains `R / D`.

## Initialization

Fresh simulations should initialize background pools from the stationary marginal. For each cell:

1. Sample `η(x, y, 0)` from the noise field at `t = 0`.
2. Compute `λ = exp(μ + σ × η)`.
3. If `D > 0`: sample `bg ~ Poisson(λ / D)`.
4. If `D = 0`: set `bg = 0` (no finite steady state).

This avoids the transient burn-in that would occur if all cells started at the global mean.

## Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `R_energy` | float > 0 | Mean energy arrival rate per cell per tick (unchanged) |
| `R_mass` | float > 0 | Mean mass arrival rate per cell per tick (unchanged) |
| `energy_sigma` | float ≥ 0 | Log-space std dev for energy noise. 0 recovers uniform arrivals. |
| `mass_sigma` | float ≥ 0 | Log-space std dev for mass noise. 0 recovers uniform arrivals. |
| `energy_N_octaves` | int ≥ 1 | Number of octaves for energy noise |
| `mass_N_octaves` | int ≥ 1 | Number of octaves for mass noise |
| `energy_persistence` | float ∈ (0, 1] | Persistence for energy noise (controls relative weight of fine vs coarse scales) |
| `mass_persistence` | float ∈ (0, 1] | Persistence for mass noise |
| `energy_L_space` | float > 0 | Base spatial wavelength for energy noise (cells) |
| `mass_L_space` | float > 0 | Base spatial wavelength for mass noise (cells) |
| `energy_L_time` | float > 0 | Base temporal wavelength for energy noise (ticks) |
| `mass_L_time` | float > 0 | Base temporal wavelength for mass noise (ticks) |
| `energy_seed` | int | RNG seed for energy simplex noise |
| `mass_seed` | int | RNG seed for mass simplex noise |

## σ = 0 Recovery

When `σ = 0`, the noise term vanishes: `λ(x, y, t) = exp(log(R)) = R` everywhere, recovering the uniform Poisson arrival model exactly. No special-casing is needed.

## Design Notes

- **Persistence controls landscape character.** Low persistence (e.g., 0.3) produces broad biomes with subtle texture. High persistence (e.g., 0.9) or `p = 1` gives every spatial scale equal influence, producing a more chaotic, fractal landscape.
- **Temporal scale sets foraging pressure.** When `L_time` is comparable to agent traversal time across `L_space`, resource patches shift fast enough that agents cannot settle permanently but slow enough that movement is rewarded. This is the regime that produces interesting foraging behavior.
- **σ controls ecological harshness.** At `σ = 1`, the ratio between a +2σ cell and a −2σ cell is roughly `e^4 ≈ 55×`. At `σ = 0.5`, it is roughly `e^2 ≈ 7×`. Larger σ creates starker deserts and oases.
- **Independent fields allow decoupled resource geography.** Energy and mass patches need not overlap. This can drive spatial specialization if organisms evolve to exploit one resource and trade or migrate for the other.
