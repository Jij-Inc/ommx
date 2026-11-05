# OMMX adapter for OpenJij

Sample candidate assignments with [OpenJij](https://www.openjij.org/) and evaluate them against the OMMX problem. You choose the penalty weights when constraints are converted to penalties.

[Installation from a prerelease or `main`](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/getting_started/install.html#install-openjij-adapter) · [日本語の導入手順](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/getting_started/install.html#install-openjij-adapter)

In v3, `SampleSet.objectives` reports the original objective. Penalties affect the sampling energy, so code that used the v2 objective values as penalized energies must account for that change.

## Sample a constrained problem

```python markdown-code-runner
from ommx import DecisionVariable, FixedPenaltyPreparation, Instance, Sense
from ommx_openjij_adapter import OMMXOpenJijSAAdapter

x = DecisionVariable.binary(0, name="x")
instance = Instance.from_components(
    decision_variables=[x],
    objective=x,
    constraints={0: x == 1},
    sense=Sense.Minimize,
)

policy = OMMXOpenJijSAAdapter.recommended_preparation_policy()
policy.fixed_penalty = FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(
    weight=2.0
)
instance.prepare(OMMXOpenJijSAAdapter.INPUT_CLASS, policy)

sample_set = OMMXOpenJijSAAdapter.sample_without_preparation(
    instance,
    num_reads=16,
)
print(sample_set.summary)
```

## Reference

[OpenJij adapter API](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/autoapi/ommx_openjij_adapter/index.html#ommx_openjij_adapter.OMMXOpenJijSAAdapter) · [日本語サイトのAPI Reference](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/autoapi/ommx_openjij_adapter/index.html#ommx_openjij_adapter.OMMXOpenJijSAAdapter)

The adapter reference above describes supported inputs and explicit preparation, including which variable IDs to supply when passing an initial state. The [Book chapter on choosing a formulation](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/book/transform_and_solve.html) explains when to keep or convert a constraint.
