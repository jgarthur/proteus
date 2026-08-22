# Config exponent and any-quantum mutation migration

Date: 2026-08-21

## Summary

CONFIG-DYADIC-K and MUTATION-ANY-QUANTUM shipped together in simulator/spec v0.4.0 and API-SPEC v0.3.0. The four configurable dyadic probabilities now cross every boundary as optional integer exponents: `k` means `2^-k`, and `null` means never. Background-stressed mutation now fires once when at least one of the consumed background quanta succeeds independently.

The provisional pre-1.0 API keeps the `/v1` route prefix despite the breaking field rename. A benchmark was not run because the exact samplers are unchanged except for the specified background-mutation call site.

## Digest movements

The example runner manifest canonical projection changed because all four field names and their serialized value shapes changed. Its golden input digest moved from:

- v0.3: `sha256:1ee9860658e963b0fdc49d1660b47c5f37c7d8845aeb5560c5cc593cc6e9953b`
- v0.4 initial migration, runner schema 0.1.0: `sha256:def2e91b9c4d323609cd33e40e2c345ff7698c60106a5f7f8b762be45bb454c5`
- v0.4 reviewed contract, runner schema 0.2.0: `sha256:6eec9c637d8967472f2b9712ee52014968086285e5fc14ad32cba13d38fe9292`

The accepted v0.4 1,000-tick serial parity literals are below. Rayon matched every row at 1, 2, 4, and 8 threads.

| Fixture | Grid digest | Report digest |
| --- | --- | --- |
| `sparse-8x8` | `4a8df5e20c25aeed` | `6b06bd28341924d1` |
| `frontend-64x64` | `324ca3f8458ed1ab` | `5a7f342fd9bb1fe1` |
| `dense-32x32` | `30746a933974dfc3` | `207428148e94afb3` |
| `moving-8x1` | `1c8e49b5edcb8c3d` | `630b1f8aa086feb9` |
| `all-opcodes-71x1` | `2d3accb4e9c770ea` | `ba174754fdf07f5d` |
| `exclusive-5x9` | `4f161e1cbaf13706` | `99d001d0da133275` |

## Mutation-law probabilities

For `mutation_background_log2 = 8`, the old law was `min(x / 256, 1)`. The new law is `1 - (1 - 1/256)^x`, with at most one mutation applied when the event fires.

| Consumed background `x` | Old law | New law | New - old |
| ---: | ---: | ---: | ---: |
| 20 | 7.812500% | 7.529267% | -0.283233 pp |
| 25 | 9.765625% | 9.321281% | -0.444344 pp |
| 32 | 12.500000% | 11.771909% | -0.728091 pp |

The new curve removes the old saturation cliff at `x = 256`; it approaches one asymptotically instead.

## Draw-stream audit

Diff inspection confirmed that decay, maintenance, spawn, and baseline-mutation call sites preserve their previous sampler choice, argument, and following-draw order when configured with equivalent exponents. Therefore a config-only migration with the old background law would be stream-identical. The sole intentional stream change is `mutate_end_of_tick_cell`, where `bernoulli_ratio_pow2` became `binomial_pow2(x, k) > 0`; the subsequent mutation index and bit draws retain their order. The `k = 0` twin-stream test also confirms the endpoint still consumes no sampler draw.
