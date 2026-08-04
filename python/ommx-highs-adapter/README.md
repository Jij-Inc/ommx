# ommx-highs-adapter

Provides an adapter to translate between [OMMX](https://github.com/Jij-Inc/ommx) and [HiGHS](https://highs.dev)

# Usage

`ommx-highs-adapter` can be installed from PyPI as follows:

```bash
pip install ommx-highs-adapter
```

An example usage of HiGHS through this adapter:

```python markdown-code-runner
from ommx_highs_adapter import OMMXHighsAdapter
from ommx import Instance, DecisionVariable

x1 = DecisionVariable.integer(1, lower=0, upper=5)
ommx_instance = Instance.from_components(
    decision_variables=[x1],
    objective=x1,
    constraints={},
    sense=Instance.MINIMIZE,
)

# Create `ommx.Solution` through `highspy.Highs`
ommx_solution = OMMXHighsAdapter.solve(ommx_instance)

print(ommx_solution)
```

The direct Adapter APIs accept only instances in
`OMMXHighsAdapter.INPUT_CLASS` and never prepare their input implicitly. For
example, prepare an instance containing a special constraint through the
Adapter's recommended common OMMX Policy:

```python markdown-code-runner
from ommx import DecisionVariable, Instance, OneHotConstraint
from ommx_highs_adapter import OMMXHighsAdapter

x = [DecisionVariable.binary(i) for i in range(2)]
instance = Instance.from_components(
    decision_variables=x,
    objective=x[0] + 2 * x[1],
    constraints={},
    one_hot_constraints={0: OneHotConstraint(variables=x)},
    sense=Instance.MINIMIZE,
)

policy = OMMXHighsAdapter.recommended_preparation_policy()
instance.prepare(policy)
solution = OMMXHighsAdapter.solve(instance)
print(solution)
```

The recommended Policy permits OMMX's existing Indicator, OneHot, and SOS1
lowering operations. `Instance.prepare()` mutates the same `Instance` and checks
that it belongs to the Policy's `input_class`, copied from HiGHS `INPUT_CLASS`.
The direct HiGHS boundary remains responsible for its own applicability check.
The Adapter already evaluates the solver state against that `Instance`, which
retains the dependency, removed-constraint, provenance, and auxiliary-variable
data produced by its transformations.

Preparation is not globally transactional: a later owner operation can fail
after earlier operations succeeded. Each operation retains its owner API's
failure semantics. The fixed-weight penalty owner API validates first, then
clones internally and commits only after penalty conversion and parameter
materialization succeed.
