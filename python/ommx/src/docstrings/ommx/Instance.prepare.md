Apply the caller's `policy` to this Instance in place to reach `input_class`
membership.

The {class}`~ommx.InstanceClass` target describes required representation
conditions; {class}`~ommx.PreparationPolicy` selects permitted transformations.
Constructing either object does not change the Instance. Preparation executes
existing Instance-owned operations and checks their resulting representation.

**Convert a representation and evaluate the same candidates**

This complete example lowers a OneHot constraint to a regular linear equality.
OneHot means exactly one of the Binary variables is one. Save the initial
evaluations to compare them later; keeping the original Instance itself is only
necessary when it must be evaluated or reused separately after the mutation.

```pycon
>>> from ommx import (
...     Equality,
...     Instance,
...     InstanceClass,
...     InstanceClassClause,
...     Kind,
...     OneHotConstraint,
...     OneHotConstraintID,
...     PolynomialRequirement,
...     PreparationPolicy,
...     Sense,
...     SpecialConstraintKind,
...     SpecialConstraintPreparation,
... )
>>> instance = Instance.maximize()
>>> x = [instance.new_binary("x", subscripts=[i]) for i in range(2)]
>>> instance.objective = 3 * x[0] + 2 * x[1]
>>> _ = instance.add_one_hot_constraint(OneHotConstraint(variables=x))
>>> state = {x[0].id: 1, x[1].id: 0}
>>> both = {x[0].id: 1, x[1].id: 1}
>>> before = instance.evaluate(state)
>>> before_both = instance.evaluate(both)
>>> linear = PolynomialRequirement.at_most(1)
>>> target = InstanceClass(
...     [
...         InstanceClassClause(
...             label="binary-linear-equality",
...             allowed_variable_kinds={Kind.Binary},
...             objective_polynomial_requirement=linear,
...             regular_constraint_polynomial_requirements={Equality.EqualToZero: linear},
...             allowed_senses={Sense.Maximize},
...         )
...     ]
... )
>>> assert not target.contains(instance)
>>> policy = PreparationPolicy(
...     special_constraints=SpecialConstraintPreparation.lower_special_constraints(
...         kinds={SpecialConstraintKind.OneHot}
...     )
... )
>>> assert set(instance.constraints) == {OneHotConstraintID(0)}
>>> assert instance.prepare(target, policy) is None
>>> assert target.contains(instance)
>>> assert len(instance.constraints) == 1
>>> assert instance.active_special_constraint_kinds == set()
>>> assert set(instance.removed_constraints) == {OneHotConstraintID(0)}
>>> assert instance.constraints_df(kind="regular").shape[0] == 1
>>> after = instance.evaluate(state)
>>> assert before.feasible and after.feasible
>>> assert before.objective == after.objective == 3.0
>>> assert not before_both.feasible
>>> assert not instance.evaluate(both).feasible

```

The active formulation now contains `x[0] + x[1] == 1`; the original OneHot is
retained for evaluation in {attr}`~ommx.Instance.removed_constraints`. The exact
lowering contract is {meth}`~ommx.Instance.convert_one_hot_to_constraint`.
Agreement for the two candidates above illustrates this operation; checking a
few candidates does not prove the guarantees of arbitrary transformations.

**Phase order, early stopping, and guarantees**

Whole-class membership is checked before the first phase and after each selected
phase. Phases are applied at most once in this order, stopping as soon as
membership is reached:

1. `special_constraints`: {meth}`~ommx.Instance.lower_special_constraints`
2. `objective`: {meth}`~ommx.Instance.convert_active_objective`
3. `integer_slack`:
   {meth}`~ommx.Instance.convert_inequality_to_equality_with_integer_slack`,
   followed by {meth}`~ommx.Instance.add_integer_slack_to_inequality` only when
   exact conversion is unavailable and `slack_upper_bound` is set
4. `integer_encoding`: {meth}`~ommx.Instance.log_encode`
5. `fixed_penalty`: the selected {class}`~ommx.FixedPenaltyPreparation` operation
6. `binary_power_reduction`: {meth}`~ommx.Instance.reduce_binary_power`

