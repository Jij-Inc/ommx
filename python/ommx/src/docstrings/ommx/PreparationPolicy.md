Select optional transformations applied by {meth}`~ommx.Instance.prepare`.

A policy stores choices and settings; constructing or editing it does not
transform an Instance. The target {class}`~ommx.InstanceClass` describes the
required representation, this policy selects permitted operations, and
`Instance.prepare(input_class, policy)` applies them to that Instance in place.

Each property selects at most one phase. Properties may be combined freely,
although owner-operation validation and target membership can still make a
combination fail for a particular {class}`~ommx.Instance`. A policy is not a list
of operations that will always run: Preparation stops as soon as the whole
target class is reached, including before the first phase if already satisfied.
See {meth}`~ommx.Instance.prepare` for the complete execution example, phase
ordering, early stopping, and mutation on failure.

**Defaults and explicit selection**

A default policy selects no Preparation phase. Future phases also default to
disabled. The following policy initially leaves every transformation unselected,
then permits only lowering OneHot constraints to regular equalities:

```pycon
>>> from ommx import (
...     PreparationPolicy,
...     SpecialConstraintKind,
...     SpecialConstraintPreparation,
... )
>>> policy = PreparationPolicy()
>>> assert (
...     policy.special_constraints,
...     policy.objective,
...     policy.integer_slack,
...     policy.integer_encoding,
...     policy.fixed_penalty,
...     policy.binary_power_reduction,
... ) == (None, None, None, None, None, None)
>>> policy.special_constraints = SpecialConstraintPreparation.lower_special_constraints(
...     kinds={SpecialConstraintKind.OneHot}
... )
>>> assert policy.integer_encoding is None
>>> assert policy.fixed_penalty is None

```

The corresponding settings are {class}`~ommx.SpecialConstraintPreparation`,
{class}`~ommx.ObjectivePreparation`, {class}`~ommx.IntegerSlackPreparation`,
{class}`~ommx.IntegerEncodingPreparation`, {class}`~ommx.FixedPenaltyPreparation`,
and {class}`~ommx.BinaryPowerPreparation`. Unselected phases do not run. The
application chooses permitted transformations, numerical tolerances, and penalty
magnitudes; a target class alone does not decide them or prove the transformations
preserve the desired mathematical guarantees.

**QUBO, HUBO, and Adapter policies**

{meth}`~ommx.PreparationPolicy.for_qubo` and
{meth}`~ommx.PreparationPolicy.for_hubo` return fresh policies for their respective
formatting targets. Creating either policy does not prepare a model. Their
penalty defaults are documented choices, not a guarantee that an arbitrary
problem's desired candidate will be selected. In particular,
{meth}`~ommx.FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight`
explains why squaring an inequality's body differs from penalizing only its
violation.

The explicit path is to call `Instance.prepare` with the target and chosen
policy, then format the active objective if coefficients are needed. The complete
QUBO example is in {meth}`~ommx.Instance.prepare`. The convenience methods
{meth}`~ommx.Instance.to_qubo` and {meth}`~ommx.Instance.to_hubo` perform both
Preparation and coefficient formatting in place; `as_qubo_format` and
`as_hubo_format` perform formatting alone.

A generic QUBO/HUBO factory is separate from an Adapter's
`recommended_preparation_policy()`. An Adapter can accept a different class or
leave problem-specific choices to the application. For explicit policies followed
by strict execution, see {class}`~ommx.adapter.SolverAdapter` and
{class}`~ommx.adapter.SamplerAdapter`.
