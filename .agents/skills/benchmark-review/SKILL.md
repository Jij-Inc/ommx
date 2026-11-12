---
name: benchmark-review
description: Use when reviewing, designing, triaging, or managing OMMX benchmarks, CodSpeed runs or flamegraphs, performance PRs or issues, benchmark workload changes, or benchmark CI policy. Prioritize PR-level regression detection on fixed inputs, define the benchmark contract and cost model, reserve scaling and profile analysis for manual follow-up, and keep CI cost proportional to the regression signal.
---

# Benchmark Review

The primary purpose of persistent benchmarks is PR-level regression detection
through CodSpeed comparisons of the same deterministic input. Choose workloads
where an unwanted scan, clone, allocation, or rebuild produces a detectable
cost increase. Such work can regress performance without changing the scaling
exponent.

Connect each family to a concrete regression risk, cost model, and lifecycle.
Cross-size exponents and flame graphs are manual follow-up tools for inspecting
the workload or diagnosing a regression. They are not required for every PR,
and automatic exponent checks are not the default goal of this workflow.

## Review Flow

### 1. Classify the primary question

Default persistent benchmarks to PR-level regression detection. Distinguish
that purpose from a task specifically requesting further analysis:

- **PR regression guardrail**: compare the same deterministic workload between
  the PR head and a comparable baseline. Detect added unnecessary work, whether
  it changes a constant factor or introduces worse complexity. Each size in an
  existing scaling family also serves this purpose as a fixed input.
- **Manual scaling analysis**: examine how cost changes as an independent
  variable grows when investigating the workload or a detected regression.
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

#### PR regression guardrail

- Keep the input deterministic and representative of the regression risk.
- Compare the same benchmark URI, input shape, runner mode, and environment
  between the PR head and baseline. Keep historical inputs stable; give a
  materially different workload a distinct URI.
- Make unwanted work visible: for example, keep active work fixed while using
  enough unrelated retained rows to expose an accidental full-table clone.
- Use the CodSpeed PR comparison as the primary signal. An unchanged exponent
  does not dismiss a regression, and exponent analysis is not a prerequisite
  for reporting one.
- A representative fixed size can be sufficient. Retain multiple sizes when
  they add detection coverage worth their CI cost; do not multiply automatic
  cases solely to make an exponent calculable.

#### Manual scaling analysis

- Perform this later when needed to inspect a workload or explain a regression,
  or when the user explicitly requests scaling evidence. It is not a routine
  PR acceptance requirement.
- For that analysis, use at least three geometrically spaced input sizes,
  such as `N`, `2N`, `4N` or `N`, `10N`, `100N`.
- Keep other shape dimensions fixed unless they are independent variables in
  the stated cost model.
- Estimate the observed exponent between adjacent points:

  ```text
  p = log(T2 / T1) / log(N2 / N1)
  ```

  Expect `p ~= 0` for independence from the varied data, `p ~= 1` for linear
  scaling, and `p ~= 2` for quadratic scaling.
- Prefer the larger points when fixed overhead distorts small inputs. Use
  same-run ratios where possible so hardware differences cancel.
- Inspect or generate a cross-size table or graph when making a scaling claim.
  CodSpeed compares each parameter separately; manual cross-size inspection is
  sufficient. Do not introduce an automatic exponent gate by default.
- Do not require a flame graph unless the task is also diagnosing why the
  observed scaling is wrong.

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

- **Persistent guardrail**: retain when its fixed inputs provide an actionable
  PR regression signal, with `main` runs supplying comparison baselines.
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
- Inspect which PR events trigger comparison and which head commit was
  measured. If later pushes are not remeasured automatically, state that
  coverage gap; a result for an older head does not cover the latest changes.
  Do not change workflow triggers or dispatch runs unless the task authorizes it.

### 6. Validate claims with CodSpeed evidence

- Use CodSpeed run results to discover benchmark URIs and values.
- Prioritize the PR head-to-baseline comparison and verify environment
  differences before attributing a change to code.
- Compute cross-size ratios or exponents in manual follow-up when a scaling
  claim or diagnosis needs them, not as a requirement for every guardrail PR.
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
mixed cost models, wrong layers, missing PR comparison coverage, profile-only
workloads in CI, or cost disproportionate to the signal. Insufficient scaling
points matter when a scaling claim is being made, not for a fixed-input
regression guardrail by itself.

Do not require every benchmark to prove an optimization. A benchmark can be a
valuable canary when its expected failure mode and response are explicit.

## OMMX Patterns

### Python `small_many` accumulation

Treat the Python `small_many` addition benchmarks as persistent PR regression
guardrails originating from PR #498:

```text
Purpose: PR-level regression detection at fixed operand counts
Regression: Repeated += clones the growing accumulator and changes O(N) to O(N^2)
Origin: PR #498
Measured boundary: Python += -> PyO3 __iadd__ -> Rust in-place merge
Independent variable: Number N of fixed-size operands
Expected evidence: CodSpeed PR comparison for each retained fixed input
Manual follow-up: Cross-size exponent near 1 when inspecting scaling
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

Treat the inactive-constraint `Instance::partial_evaluate` benchmark from issue
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

Use a shape containing only inactive regular constraints to keep active semantic
work small.
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

- Does the persistent benchmark provide a useful PR-level regression signal?
- Is any additional task manual scaling analysis, profiling, or workflow cost?
- What concrete regression and origin does the contract record?
- What operation and language or system boundary does it measure?
- What is the cost model, and which dimensions are independent variables?
- Does the evidence match the purpose?
- Are PR head and baseline measurements comparable, and is the measured head
  the one under review?
- If making a scaling claim, are there enough points and was the exponent
  examined manually? Do not require this for routine PR regression detection.
- For profiling, are percentages used diagnostically rather than as thresholds?
- Is the benchmark in the narrowest valid layer?
- Is it a persistent guardrail or a temporary/manual diagnostic?
- Are build-included wall time and runner cost proportionate to the signal?
- Are performance claims backed by comparable measurements?
