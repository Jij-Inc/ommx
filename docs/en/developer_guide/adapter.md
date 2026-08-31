# Implement an OMMX Adapter

This page is the entry point for Adapter authors connecting a custom Backend to OMMX. Start with the following models, which are shared with Adapter users.

1. [Adapter Exact Inputs (`INPUT_CLASS`)](../user_guide/adapter_input_class.md)
2. [Instance Preparation and `PreparationPolicy`](../user_guide/preparation_policy.md)
3. [Removed Constraints and Feasibility](../user_guide/removed_constraints.md)

## APIs Used by an Implementation

- Solver Adapter: {class}`~ommx.adapter.SolverAdapter` and its strict {meth}`~ommx.adapter.SolverAdapter.solve_without_preparation` method
- Sampler Adapter: {class}`~ommx.adapter.SamplerAdapter` and its strict {meth}`~ommx.adapter.SamplerAdapter.sample_without_preparation` method
- Decision variables to send to the Backend: {attr}`~ommx.Instance.used_decision_variables`
- APIs for evaluating Backend output: {meth}`~ommx.Instance.evaluate` / {meth}`~ommx.Instance.evaluate_samples`

Every concrete Adapter must declare a non-optional `INPUT_CLASS`. Membership in that class is the complete applicability condition: inherit `check_applicability()` and `require_applicable()` instead of adding a second Adapter-specific precondition layer. Converter-local representation checks and Backend limits may still fail while constructing or running the Backend input, but those failures are not applicability failures.

Implement `solve_without_preparation()` or `sample_without_preparation()` as the exact-input operation. It must reject a non-member, must not prepare or mutate its input, and must evaluate the Backend result with that same exact `Instance`. The easy `solve()` and `sample()` APIs make an isolated copy, apply the fresh Policy returned by `recommended_preparation_policy()`, and then invoke the strict method. If a concrete Adapter adds options to an easy method, forward them with an explicit typed signature.

Only encode {attr}`~ommx.Instance.used_decision_variables`. Preserve the correspondence between those OMMX IDs and Backend variables, decode every used variable ID, and pass the decoded state or samples to the exact `Instance` for evaluation. Fixed, dependent, irrelevant, output-objective-only, removed-constraint-only, and named-function-only variables are not independent Backend inputs; the `Instance` owns their reconstruction and the evaluation of removed constraints.

## Reference Implementations

For a Solver Adapter, use the [PySCIPOpt Adapter implementation](https://github.com/Jij-Inc/ommx/blob/main/python/ommx-pyscipopt-adapter/ommx_pyscipopt_adapter/adapter.py) as the reference. It shows the required `INPUT_CLASS`, a fresh recommended Policy, the strict solve path, encoding and decoding of used decision variables, and evaluation with the same exact `Instance` in one implementation.

For a Sampler Adapter, also see the [OpenJij Adapter implementation](https://github.com/Jij-Inc/ommx/blob/main/python/ommx-openjij-adapter/ommx_openjij_adapter/adapter.py) and its [sample decoding implementation](https://github.com/Jij-Inc/ommx/blob/main/python/ommx-openjij-adapter/ommx_openjij_adapter/_decode.py).
