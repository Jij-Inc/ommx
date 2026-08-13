# Implement an OMMX Adapter

This page is the entry point for Adapter authors connecting a custom Backend to OMMX. Start with the following models, which are shared with Adapter users.

1. [Adapter Exact Inputs (`INPUT_CLASS`)](../user_guide/adapter_input_class.md)
2. [Instance Preparation and `PreparationPolicy`](../user_guide/preparation_policy.md)
3. [Removed Constraints and Feasibility](../user_guide/removed_constraints.md)

## APIs Used by an Implementation

- Solver Adapter: {class}`~ommx.adapter.SolverAdapter`
- Sampler Adapter: {class}`~ommx.adapter.SamplerAdapter`
- Decision variables to send to the Backend: {attr}`~ommx.Instance.used_decision_variables`
- APIs for evaluating Backend output: {meth}`~ommx.Instance.evaluate` / {meth}`~ommx.Instance.evaluate_samples`

## Reference Implementations

For a Solver Adapter, use the [PySCIPOpt Adapter implementation](https://github.com/Jij-Inc/ommx/blob/main/python/ommx-pyscipopt-adapter/ommx_pyscipopt_adapter/adapter.py) as the reference. It shows `INPUT_CLASS`, applicability checks, encoding and decoding of used decision variables, and evaluation with the same `Instance` in one implementation.

For a Sampler Adapter, also see the [OpenJij Adapter implementation](https://github.com/Jij-Inc/ommx/blob/main/python/ommx-openjij-adapter/ommx_openjij_adapter/adapter.py) and its [sample decoding implementation](https://github.com/Jij-Inc/ommx/blob/main/python/ommx-openjij-adapter/ommx_openjij_adapter/_decode.py).
