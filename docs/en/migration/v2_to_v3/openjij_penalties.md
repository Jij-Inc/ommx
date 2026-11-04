# Move OpenJij penalty options into the preparation policy

```{index} sample_qubo_sa (v2), uniform_penalty_weight (v2), penalty_weights (v2), inequality_integer_slack_max_range (v2)
```

If your v2 call passed `uniform_penalty_weight` or relied on the implicit weight of 1, choose a weight for the original objective scale. Here selecting both options gains two points over the best allowed choice. A weight of four offsets that gain. Keep the source model so that another weight can be tried on a fresh copy. Install the OpenJij adapter to run this example.

```python
from copy import copy

from ommx import FixedPenaltyPreparation, Instance, Sense
from ommx_openjij_adapter import OMMXOpenJijSAAdapter

source = Instance.maximize()
x, y = source.new_binary("x"), source.new_binary("y")
source.objective = 3 * x + 2 * y
source.add_constraint(x + y == 1)
prepared = copy(source)
policy = OMMXOpenJijSAAdapter.recommended_preparation_policy()
policy.fixed_penalty = (
    FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(weight=4.0)
)
prepared.prepare(OMMXOpenJijSAAdapter.INPUT_CLASS, policy)
samples = OMMXOpenJijSAAdapter.sample_without_preparation(prepared, num_reads=2, seed=42)
assert samples.sense == Sense.Maximize
```

The sampling result reports the original objective. If your application also reads dual values, note that HiGHS and Python-MIP omit them when the prepared input has an explicit output objective.

## Update downstream consumers

The v2 helper `sample_qubo_sa` returned raw samples. The v3 adapter returns an evaluated {class}`~ommx.SampleSet`. For code that already has a native OpenJij response and only needs to decode it, replace `response_to_samples` with `decode_to_samples`. If you change a penalty, start again from the original model: preparing an already applicable input does not replace the penalty stored in its objective.
