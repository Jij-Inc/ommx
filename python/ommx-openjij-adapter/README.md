# ommx-openjij-adapter

Provides an adapter between [OMMX](https://github.com/Jij-Inc/ommx) and
[OpenJij](https://www.openjij.org/).

## Usage

`ommx-openjij-adapter` can be installed from PyPI:

```bash
pip install ommx-openjij-adapter
```

OpenJij directly accepts a Binary, unconstrained minimization model through
this adapter. Preparation is an explicit `Instance` operation driven by a
common OMMX policy recommended by the Adapter:

```python markdown-code-runner
from ommx import DecisionVariable, Instance
from ommx_openjij_adapter import OMMXOpenJijSAAdapter

x = DecisionVariable.binary(0, name="x")
instance = Instance.from_components(
    decision_variables=[x],
    objective=x,
    constraints={0: x == 1},
    sense=Instance.MINIMIZE,
)

policy = OMMXOpenJijSAAdapter.recommended_preparation_policy(
    uniform_penalty_weight=2.0,
)
instance.prepare(policy)

sample_set = OMMXOpenJijSAAdapter.sample(
    instance,
    num_reads=16,
)

print(sample_set.summary)
```

The finite penalty weight is part of `PreparationPolicy`, not an OpenJij
backend sampler parameter. It must be selected explicitly when constraints
remain. A finite penalty does not ensure that every returned sample is feasible
for the original constraints; inspect the evaluated `SampleSet`.

## Input class and preparation

`OMMXOpenJijSAAdapter.INPUT_CLASS` describes the instances that the adapter
accepts directly:

- Binary decision variables
- a polynomial objective of any degree (QUBO or Binary HUBO)
- no active regular or special constraints
- minimization

`OMMXOpenJijSAAdapter.check_applicability()` checks that direct-input contract
and the OpenJij-specific preconditions. `sample()`, `solve()`, and the Adapter
constructor never prepare or mutate their input implicitly.

`recommended_preparation_policy()` returns the common OMMX policy with that
`INPUT_CLASS` as its `input_class`. The default permits special-constraint
lowering, bounded Integer log encoding, minimization-sense normalization, and
exact integer slack. Discrete slack approximation remains disabled unless
`allow_approximate_integer_slack=True` is passed. Finite penalties remain
disabled unless `uniform_penalty_weight` or `penalty_weights` is supplied.

Per-constraint penalty weights use source regular-constraint IDs. Those IDs
remain unchanged through integer-slack conversion, and a configured source ID
may remain after its constraint was removed as trivial. Special-constraint
lowering instead adds fresh regular rows outside that source map's domain, so
use a uniform penalty weight when such rows remain active.

`Instance.prepare(policy)` mutates the caller's `Instance` until it belongs to
the Policy's `input_class`, copied from OpenJij `INPUT_CLASS`. It does not run
the Adapter or check backend-specific preconditions. The direct Adapter
boundary remains authoritative:

```python
OMMXOpenJijSAAdapter.require_applicable(instance)
```

The Adapter already evaluates returned samples against that same `Instance`.
It retains the decision-variable dependencies, removed constraints, provenance,
and auxiliary variables created by its transformations, so its ordinary
evaluation path is sufficient. The reported objective is the prepared
objective, including any finite penalty.

Preparation is not globally transactional. Existing in-place operations run
directly and retain their own failure semantics. The consuming penalty operation
is the only exception: it works on a clone of the current `Instance` and commits
that value only after penalty conversion and parameter materialization succeed.

The maximum of 53 auxiliary bits for each Integer variable belongs to OMMX's
Integer-to-Binary log-encoding operation. It is neither a property of the
OpenJij input class nor an `ommx.v2.Feature`.

OMMX does not yet implement `Kind::Spin`. Its addition, including direct
OpenJij Spin input, is tracked separately in
[OMMX issue #1082](https://github.com/Jij-Inc/ommx/issues/1082).
