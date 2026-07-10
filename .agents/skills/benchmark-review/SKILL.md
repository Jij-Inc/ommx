---
name: benchmark-review
description: Use when reviewing, designing, triaging, or managing OMMX benchmarks, CodSpeed runs or flamegraphs, performance PRs or issues, benchmark workload changes, or benchmark CI policy. Classify the measurement purpose, define the benchmark contract and cost model, validate the required scaling or profile evidence, assign a lifecycle and run policy, and keep CI cost proportional to the regression signal.
---

# Benchmark Review

Treat each benchmark family as a hypothesis-driven measurement instrument.
Connect a concrete regression risk to a cost model, the evidence needed to
confirm or falsify it, and an explicit lifecycle. Do not retain a workload only
because it is slow or because a profiler can produce a flame graph for it.

## Review Flow

### 1. Classify the primary question

Choose one primary purpose before judging the implementation or numbers:

- **Scaling guardrail**: determine how cost changes as an independent variable
  grows and detect a complexity regression such as `O(N)` to `O(N^2)`.
- **Fixed-input regression**: compare the same deterministic workload across
  commits to detect a constant-factor regression.
- **Profiling diagnostic**: attribute a fixed input's cost to functions or
  subsystems while investigating an optimization.
- **Workflow cost**: determine whether setup, build, instrumentation, and
  benchmark execution are affordable under the CI run policy.

Keep the measurement layer as a separate axis. Name whether the boundary is a
Rust SDK operation, Python API and PyO3 boundary, adapter, end-to-end user flow,
artifact I/O, or CI workflow. Do not use `end-to-end` as a substitute for an
explicit primary purpose.

### 2. Write the benchmark contract

Write one contract for each benchmark family, not one for every parameter:

```text
Purpose:
Regression:
Origin:
Measured boundary:
Independent variables:
Cost model:
Expected evidence:
Input rationale:
Lifecycle:
Run policy:
Runtime budget:
```

Link `Origin` to the issue, PR, incident, or optimization that established the
regression risk when one exists. State a concrete failure mode rather than a
generic goal such as "measure addition performance."

### 3. Match the evidence to the purpose

#### Scaling guardrail

- Use at least three geometrically spaced input sizes, such as `N`, `2N`,
  `4N` or `N`, `10N`, `100N`.
- Keep other shape dimensions fixed unless they are independent variables in
  the stated cost model.
- Estimate the observed exponent between adjacent points:

  ```text
  p = log(T2 / T1) / log(N2 / N1)
  ```

  Expect `p ~= 1` for linear scaling and `p ~= 2` for quadratic scaling.
- Prefer the larger points when fixed overhead distorts small inputs. Use
  same-run ratios where possible so hardware differences cancel.
- Inspect or generate a cross-size table or graph. CodSpeed tracks each
  parameter as a separate result and does not replace explicit cross-size
  scaling analysis.
- Do not require a flame graph unless the task is also diagnosing why the
  observed scaling is wrong.

#### Fixed-input regression

- Keep the input deterministic and representative of the regression risk.
- Compare the same benchmark URI, input shape, runner mode, and environment.
- Use commit-to-commit timing as the primary evidence. Use a profile only when
  explaining a detected change.

#### Profiling diagnostic

- Fix an input large enough for the suspected term to be visible without
  mixing unrelated work.
- Predict the expected root path and dominant subsystems. Treat unexpected
  clone/drop, allocator churn, tracing, serialization, import/setup, or data
  conversion as evidence that the benchmark measures a different cost.
- Inspect absolute self and total time when available. Use percentages to
  prioritize work, not as a permanent threshold: optimizing one function
  necessarily raises other functions' percentages.
- Keep a diagnostic workload only when it also provides an actionable
  regression signal. Otherwise remove it after the investigation or keep it
  manual.

#### Workflow cost

- Report setup, dependency installation, build, benchmark execution, job wall
  time, and total runner time separately.
- Treat build and cache behavior as CI cost, not measured SDK performance.
- Ensure the measured artifact uses the intended optimized profile. Do not
  interpret an unoptimized development build as a product runtime benchmark.
- Evaluate the build-included job time against the run policy; reducing only
  benchmark duration is insufficient when build time already exceeds budget.

### 4. Choose the narrowest valid layer

- Prefer Rust benchmarks for Rust-internal algorithmic scaling such as large
  `to_qubo`, substitution, encoding, sampling evaluation, or expression
  rewriting.
- Keep Python benchmarks when the regression includes Python-visible behavior:
  overloaded operators, PyO3 dispatch, Python-driven expression construction,
  container conversion, or a public Python end-to-end path.
- Pair a narrow Rust benchmark with a small Python smoke benchmark when Python
  only dispatches a heavy Rust-internal operation.
