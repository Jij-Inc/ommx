A set of exact {class}`~ommx.Instance` representations, used for inspection and
as a target for {meth}`~ommx.Instance.prepare`.

Membership checks the Instance as it is: it does not convert expressions,
prepare a model, or run a solver. An Adapter's `INPUT_CLASS` is one use of this
common representation contract.

**Clauses describe alternatives**

An InstanceClass is a finite union of {class}`~ommx.InstanceClassClause` values.
An Instance belongs to the class if it satisfies every condition in at least one
clause. Conditions from different clauses are not combined. For example, a
union of continuous quadratic problems and mixed-integer linear problems does
not automatically include mixed-integer quadratic problems. An empty list of
clauses accepts no Instance.

A clause specifies the allowed kinds of used variables, the objective's
polynomial representation and degree, the relations and polynomial requirements
of regular constraints and Indicator bodies, whether active OneHot and SOS1
constraints are allowed, and the optimization direction. Its label identifies
it in diagnostics without changing membership.

**Inspect membership without changing the model**

This target accepts Binary variables, a linear objective, regular linear
equalities, and maximization. It does not accept active special constraints.

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
...     Sense,
... )
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
>>> instance = Instance.maximize()
>>> xs = [instance.new_binary("x", subscripts=[i]) for i in range(2)]
>>> instance.objective = 3 * xs[0] + 2 * xs[1]
>>> _ = instance.add_one_hot_constraint(OneHotConstraint(variables=xs))
>>> assert not target.contains(instance)
>>> report = target.check_membership(instance)
>>> assert not report.is_member
>>> assert report.matching_clauses == []
>>> assert report.clause_reports[0].clause_label == "binary-linear-equality"
>>> assert report.clause_reports[0].mismatches
>>> assert set(instance.constraints) == {OneHotConstraintID(0)}

```

OneHot means exactly one member is one. Although a linear equality can express
that condition for Binary variables, its dedicated representation does not
match a clause that permits only regular equalities. The check leaves OneHot
unchanged. {meth}`~ommx.Instance.prepare` contains a complete example converting
this representation and comparing candidate evaluations before and after it.

{meth}`~ommx.InstanceClass.contains` returns a Boolean.
{meth}`~ommx.InstanceClass.check_membership` returns an
{class}`~ommx.InstanceClassMembershipReport`; its `clause_reports` describe each
alternative, and `matching_clauses` identifies every matching index and label.
Each {class}`~ommx.InstanceClassClauseReport` exposes structured
{class}`~ommx.InstanceClassMismatch` values with the affected positions or IDs.

Alternatively, add a clause that accepts the dedicated OneHot representation:

```pycon
>>> with_one_hot = InstanceClass(
...     [
...         InstanceClassClause(
...             label="binary-linear-with-one-hot",
...             allowed_variable_kinds={Kind.Binary},
...             objective_polynomial_requirement=linear,
...             allowed_senses={Sense.Maximize},
...             allows_one_hot=True,
...         )
...     ]
... )
>>> combined = target.union(with_one_hot)
>>> assert combined.contains(instance)
>>> assert combined.check_membership(instance).matching_clauses == [
...     (1, "binary-linear-with-one-hot")
... ]
>>> assert not target.contains(instance)

```

**The active representation determines membership**

Variable-kind checks use {attr}`~ommx.Instance.used_decision_variables`: variables
referenced by the active objective and active constraints, including special
constraints. Fixed, dependent, and irrelevant variables are excluded, as are
variables referenced only by removed constraints, named functions, or the output
objective. Removed constraints do not contribute their relations, degrees, or
special-constraint kinds. A lowered Instance can therefore retain the original
special constraints for evaluation while matching a class that disallows them
as active constraints.

The stored {class}`~ommx.Function` representation matters as well as its
mathematical meaning. Polynomial requirements reject composed expressions even
when they happen to define a polynomial; see {meth}`~ommx.Function.powi` for an
example. Membership establishes representation requirements, not equivalence to
a previous model or a guarantee about the solution returned by a backend.

**QUBO is one Instance class**

{meth}`~ommx.InstanceClass.qubo` accepts Binary polynomial minimization of degree
at most two with no active constraints. Linear and constant objectives are
included. The model remains an OMMX Instance:

```pycon
>>> qubo = Instance.minimize()
>>> a = qubo.new_binary("a")
>>> b = qubo.new_binary("b")
>>> qubo.objective = a + 2 * a * b + 3
>>> assert InstanceClass.qubo().contains(qubo)
>>> coefficients, offset = qubo.as_qubo_format()
>>> assert coefficients == {(a.id, a.id): 1.0, (a.id, b.id): 2.0}
>>> assert offset == 3.0

```

{meth}`~ommx.Instance.as_qubo_format` extracts active-objective coefficients and
the constant without Preparation. Those values do not replace the Instance's
reconstruction rules, retained constraints, or output objective.
{meth}`~ommx.Instance.to_qubo` combines Preparation and formatting and changes
the Instance in place; {meth}`~ommx.Instance.prepare` shows the explicit path.

Use a concrete Adapter's `INPUT_CLASS` rather than inferring it from a solver
name: some samplers accept Binary polynomials beyond degree two. For membership,
recommended policies, and the distinction between automatic and explicit
Preparation, see {class}`~ommx.adapter.SolverAdapter` and
{class}`~ommx.adapter.SamplerAdapter`.
