# Implement an OMMX Adapter

This is an entry point for authors connecting a new backend to OMMX. First understand the model shared with Adapter users:

1. [InstanceClass and Adapter input conditions](../user_guide/instance_class.md).
2. [Instance Preparation and PreparationPolicy](../user_guide/preparation_policy.md).
3. [Transformed problems and removed constraints](../user_guide/removed_constraints.md).

## APIs to consult

- Solver Adapter: {class}`~ommx.adapter.SolverAdapter`.
- Sampler Adapter: {class}`~ommx.adapter.SamplerAdapter`.
- Variables to pass to the backend: {attr}`~ommx.Instance.used_decision_variables`.
- Evaluate backend output: {meth}`~ommx.Instance.evaluate` / {meth}`~ommx.Instance.evaluate_samples`.

The variables retained by an Instance are not identical to backend variables: some are kept for pre-transformation representation or evaluation. See [Variable roles and value recovery](../user_guide/variable_roles.md) if this distinction is new.

## Contracts to implement

- Declare `INPUT_CLASS`: the exact set of Instances accepted without Preparation and the complete applicability condition.
- Make `check_applicability()` and `require_applicable()` report/enforce only that membership. Do not treat converter or backend errors as additional applicability conditions or as {class}`~ommx.adapter.AdapterNotApplicableError`.
- Implement {meth}`~ommx.adapter.SolverAdapter.solve_without_preparation` or {meth}`~ommx.adapter.SamplerAdapter.sample_without_preparation`. These strict paths do not prepare inputs and reject non-members.
- The usual `solve()` and `sample()` apply {meth}`~ommx.adapter.SolverAdapter.recommended_preparation_policy` to an input copy before strict execution. Preserve that copy → Preparation → strict-method sequence when overriding them for backend-specific arguments.
- The Adapter constructor also accepts only exact inputs and performs no Preparation.
- Encode only {attr}`~ommx.Instance.used_decision_variables` as OMMX-derived independent backend variables. Backend-specific auxiliary variables are allowed. Preserve the OMMX-ID/backend-variable mapping and decode every used ID. Fixed, dependent, irrelevant, removed-constraint-only, named-function-only, and `output_objective`-only variables are not independent backend inputs.
- Evaluate decoded states or samples against the same Instance used to build the backend input. Require it to remain unchanged from encoding through backend execution and decoding. It owns dependent-value recovery and removed-constraint evaluation. Consider an internal snapshot only if callers must modify the original during this period.

When attaching backend-reported optimality to a Solution, pass it through the evaluation Instance's {meth}`~ommx.Instance.map_active_optimality`. See [Transformed problems and removed constraints](../user_guide/removed_constraints.md) for the distinction between active-formulation optimality and optimality for the reported objective.

## Reference implementations

The following references are pinned to reviewed revision `dc533e65be3520137f202538023ab18cbbe631ce`. When targeting a published SDK, check that release's source and API contracts.

For solvers, read the [PySCIPOpt Adapter](https://github.com/Jij-Inc/ommx/blob/dc533e65be3520137f202538023ab18cbbe631ce/python/ommx-pyscipopt-adapter/ommx_pyscipopt_adapter/adapter.py): its input class, checks, used-variable encoding/decoding, and evaluation against the same Instance. Its constructor retains the supplied Instance by reference, so callers must leave it unchanged until decoding finishes.

For samplers, also read the [OpenJij Adapter](https://github.com/Jij-Inc/ommx/blob/dc533e65be3520137f202538023ab18cbbe631ce/python/ommx-openjij-adapter/ommx_openjij_adapter/adapter.py) and its [decode implementation](https://github.com/Jij-Inc/ommx/blob/dc533e65be3520137f202538023ab18cbbe631ce/python/ommx-openjij-adapter/ommx_openjij_adapter/_decode.py).
