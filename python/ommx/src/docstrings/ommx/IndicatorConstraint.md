A condition enforced when a controlling Binary variable equals its trigger value.

For a Binary variable $z$, an Indicator requires $f(x)\leq0$ or $f(x)=0$
when $z=1$ by default. With `trigger_value=0`, it is enforced when $z=0$.
Only 0 and 1 are valid trigger values.
At the other binary value, the Indicator is satisfied regardless of the body's
value; variable domains and other constraints still apply. This is a one-way
implication: a satisfied body does not force the variable to its trigger value.

Construct one from a regular constraint with {meth}`~ommx.Constraint.with_indicator`,
or pass `indicator_variable`, `function`, and `equality` to this class.
The controlling variable accepts a variable ID, a standalone
{class}`~ommx.DecisionVariable`, or an {class}`~ommx.AttachedDecisionVariable`.
These accepted forms are exposed as the `VariableIDLike` type alias.
Its ID and trigger value are stored. The enclosing Instance validates that the referenced
variables exist and that the controlling variable is Binary when inserting it.

**Construct and evaluate a conditional limit**

The following model limits the amount to five only when the facility is enabled:

```pycon
>>> from ommx import ConstraintKind, Equality, IndicatorConstraintID, Instance
>>> instance = Instance.maximize()
>>> enabled = instance.new_binary("enabled")
>>> amount = instance.new_continuous("amount", lower=0, upper=10)
>>> instance.objective = amount
>>> condition = (amount <= 5).with_indicator(enabled)
>>> assert condition.indicator_variable_id == enabled.id
>>> assert condition.trigger_value == 1
>>> assert condition.equality == Equality.LessThanOrEqualToZero
>>> attached = instance.add_indicator_constraint(condition)
>>> cid = attached.constraint_id
>>> assert isinstance(cid, IndicatorConstraintID)
>>> off = instance.evaluate({enabled.id: 0, amount.id: 6})
>>> on = instance.evaluate({enabled.id: 1, amount.id: 5})
>>> violated = instance.evaluate({enabled.id: 1, amount.id: 6})
>>> assert off.feasible and on.feasible and not violated.feasible
>>> assert not off.constraints_df(kind=ConstraintKind.Indicator).loc[cid, "indicator_on"]
>>> assert on.constraints_df(kind=ConstraintKind.Indicator).loc[cid, "indicator_on"]

```

Pass `trigger_value=0` to enforce the body when the controlling variable is zero:

```pycon
>>> disabled_limit = (amount <= 5).with_indicator(enabled, trigger_value=0)
>>> assert disabled_limit.trigger_value == 0

```

The constructor accepts the same keyword. Attached, inactive, evaluated, and
sampled rows expose the integer `trigger_value`; Indicator DataFrames retain it
as a column. This is the constraint's configuration, independently of its ON/OFF
state at a particular candidate.

The `indicator_on` column records whether the controlling variable equals its
trigger value, using the evaluation tolerance. It distinguishes an OFF
Indicator from an ON Indicator whose body is satisfied. This is independent of
`role`, which records whether the constraint belongs to the current
formulation and its default feasibility participation. An active constraint can
have its Indicator OFF; an inactive constraint can have its Indicator ON.
Indicator results do not have a dual variable.
See {meth}`~ommx.Solution.constraints_df` for the precise columns and violation
semantics, and {meth}`~ommx.Instance.evaluate` for numerical tolerances and invalid
controlling-variable values.

**Identity, transformation, and solver use**

{meth}`~ommx.Instance.add_indicator_constraint` returns an attached handle. The
constructor above creates an independent value; context changes on it do not
edit a constraint already inserted into a host. Alternatively, provide explicit
IDs through the `active_indicator_constraints` mapping of
{meth}`~ommx.Instance.from_components`. Constraint numbers must be distinct
across all active and inactive families, as described by {class}`~ommx.Instance`.

{meth}`~ommx.Instance.deactivate_indicator_constraint` moves a condition out of the
active formulation while retaining it for evaluation.
{meth}`~ommx.Instance.activate_indicator_constraint` makes it active again, subject
to the current variable dependencies and fixed values.

A backend may receive this structure directly or require regular constraints.
For explicit Big-M lowering, use {meth}`~ommx.Instance.convert_indicator_to_constraint`;
its finite-bound and numerical conditions are part of that operation's contract.
The {class}`PySCIPOpt Adapter <ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter>`
documents native handling and its complete solving examples.