- Review workflow overhead independently from benchmark runtime.

### 5. Assign lifecycle and run policy

- **Persistent guardrail**: retain on `main` when it detects a concrete
  complexity or fixed-input regression.
- **Diagnostic benchmark**: run manually during performance work; remove it or
  convert it to a persistent guardrail once the hypothesis is resolved.
- Allow CodSpeed to produce profiles on `main` without treating every profile
  as a review artifact. Keep inputs that exist only for profile detail out of
  the persistent suite.
- For OMMX, treat a build-included wall time of roughly 2-3 minutes as the
  target at which a lightweight guardrail suite is cheap enough to run
  automatically on PRs. Until then, keep full PR runs manual and use automatic
  `main` runs as catch-all coverage.
- State both the regression signal and CI cost before retaining a large input.

### 6. Validate claims with CodSpeed evidence

- Use CodSpeed run results to discover benchmark URIs and values.
- Use run comparisons for fixed-input regressions and verify environment
  differences before attributing a change to code.
- Compute cross-size ratios or exponents explicitly for scaling guardrails.
- Query flame graphs for profiling diagnostics or to explain a scaling failure;
  do not query every flame graph merely because it is available.
- Never claim a performance improvement or regression without comparable
  benchmark data and methodology. State the evidence gap when the relevant run
  or profile is unavailable.

### 7. Produce an actionable review

Review benchmark families rather than listing every parameter separately. For
each family, state the purpose, boundary, cost model, evidence, lifecycle, and
recommended action: retain on `main`, move to manual diagnostics, narrow or
relocate the benchmark, or remove it. Lead findings with missing hypotheses,
mixed cost models, wrong layers, insufficient scaling points, profile-only
workloads in CI, or cost disproportionate to the signal.

Do not require every benchmark to prove an optimization. A benchmark can be a
valuable canary when its expected failure mode and response are explicit.

## OMMX Patterns

### Python `small_many` accumulation

Treat the Python `small_many` addition benchmarks as persistent scaling
guardrails originating from PR #498:

```text
Purpose: Scaling guardrail
Regression: Repeated += clones the growing accumulator and changes O(N) to O(N^2)
Origin: PR #498
Measured boundary: Python += -> PyO3 __iadd__ -> Rust in-place merge
Independent variable: Number N of fixed-size operands
Expected evidence: Geometric input sizes and an observed exponent near 1
Lifecycle: Persistent on main
```

For operand size `m`, the intended cost is approximately:

```text
T(N, m) ~= N * Python_and_PyO3_dispatch
          + N * merge(m)
          + amortized_hash_table_growth(N * m)
          = O(N * m)
```

Cloning the accumulator on iteration `k` adds
`sum(k * m) = O(N^2 * m)`. During an optimization, expect the profile to enter
`__iadd__`, `try_add_assign_in_place`, and `add_term`. Hash-table rehash copies
may be legitimate; distinguish them from a whole-accumulator clone on every
addition. Keep `large_little` under a separate contract because a fixed number
of large merges tests per-term merge and rehash cost, not the `small_many`
quadratic regression.

### Instance partial evaluation

Treat the removed-constraint `Instance::partial_evaluate` benchmark from issue
#1027 and PR #1028 as a regression-shaped guardrail. Its cost model separates
semantic evaluation from transaction overhead:

```text
T ~= fixed_value_state
   + atomic_staging_or_clone
   + fixed_value_registration
   + active_constraint_partial_evaluation
   + special_constraint_delta_preparation
   + commit_swap_and_drop
```

Use a removed-regular-constraints-only shape to keep active semantic work small.
When diagnosing it, dominance by `Instance::clone`, fixed-value registration,
or old-instance drop identifies transaction overhead rather than constraint
rewriting.

### Python API boundary

Keep a heavy Python benchmark only when the Python boundary is part of the
regression hypothesis. If nearly all expected work is Rust-internal, move the
scaling guardrail to Rust and retain only a small Python boundary smoke test.
Describe a tiny benchmark dominated by tracing or setup as an overhead smoke
test, not as an algorithmic scaling benchmark.

## Review Checklist

- What is the primary purpose: scaling, fixed-input regression, profiling, or
  workflow cost?
- What concrete regression and origin does the contract record?
- What operation and language or system boundary does it measure?
- What is the cost model, and which dimensions are independent variables?
- Does the evidence match the purpose?
- For scaling, are there enough geometric points and was the exponent examined?
- For profiling, are percentages used diagnostically rather than as thresholds?
- Is the benchmark in the narrowest valid layer?
- Is it a persistent guardrail or a temporary/manual diagnostic?
- Are build-included wall time and runner cost proportionate to the signal?
- Are performance claims backed by comparable measurements?
