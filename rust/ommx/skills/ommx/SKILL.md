---
name: ommx
description: Build, solve, transform, and exchange optimization instances with the OMMX Python SDK, solver adapters, Artifacts, and Experiments. Use when working with OMMX objects or .ommx files.
---

# OMMX

Use the OMMX installed in the user's project. This skill is bundled with that
SDK; after changing SDK versions, obtain the skill path again and update any
project link or copy. The examples below use the v3 Python API.

## Build and solve an instance

```python
from ommx import DecisionVariable, Instance, Sense
from ommx_highs_adapter import OMMXHighsAdapter

x = DecisionVariable.binary(0, name="x")
y = DecisionVariable.binary(1, name="y")
instance = Instance.from_components(
    decision_variables=[x, y],
    objective=3 * x + 2 * y,
    constraints={0: x + y <= 1},
    sense=Sense.Maximize,
)
solution = OMMXHighsAdapter.solve(instance)
assert solution.feasible
assert solution.objective == 3
```

Install the chosen adapter in the project environment, for example
`uv add ommx-highs-adapter`. Follow the project's environment and dependency
conventions when running code.

- Import domain classes from top-level `ommx`. `ommx._ommx_rust` is internal;
  `ommx.v1` describes the protobuf wire format, not the Python SDK API.
- Register every referenced variable in the instance and use unique variable
  IDs. Constraint IDs are the keys of the `constraints` mapping. Keep names
  and subscripts as labels; do not substitute them for IDs.
- Comparisons construct constraints normalized to `function <= 0` or
  `function == 0`. State the optimization sense explicitly. Do not change a
  formulation's variable domains, bounds, or objective sense to fit a solver.
- Use `Instance.load_mps(...)` or `Instance.load_qplib(...)` for existing
  benchmark files. Use `ParametricInstance.with_parameters(...)` when
  instantiating a parameterized model.

## Adapter input and transformations

- An adapter's `INPUT_CLASS` describes exact accepted instances.
  `check_applicability()` inspects membership without preparing the instance.
- Prefer the adapter's `solve()` or `sample()` convenience operation for the
  normal workflow. These prepare a private copy when needed; direct adapter
  construction expects an already applicable instance.
- `Instance.prepare()` and `lower_special_constraints()` mutate the instance.
  Explicit preparation can leave partial changes on failure. Preserve the
  source instance when needed and recheck applicability after transformations.
- `Function ** n` represents composed powers. Polynomial-only QUBO/HUBO
  operations can reject them. For a polynomial square, use `g * g` when an
  expanded polynomial is required; do not assume arbitrary composed functions
  can be expanded into polynomials.
- Check feasibility and solver termination information before interpreting
  variable values as an optimum. Preserve the full `SampleSet` from sampling
  when comparing multiple candidates.

## Store and share results

- Use `ommx.artifact` for immutable stored Artifacts and `.ommx` archives.
  Build a new artifact with `ArtifactDraft` and commit it to obtain a
  `LocalArtifact`. The Local Registry owns stored refs and blobs; archives and
  remote registries are import/export boundaries.
- Use `ommx.experiment.Experiment` to record repeated trials. A Run is one
  comparison unit and can contain several Solves or Samplings. Attach source
  data and parameters needed to understand and reproduce the results.
- Use `ommx inspect`, `list`, `import`, and `export` for CLI exchange. `rm`
  removes a ref; `gc --delete` reclaims eligible unreferenced blobs. Only
  remove stored data when the user's task calls for it.

## Find API details

Consult the installed API's signatures and docstrings for version-specific
details. Use the relevant guide instead of guessing obsolete constructors:

- [Build and solve](https://jij-inc.github.io/ommx/en/tutorial/solve_with_ommx_adapter.html)
- [Adapter input and lowering](https://jij-inc.github.io/ommx/en/user_guide/capability_model.html)
- [Artifacts](https://jij-inc.github.io/ommx/en/tutorial/share_in_ommx_artifact.html)
- [Experiments](https://jij-inc.github.io/ommx/en/tutorial/experiment_management.html)
- [Python API reference](https://jij-inc.github.io/ommx/en/api/index.html)
- [Rust SDK](https://docs.rs/ommx/)
