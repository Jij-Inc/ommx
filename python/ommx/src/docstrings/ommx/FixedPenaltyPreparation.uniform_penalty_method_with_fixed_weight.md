Select the uniform fixed-weight penalty owner operation.

`weight` is the common $w$ in the {class}`~ommx.FixedPenaltyPreparation` formula
for every active regular constraint. This replaces $F(x)$ by
$F(x) + w \sum_i g_i(x)^2$ for minimization and by
$F(x) - w \sum_i g_i(x)^2$ for maximization, after normalizing $w$.
Creating this setting does not apply it; select it in a
{class}`~ommx.PreparationPolicy` passed to {meth}`~ommx.Instance.prepare`.

`atol` is used by the owner operation to accept a penalty magnitude down to
`-atol` and normalize a tolerated negative value to zero. When at least one active
regular constraint is converted, the weight must be finite. With no active
constraint of any family, the owner operation is an identity and does not inspect
the weight. Removed constraints record
`reason="ommx.Instance.uniform_penalty_method_with_fixed_weight"` with no reason
parameters. The class documentation defines atomicity, retained output semantics,
and rejection of active special constraints.

**An inequality body is not its violation**

For $x \in \{0,1\}$, minimize $x$ subject to $x-1\leq0$. Both values are feasible,
and the original objective prefers zero. Squaring the inequality's body with
weight two gives $x + 2(x-1)^2$, which instead prefers one:

| $x$ | Original objective $x$ | Squared body $(x-1)^2$ | Penalized objective |
| --- | --- | --- | --- |
| 0 | 0 | 1 | 2 |
| 1 | 1 | 0 | 1 |

This differs from the one-sided penalty $\max(0,x-1)^2$, which is zero for both
candidates. Increasing the weight does not fix an unsuitable penalty formula.
The following applies only the selected fixed-penalty phase, so the inequality
body is squared directly:

```pycon
>>> from ommx import FixedPenaltyPreparation, Instance, InstanceClass, PreparationPolicy
>>> instance = Instance.minimize()
>>> x = instance.new_binary("x")
>>> instance.objective = x
>>> _ = instance.add_constraint(x <= 1)
>>> before_zero = instance.evaluate({x.id: 0})
>>> before_one = instance.evaluate({x.id: 1})
>>> assert before_zero.feasible and before_one.feasible
>>> assert (before_zero.objective, before_one.objective) == (0.0, 1.0)
>>> policy = PreparationPolicy(
...     fixed_penalty=FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(
...         weight=2.0
...     )
... )
>>> instance.prepare(InstanceClass.qubo(), policy)
>>> assert instance.objective.evaluate({x.id: 0}) == 2.0
>>> assert instance.objective.evaluate({x.id: 1}) == 1.0
>>> assert instance.evaluate({x.id: 0}).feasible
>>> assert instance.evaluate({x.id: 0}).objective == 0.0

```

The reported objective still describes the preserved output problem; inspect
feasibility separately from the penalized objective. To penalize an inequality as
an equality with a suitable slack variable, use an appropriate conversion first,
such as {meth}`~ommx.Instance.convert_inequality_to_equality_with_integer_slack`.
Its integer-domain, range, and numerical preconditions still apply. Choosing the
penalty formula and a sufficient weight belongs to the application.
