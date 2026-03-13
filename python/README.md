# Proteus Python Backend

Python backend for the Proteus simulator, including the reference engine and optional Cython helper acceleration.

## Acceleration

The simulator always has a pure-Python reference path in [`src/proteus/engine_reference.py`](src/proteus/engine_reference.py).
If the optional compiled helper module is available, the hot hash/loop-scan helpers are imported from `proteus._engine_cython_ops`.
Otherwise the engine silently falls back to the Python helpers while keeping the same semantics.

The active backend is visible at runtime in the benchmark output and in the frontend metrics panel.

## Build The Optional Cython Helper

1. `cd python`
2. `uv sync --extra cython`
3. `.venv/bin/python setup.py build_ext --inplace`

After the build, the runtime should report `engine_backend = cython-ops`.

## Benchmark And Evaluation

Throughput benchmark:

`python/.venv/bin/python python/benchmarks/engine_benchmark.py --case default --repeat 3`

Milestone evaluation for the default run:

`python/.venv/bin/python python/benchmarks/engine_evaluation.py --case default --ticks 0 100 1000 10000 20000`

Add `--json` to the evaluation command to dump the full structured report.

Preset parameter sweep for anti-scavenger regimes:

`python/.venv/bin/python python/benchmarks/parameter_sweep.py --case default --details`

Live time-series monitoring for a long run:

`python/.venv/bin/python python/benchmarks/live_monitor.py --case default --ticks 50000 --sample-every 1000`

You can override system parameters inline during monitoring, for example:

`python/.venv/bin/python python/benchmarks/live_monitor.py --case default --ticks 50000 --sample-every 1000 --set inert_grace_ticks=10`
