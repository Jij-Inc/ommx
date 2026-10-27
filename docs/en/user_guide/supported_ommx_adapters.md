# Choose an Adapter

An Adapter connects an OMMX Instance to an execution backend's inputs and outputs.
If you already know the solver or sampler you want, go directly to its workflow.
You do not need to execute every Tutorial in order.

| Goal | First procedure | Returned result |
|---|---|---|
| Solve a first small problem | [PySCIPOpt Tutorial](../tutorial/solve_with_ommx_adapter) | Solution |
| Handle Indicator and SOS1 directly | [Solve special constraints with PySCIPOpt](../tutorial/solve_special_constraints_with_pyscipopt_adapter.md) | Solution |
| Lower special constraints and solve with HiGHS | [Preparation for an Adapter](../tutorial/prepare_instance_for_adapter.md) | Solution |
| Obtain several candidates with OpenJij | [Sample a small problem](../how_to/sample_with_openjij_adapter) | SampleSet |

Follow [Installation](../getting_started/install.md) to add only the selected Adapter.
A `Solution` evaluates one candidate; a `SampleSet` handles several candidates and information such as occurrence counts.
Matching return types do not imply identical search methods or optimality guarantees.

## Four checks when choosing

1. **Direct input**: an Adapter's `INPUT_CLASS` specifies the Instances its implementation accepts directly.
   Check variable kinds, expression representations, constraint families, and sense rather than only broad names such as “linear” or “QUBO.”
2. **Required Preparation**: if direct input is unsuitable, start from the Adapter's recommended Policy.
   Unsupported constraints or missing finite bounds may prevent the needed conversion.
3. **User-owned settings**: these include penalty weights, tolerances, read counts, and time limits.
   Obtaining a recommendation does not determine every appropriate setting for a particular problem.
4. **Information in the result**: besides reported objective, feasibility, and candidate counts, inspect backend termination reasons,
   bounds, gaps, and optimality information. Available diagnostics differ by Adapter.

`INPUT_CLASS` membership checks representation conditions, not mathematical equivalence with the source or backend optimality guarantees.
Judge these separately. [InstanceClass](instance_class.md), [PreparationPolicy](preparation_policy.md),
and [Transformation guarantees](transformation_guarantees.md) explain the distinctions.

## Solver names and problem classes are not one-to-one

For example, OpenJij SA's direct input allows Binary polynomial minimization without active constraints, not only quadratic expressions.
Use its actual `INPUT_CLASS` and recommendation instead of assuming that OpenJij always requires prior QUBO conversion.
See the [OpenJij How-to](../how_to/sample_with_openjij_adapter) for an example requiring an explicit weight.

Conversely, Adapters accepting the same Instance can differ in settings, search methods, and termination conditions.
[Compare Adapters](../how_to/compare_adapters.md) compares reported objectives and feasibility for the same problem;
[Diagnostics](../how_to/inspect_adapter_diagnostics.md) inspects backend observations.
If you have an initial candidate, see [Initial states](../how_to/use_initial_state.md).
To inspect where execution time is spent, see [Tracing](../how_to/trace_execution).
None requires learning Experiment.

## Packages and contracts

The following Adapters are developed in this repository. The Book's executable examples use public `3.0.0b7`.
Implementation links point to the same tag. Check each API for exact arguments and supported inputs.

| Package | Backend | b7 implementation |
|---|---|---|
| `ommx-pyscipopt-adapter` | SCIP / PySCIPOpt | [Implementation](https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7/python/ommx-pyscipopt-adapter) |
| `ommx-highs-adapter` | HiGHS | [Implementation](https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7/python/ommx-highs-adapter) |
| `ommx-openjij-adapter` | OpenJij SA / SQA | [Implementation](https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7/python/ommx-openjij-adapter) |
| `ommx-python-mip-adapter` | Python-MIP | [Implementation](https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7/python/ommx-python-mip-adapter) |

This Book's API Reference is generated from its development revision.
For features differing from the public package, follow the [Reference version guide](../reference/index.md)
and check the public tag's contracts too.

Adapters in separate repositories include [Gurobi](https://github.com/Jij-Inc/ommx-gurobipy-adapter),
[D-Wave](https://github.com/Jij-Inc/ommx-dwave-adapter),
[Fixstars Amplify](https://github.com/Jij-Inc/ommx-fixstars-amplify-adapter),
[Fujitsu DA4](https://github.com/Jij-Inc/ommx-da4-adapter),
[Kipu Iskay](https://github.com/Jij-Inc/ommx-kipu-iskay-adapter),
and [Q-CTRL QAOA](https://github.com/Jij-Inc/ommx-qctrl-qaoa-adapter).
Check each project's SDK compatibility, runtime, authentication, licensing, and result contracts.
Inclusion here does not mean it has been executed in the Book's b7 environment.

For implementing an Adapter, see the [Developer Guide](../developer_guide/adapter.md).
