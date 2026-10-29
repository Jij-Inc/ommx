Restore fixed and dependent values and fill omitted irrelevant values in a candidate.

Accepts a {class}`~ommx.State`, an ID-to-value mapping, or an iterable of ID/value
pairs. The returned State contains exactly the decision-variable IDs retained by
this Instance. Supply every variable used by its current objective and active
constraints; missing used values are errors, not opportunities to choose defaults.

Use {meth}`~ommx.Instance.evaluate` to populate a candidate and then evaluate its
objective, variable domains, and constraints, or
{meth}`~ommx.Instance.evaluate_samples` for multiple candidates. Population alone
does not establish feasibility or optimality.

**Fixed values and retained variables**

Fixing a variable removes it from the values a solver must choose, while retaining
its definition and fixed value for results. Here `partial_evaluate` returns a new
Instance; the original remains unchanged.

```pycon
>>> from ommx import DecisionVariableRole, Instance
>>> original = Instance.maximize()
>>> x = original.new_binary("x")
>>> y = original.new_binary("y")
>>> original.objective = x + y
>>> _ = original.add_constraint(x * y == 0)
>>> fixed = original.partial_evaluate({x.id: 1})
>>> assert {v.id for v in fixed.decision_variables} == {x.id, y.id}
>>> assert {v.id for v in fixed.used_decision_variables} == {y.id}
>>> assert fixed.decision_variable_role(x.id) == DecisionVariableRole.Fixed
>>> assert fixed.fixed_decision_variables() == {x.id: 1.0}
>>> assert fixed.populate_state({y.id: 0}).entries == {x.id: 1.0, y.id: 0.0}
>>> assert fixed.evaluate({y.id: 0}).objective == 1.0
>>> assert original.decision_variable_role(x.id) == DecisionVariableRole.Used

```

{attr}`~ommx.Instance.decision_variables` contains all retained variables.
{attr}`~ommx.Instance.used_decision_variables` selects solver-input variables,
including variables in active special constraints and an Indicator's controlling
Binary variable. Roles belong to an Instance and can change after transformation;
they are independent of the variable's Binary, Integer, or other kind. See
{class}`~ommx.DecisionVariableRole` for the four roles and the corresponding queries.

**Dependent values**

A substituted variable is restored from its recorded expression. In this example,
`count` is represented as twice the Binary variable `flag`.

```pycon
>>> from ommx import DecisionVariableRole, Instance
>>> encoded = Instance.minimize()
>>> count = encoded.new_integer("count", lower=0, upper=2)
>>> flag = encoded.new_binary("flag")
>>> encoded.objective = count + flag
>>> encoded.substitute({count.id: 2 * flag})
>>> assert encoded.decision_variable_roles() == {
...     count.id: DecisionVariableRole.Dependent,
...     flag.id: DecisionVariableRole.Used,
... }
>>> assert encoded.dependent_decision_variable_ids() == {count.id}
>>> assert encoded.populate_state({flag.id: 1}).entries == {count.id: 2.0, flag.id: 1.0}
>>> assert encoded.evaluate({flag.id: 1}).objective == 3.0

```

The restored `count=2` is in its domain, but `count=1` cannot be represented by
`2 * flag`. Reconstruction therefore does not prove that the original and
transformed problems are equivalent. {meth}`~ommx.Instance.substitute` does not
automatically turn the substituted variable's domain into new constraints; check
its transformation contract when choosing an expression.

**Irrelevant values and defaults**

An irrelevant variable is neither used, fixed, nor dependent. Its value is not
determined by optimization or reconstruction. When omitted, Binary, Integer, and
Continuous variables receive the value in their bound nearest to zero;
SemiInteger and SemiContinuous variables receive zero. The latter kinds allow
zero as well as values in their nonzero interval.

```pycon
>>> from ommx import DecisionVariable, DecisionVariableRole, Instance, Sense
>>> variables = [
...     DecisionVariable.binary(0),
...     DecisionVariable.integer(1, lower=2, upper=5),
...     DecisionVariable.continuous(2, lower=-5, upper=-2),
...     DecisionVariable.semi_integer(3, lower=2, upper=5),
...     DecisionVariable.semi_continuous(4, lower=2, upper=5),
... ]
>>> reporting = Instance.from_components(
...     decision_variables=variables, objective=0, constraints={}, sense=Sense.Minimize
... )
>>> assert reporting.irrelevant_decision_variable_ids() == {0, 1, 2, 3, 4}
>>> assert reporting.decision_variable_role(1) == DecisionVariableRole.Irrelevant
>>> assert reporting.populate_state({}).entries == {0: 0.0, 1: 2.0, 2: -2.0, 3: 0.0, 4: 0.0}
>>> assert reporting.populate_state({1: 4}).entries[1] == 4.0

```

A reference only from a named function, removed constraint, output objective, or
dependency right-hand side does not make a variable used. If such a variable is
irrelevant, omitting its value means these expressions use its default. Supply the
value explicitly when checking a particular candidate; the default is neither a
solver-selected value nor a reconstruction of that candidate.

**Canonicalization and consistency assertions**

For finite supplied coordinates that are neither fixed nor dependent, Binary
values at most `atol` away from zero or one and Integer or SemiInteger values at
most `atol` away from an integer are represented exactly. Derived dependent values
use the same target-kind rule. Continuous and SemiContinuous values are not
rounded. Other finite values remain available for feasibility checks.

A caller-provided fixed or dependent value is a consistency assertion, not an
override. After validation, the returned state uses the stored fixed value
unchanged or the canonicalized derived dependent value. Dependencies consume
canonicalized non-fixed, non-dependent inputs. A supplied dependent assertion is
compared with the value derived from those inputs before target-kind
canonicalization, so an assertion computed from the uncanonicalized solver vector
can be rejected as inconsistent.

```pycon
>>> assert fixed.populate_state({x.id: 1 - 2e-7, y.id: 0}, atol=1e-6).entries[x.id] == 1.0
>>> assert encoded.populate_state({flag.id: 1 - 2e-7, count.id: 2}, atol=1e-6).entries == {
...     count.id: 2.0,
...     flag.id: 1.0,
... }
>>> try:
...     encoded.populate_state({flag.id: 1, count.id: 1})
... except ValueError:
...     pass
... else:
...     raise AssertionError("a dependent assertion overwrote the recorded expression")
>>> assert encoded.populate_state({flag.id: 0.25}).entries[flag.id] == 0.25
>>> assert not encoded.evaluate({flag.id: 0.25}).feasible

```

`atol` defaults to {attr}`~ommx.DEFAULT_ATOL`. Missing used IDs, unknown IDs,
non-finite supplied values, and inconsistent fixed or dependent assertions raise
`ValueError`. Invalid finite values can remain in the populated state: use
{meth}`~ommx.Instance.evaluate` to inspect kind, bound, and constraint feasibility.

**Preserved output evaluation**

Fixed values are restored even when needed only by preserved output semantics.
Changing the solver-facing objective does not change which fixed value is restored:

```pycon
>>> from ommx import DecisionVariable, Instance, Sense
>>> reward = DecisionVariable.binary(0)
>>> instance = Instance.from_components(
...     decision_variables=[reward],
...     objective=3 * reward,
...     constraints={},
...     sense=Sense.Maximize,
... )
>>> assert instance.convert_active_objective(Sense.Minimize)
>>> fixed_reward = instance.partial_evaluate({0: 1})
>>> assert fixed_reward.populate_state({}).entries == {0: 1.0}
>>> assert fixed_reward.evaluate({}).objective == 3.0

```
