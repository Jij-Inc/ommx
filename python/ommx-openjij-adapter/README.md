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
    uniform_penalty_method_with_weight=2.0,
)
instance.prepare(OMMXOpenJijSAAdapter.INPUT_CLASS, policy)

sample_set = OMMXOpenJijSAAdapter.sample(
    instance,
    num_reads=16,
)

print(sample_set.summary)
```

`uniform_penalty_method_with_weight` stores the argument for the existing
`Instance.uniform_penalty_method_with_weight()` operation. It is part of
`PreparationPolicy`, not an OpenJij backend sampler parameter, and must be
selected explicitly when constraints remain. A finite penalty does not ensure
that every returned sample is feasible for the original constraints; inspect
the evaluated `SampleSet`.

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

`recommended_preparation_policy()` returns the common OMMX policy recommended
by the Adapter. Independently, `INPUT_CLASS` describes the Adapter's direct
inputs; the caller supplies that class separately as the target of
`Instance.prepare()`. Each Policy field is named after an existing `Instance`
owner operation and stores its arguments. The default recommendation includes:

- `lower_special_constraints` for Indicator, OneHot, and SOS1;
- `log_encode_used_integers` with the SDK default absolute tolerance;
- `as_minimization_problem=True`;
- `convert_inequality_to_equality_with_integer_slack=(32, atol)`; and
- `add_integer_slack_to_inequality=32`.

For every active regular inequality, preparation first invokes
`Instance.convert_inequality_to_equality_with_integer_slack()` with maximum
range 32. Only `ExactIntegerSlackError` selects
`Instance.add_integer_slack_to_inequality()` as a fallback, using slack upper
bound 32. Other failures propagate. Special-constraint lowering runs first, so
any generated regular inequalities participate in the same slack stage; no
manual slack processing is required.

Finite penalties remain disabled unless
`uniform_penalty_method_with_weight` or `penalty_method_with_weights` is
supplied; those two keyword arguments cannot be supplied together. The
recommendation resolves the SDK default absolute tolerance and stores it
independently for each operation that uses it. To choose different operation
arguments, construct `PreparationPolicy` directly.

Per-constraint penalty weights use regular-constraint IDs. Both Integer-slack
operations preserve those IDs. Every active row must have a weight; an
entry for a regular constraint already moved to
`removed_constraints` is ignored. Special-constraint lowering adds fresh
regular IDs, which a per-constraint map must also cover. Use a uniform penalty
weight when those generated IDs are not configured explicitly.

`Instance.prepare(OMMXOpenJijSAAdapter.INPUT_CLASS, policy)` mutates the caller's
`Instance` until it belongs to the supplied input class. It does not run the
Adapter or check backend-specific preconditions. The direct Adapter boundary
remains authoritative:

```python
OMMXOpenJijSAAdapter.require_applicable(instance)
```

The Adapter already evaluates returned samples against that same `Instance`.
It retains the decision-variable dependencies, removed constraints, provenance,
and auxiliary variables created by its transformations, so its ordinary
evaluation path is sufficient. The reported objective is the prepared
objective, including any finite penalty.

Preparation is not globally transactional: a later owner operation can fail
after earlier operations succeeded. Each operation retains its owner API's
failure semantics. Fixed-penalty conversion does not commit a partial
conversion.

The maximum of 53 auxiliary bits for each Integer variable belongs to OMMX's
Integer-to-Binary log-encoding operation. It is neither a property of the
OpenJij input class nor an `ommx.v2.Feature`.

OMMX does not yet implement `Kind::Spin`. Its addition, including direct
OpenJij Spin input, is tracked separately in
[OMMX issue #1082](https://github.com/Jij-Inc/ommx/issues/1082).
