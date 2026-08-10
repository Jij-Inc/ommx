# ommx-openjij-adapter

Provides an adapter between [OMMX](https://github.com/Jij-Inc/ommx) and
[OpenJij](https://www.openjij.org/).

## Usage

`ommx-openjij-adapter` can be installed from PyPI:

```bash
pip install ommx-openjij-adapter
```

OpenJij directly accepts a Binary, unconstrained minimization model through
this adapter. Prepare a constrained model in place before sampling it:

```python markdown-code-runner
from ommx import DecisionVariable, FixedPenaltyPreparation, Instance
from ommx_openjij_adapter import OMMXOpenJijSAAdapter

x = DecisionVariable.binary(0, name="x")
instance = Instance.from_components(
    decision_variables=[x],
    objective=x,
    constraints={0: x == 1},
    sense=Instance.MINIMIZE,
)

input_class = OMMXOpenJijSAAdapter.INPUT_CLASS
assert input_class is not None
policy = OMMXOpenJijSAAdapter.recommended_preparation_policy()
policy.fixed_penalty = FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(
    weight=2.0
)
instance.prepare(input_class, policy)
OMMXOpenJijSAAdapter.require_applicable(instance)

sample_set = OMMXOpenJijSAAdapter.sample(
    instance,
    num_reads=16,
)
print(sample_set.summary)
```

The finite penalty weight is part of the caller-owned preparation policy, not
an OpenJij backend sampler parameter. The adapter deliberately does not choose
one: a finite penalty is application-specific and does not guarantee that every
returned sample is feasible.

## Input class and preparation recommendation

`OMMXOpenJijSAAdapter.INPUT_CLASS` describes the instances that the adapter
accepts directly:

- Binary decision variables
- a polynomial objective of any degree (QUBO or Binary HUBO)
- no active regular or special constraints
- minimization

`check_applicability()` and the constructor are strict and never transform an
input. In addition to input-class membership, they check the OpenJij-specific
requirements that used variable IDs fit a signed 64-bit integer and converted
interaction coefficients are finite.

`recommended_preparation_policy()` returns a fresh editable
`ommx.PreparationPolicy`. It recommends:

- lowering active Indicator, OneHot, and SOS1 constraints;
- normalizing maximization to minimization;
- attempting exact Integer slack with range 32, while permitting
  inequality-preserving Integer slack with upper bound 32 when exact equality
  conversion is unavailable; and
- log-encoding all used Integer variables.

Finite penalty remains disabled until the caller selects either a uniform
weight or weights keyed by active regular constraint ID. Uniform weights are
usually the convenient choice when special-constraint lowering creates regular
constraints with generated IDs.

`Instance.prepare()` mutates the same `Instance` that is then passed to the
adapter. Success guarantees membership in `INPUT_CLASS`; the adapter-specific
preconditions must still be checked normally. Preparation uses the shared OMMX
owner operations and their existing exception types. It is not globally
transactional, so changes completed before a later error remain.

The mutated instance remains the evaluation owner for the returned `SampleSet`.
It retains the dependency and removed-constraint data needed for evaluation;
there is no separate OpenJij preparation result or source-reconstruction API.
If an application separately needs the pre-transformation model, it should
copy that model before calling `prepare()`.

The exact representability and bit-count limits of Integer log encoding belong
to the OMMX encoding operation, not to the OpenJij input class or an
`ommx.v2.Feature`. OMMX does not yet implement `Kind::Spin`; direct OpenJij Spin
input is tracked separately in
[OMMX issue #1082](https://github.com/Jij-Inc/ommx/issues/1082).
