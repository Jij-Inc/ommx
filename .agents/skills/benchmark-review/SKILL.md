---
name: benchmark-review
description: Use when reviewing, designing, or triaging OMMX benchmarks, CodSpeed results, flamegraphs, performance PRs/issues, or benchmark workload changes to verify that each benchmark has a clear measurement intent, cost model, expected scaling/flamegraph, and actionable regression signal.
---

# Benchmark Review

Use this skill to review benchmarks as measurement instruments, not as
standalone slow workloads. A good benchmark should connect a specific
regression risk to an explicit cost model and produce evidence that confirms
or falsifies that model.

## Review Flow

1. Name the measured operation and boundary.
   - State the public or internal entrypoint being measured.
   - Identify whether the benchmark targets Rust SDK internals, Python API
     boundary behavior, PyO3 dispatch, solver adapter behavior, artifact I/O,
     or CI/workflow overhead.
   - For Python benchmarks, justify why the cost must be measured through the
     Python API. If almost all expected work is Rust-internal algorithmic
     scaling, prefer a Rust benchmark plus a smaller Python smoke benchmark.

2. State the cost model before judging the numbers.
   - Write the expected decomposition, for example:
     `T ~= fixed_overhead + N * per_row_work + clone_cost + drop_cost`.
   - Name the independent variables: variables, constraints, terms, samples,
     special-constraint families, active vs removed rows, dense vs sparse
     structure, or Python operation count.
   - Separate semantic work from incidental overhead: validation, atomic
     staging, clone, allocation, hashing, drop, tracing, PyO3 dispatch, and
     data conversion.
   - State the expected scaling and which term should dominate at the chosen
     input sizes.

3. Check whether the input shape isolates the intended term.
   - Prefer input shapes that make one suspected cost visible without mixing
     unrelated costs.
   - Include enough sizes to distinguish fixed overhead from scaling behavior.
   - Use no-op, removed-only, or degenerate shapes when the goal is to detect
     overhead that should not scale with semantic work.
   - Avoid retaining very large CI benchmarks whose only signal is that a
     Rust-internal operation is expensive; move that signal to the narrower
     benchmark layer.

4. Predict the flamegraph.
   - List the functions or subsystems that should dominate if the benchmark is
     measuring the intended quantity.
   - Treat unexpected dominance as evidence that the benchmark measures a
     different thing. Common red flags are whole-object clone/drop, memcpy,
     allocator churn, tracing/OpenTelemetry setup, Python import/setup, or
     serialization when those are not the stated target.
   - If the expected root function or call path is absent, do not accept the
     benchmark label at face value.

5. Validate performance claims with measurements.
   - Never claim a performance improvement or regression without benchmark
     data and methodology.
   - Use CodSpeed results and flamegraphs when available. If the relevant run
     or flamegraph is unavailable, state the evidence gap instead of inferring
     from code shape alone.
   - Compare like with like: same benchmark, same input shape, same branch/run
     relationship, and enough context to avoid confusing new benchmarks with
     changed results.

6. Write findings as benchmark-design issues.
   - Lead with the measurement failure: missing hypothesis, mixed cost model,
     wrong layer, fixed-overhead domination, absent flamegraph evidence, or CI
     cost disproportionate to the regression signal.
   - Propose the narrower benchmark or workload split that would detect the
     intended regression.
   - Do not require every benchmark to prove an optimization. A benchmark can
     be valuable as a canary if its cost model and expected failure mode are
     explicit.

## OMMX Patterns

### Instance partial evaluation

For `Instance::partial_evaluate`, distinguish semantic partial evaluation
from transaction overhead:

```text
T ~= fixed_value_state
   + atomic_staging_or_clone
   + fixed_value_registration
   + active_constraint_partial_evaluation
   + special_constraint_delta_preparation
   + commit_swap_and_drop
```

A removed-regular-constraints-only shape is valuable because semantic active
constraint work should be small. If the flamegraph is dominated by
`Instance::clone`, fixed-value registration, or dropping the old instance, the
benchmark detects transaction overhead rather than constraint rewriting.

### Python API benchmarks

For Python benchmarks, separate API-boundary regressions from Rust algorithm
scaling:

- Keep Python benchmarks when the risk is Python-visible behavior: PyO3 method
  dispatch, overloaded arithmetic, Python-driven expression construction,
  conversion between Python containers and Rust types, or the public API
  end-to-end path.
- Prefer Rust benchmarks for heavy internal algorithms such as large
  `to_qubo`, substitution, encoding, sampling evaluation, or expression
  rewriting unless the Python boundary is part of the suspected regression.
- If a tiny benchmark is dominated by tracing or setup, describe it as an API
  smoke/overhead benchmark rather than a scaling benchmark.

### Expression construction

For Python-driven expression construction, the useful cost model often is:

```text
T ~= Python operation count * PyO3 dispatch
   + number_of_additions * Rust add_or_merge
   + number_of_clones * expression_size
```

Expected flamegraph entries include `_ommx_rust` PyO3 methods,
`polynomial_base::add`, hash-map merge/clone work, and memcpy when clone cost
is dominant. This benchmark is useful if the suspected regression is caused by
Python operators triggering expensive Rust-side expression cloning.

## Review Checklist

- What regression should this benchmark detect?
- What operation and API/language boundary does it measure?
- What is the explicit cost model, and which input dimension exercises each
  term?
- Which term should dominate at each benchmark size?
- Does the chosen input shape isolate the intended cost?
- What flamegraph path is expected, and was it observed?
- Is the benchmark in the right layer: Rust, Python, adapter, or workflow?
- Is any retained CI workload proportionate to the signal it provides?
- Are performance claims backed by comparable benchmark data?
