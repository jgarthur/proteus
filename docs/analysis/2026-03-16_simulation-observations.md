# Simulation Observations — March 16, 2026

**Context**: Non-authoritative experiment notes from Proteus v0.2.0 commit `1fdc90c050e098aa0815fc46ab2391a15be57e8a` plus frontend changes.

## Observations

- program size grows pretty immediately upwards. duh, programs are writing into their offspring
- most programs end up with crazy huge stacks. probably inevitable even given random drift? something to consider.
  - stack is also completely outside the physics...
- config 1 below was a nice change. added 2 move instructions and increased maintenance a bit. move to have things spread out a bit and not smother offspring
- key insight! most important code at beginning given prefix bias on failed replication!
  - old 99, 99, 81, 81, 81, 83, 64, 66, 48, 85, 95, 49, 100
  - new 64, 66, 48, 85, 95, 49, 100, 99, 99, 81, 81, 81, 83
- after that change (config 2), genuinely some interesting behavior. decent variability across random seeds where population can either hit a useful mutation and expand or not.
  - some gliders as usual, move/absorb/collect but no reproduction
- seed 8793547488703238 quite interesting
  - multiple populations take off around tick 1600-1800 and fill the map.
- seed 8694427656868726 has some fun diagonal gliders.
  - mean program size 36 and steadily climbing after 8k ticks.
  - mutations rise quite a bit from 6k-8k. along with drop in energy. programs hitting bg radiation usage and higher mutation rates!
  - after 10k ticks, server hovering around 1.4 GB memory usage. good to profile later. probably stacks growing huge.
- seed 6241520189105184 more diagonals
  -  BUT rerun with maintenance at .02 (config 1 setting) and nothing much. some immortals and gliders)
- maintenance at .01 (config 3) with seed above gives expansion but takes a couple 1000 ticks to happen.
- seed 3072905103314130 very slow steady expansion still climbing at 12k ticks. cool double line diagonal glider around 13k ticks. cool little guy making a square at 19k ticks... still going at 24k dropping nonviable offspring. population leveled out around 2.5k after maybe 20k ticks. stopped at 36k. programs are huge and kinda garbage, lots of nop.


## Config 1

```json
{
  "width": 256,
  "height": 256,
  "seed": 7609376551830126,
  "r_energy": 0.25,
  "r_mass": 1,
  "d_energy": 0.01,
  "d_mass": 0.01,
  "t_cap": 4,
  "maintenance_rate": 0.02,
  "maintenance_exponent": 1,
  "local_action_exponent": 1,
  "n_synth": 1,
  "inert_grace_ticks": 10,
  "p_spawn": 0,
  "mutation_base_log2": 16,
  "mutation_background_log2": 8,
  "seed_programs": [
    {
      "x": 32,
      "y": 32,
      "code": [
        99,
        99,
        81,
        81,
        81,
        81,
        83,
        64,
        66,
        48,
        85,
        95,
        49,
        100
      ],
      "free_energy": 20,
      "free_mass": 12
    },
    {
      "x": 1,
      "y": 1,
      "code": [
        99,
        99,
        81,
        81,
        81,
        81,
        83,
        64,
        66,
        48,
        85,
        95,
        49,
        100
      ],
      "free_energy": 20,
      "free_mass": 12
    }
  ]
}
```

## Config 2

```json
{
  "width": 256,
  "height": 256,
  "seed": 1,
  "r_energy": 0.25,
  "r_mass": 1,
  "d_energy": 0.01,
  "d_mass": 0.01,
  "t_cap": 4,
  "maintenance_rate": 0.0078125,
  "maintenance_exponent": 1,
  "local_action_exponent": 1,
  "n_synth": 1,
  "inert_grace_ticks": 10,
  "p_spawn": 0,
  "mutation_base_log2": 16,
  "mutation_background_log2": 8,
  "seed_programs": [
    {
      "x": 32,
      "y": 32,
      "code": [
        64,
        66,
        48,
        85,
        95,
        49,
        100,
        99,
        99,
        81,
        81,
        81,
        83
      ],
      "free_energy": 20,
      "free_mass": 12
    },
    {
      "x": 128,
      "y": 128,
      "code": [
        64,
        66,
        48,
        85,
        95,
        49,
        100,
        99,
        99,
        81,
        81,
        81,
        83
      ],
      "free_energy": 20,
      "free_mass": 12
    }
  ]
}
```

## Config 3

```json
{
  "width": 256,
  "height": 256,
  "seed": 6241520189105184,
  "r_energy": 0.25,
  "r_mass": 1,
  "d_energy": 0.01,
  "d_mass": 0.01,
  "t_cap": 4,
  "maintenance_rate": 0.01,
  "maintenance_exponent": 1,
  "local_action_exponent": 1,
  "n_synth": 1,
  "inert_grace_ticks": 10,
  "p_spawn": 0,
  "mutation_base_log2": 16,
  "mutation_background_log2": 8,
  "seed_programs": [
    {
      "x": 32,
      "y": 32,
      "code": [
        64,
        66,
        48,
        85,
        95,
        49,
        100,
        99,
        99,
        81,
        81,
        81,
        83
      ],
      "free_energy": 20,
      "free_mass": 12
    },
    {
      "x": 128,
      "y": 128,
      "code": [
        64,
        66,
        48,
        85,
        95,
        49,
        100,
        99,
        99,
        81,
        81,
        81,
        83
      ],
      "free_energy": 20,
      "free_mass": 12
    }
  ]
}
```