If the input already belongs to the target class, no phase runs and the Instance
is unchanged. Unselected phases do not run. This is not a general loop that
selects or repeats preprocessing operations until convergence.

Success guarantees `input_class` membership. When `input_class` is an Adapter's
`INPUT_CLASS`, membership is its complete applicability condition; converter-local
or backend failures may still occur later. Membership alone does not establish
equivalence to the source problem, preservation of every feasible candidate, or
optimality of a returned result. Each selected operation owns those guarantees
and their domain, bound, and numerical conditions.

**QUBO preparation and coefficient formatting**

{meth}`~ommx.PreparationPolicy.for_qubo` constructs a policy without modifying a
model. Here Preparation changes the active maximization into minimization and
turns a regular equality into a penalty. The selected weight `2.0` is specific to
this example; the factory's default weight is not a sufficiency guarantee.

```pycon
>>> from ommx import (
...     DecisionVariable,
...     Instance,
...     InstanceClass,
...     Optimality,
...     PreparationPolicy,
...     Sense,
... )
>>> x = DecisionVariable.binary(0)
>>> instance = Instance.from_components(
...     decision_variables=[x], objective=x, constraints={7: x == 1}, sense=Sense.Maximize
... )
>>> policy = PreparationPolicy.for_qubo(uniform_penalty_weight=2.0)
>>> assert instance.prepare(InstanceClass.qubo(), policy) is None
>>> assert InstanceClass.qubo().contains(instance)
>>> assert instance.sense == Sense.Minimize
>>> assert instance.objective.evaluate({0: 0}) == 2.0
>>> solution = instance.evaluate({0: 0})
>>> assert (solution.sense, solution.objective) == (Sense.Maximize, 0.0)
>>> assert not solution.feasible
>>> assert instance.map_active_optimality(Optimality.Optimal) == Optimality.Unspecified
>>> coefficients, offset = instance.as_qubo_format()
>>> assert coefficients == {(0, 0): -3.0}
>>> assert offset == 2.0

```

{meth}`~ommx.Instance.as_qubo_format` performs no Preparation. It extracts the
active objective's coefficients and constant, which need not equal the output
objective used by `evaluate`. The Instance still holds reconstruction and retained
evaluation information; coefficients alone do not carry it to another system.
Keep the Instance when those semantics are needed for evaluation or storage.
{meth}`~ommx.Instance.to_qubo` combines Preparation and formatting in place.

For an Adapter, use its own `INPUT_CLASS` and recommended policy as the starting
point. A backend can accept higher-degree Binary polynomials or require choices
that a generic QUBO policy does not express. To execute a caller-selected policy,
prepare explicitly and then use `solve_without_preparation` or
`sample_without_preparation`; see {class}`~ommx.adapter.SolverAdapter` and
{class}`~ommx.adapter.SamplerAdapter`.

**Errors and mutation on failure**

Preparation is not globally transactional. Changes committed by an earlier owner
operation remain if a later operation raises an error. Within the Integer-slack
phase, active regular inequalities are processed in ascending constraint-ID order,
and a later failure leaves earlier conversions committed. Other phases retain the
mutation and failure semantics of their owner operations. If the original
Instance must remain available after failure, make a copy before calling
`prepare` and discard the partially prepared copy on error.

Existing Rust owner signals retain their Python exception mappings. When
configured phases are exhausted without reaching `input_class`,
{class}`~ommx.PreparationTargetNotReachedError` exposes the final membership report
through its `report` attribute:

```pycon
>>> from ommx import (
...     Instance,
...     InstanceClass,
...     PreparationPolicy,
...     PreparationTargetNotReachedError,
... )
>>> blocked = Instance.minimize()
>>> z = blocked.new_binary("z")
>>> blocked.objective = z
>>> _ = blocked.add_constraint(z == 1)
>>> target = InstanceClass.qubo()
>>> try:
...     blocked.prepare(target, PreparationPolicy())
... except PreparationTargetNotReachedError as error:
...     assert not error.report.is_member
...     assert error.report == target.check_membership(blocked)
... else:
...     raise AssertionError("an empty policy unexpectedly removed a constraint")

```
