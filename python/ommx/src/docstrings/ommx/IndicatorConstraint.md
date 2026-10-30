A condition enforced when a controlling Binary variable equals one.

For a Binary variable $z$, an Indicator requires $f(x)\leq0$ or $f(x)=0$
when $z=1$. At $z=0$, the Indicator is satisfied regardless of the body's
value; variable domains and other constraints still apply. This is a one-way
implication: a satisfied body does not require $z=1$.

Construct one from a regular constraint with {meth}`~ommx.Constraint.with_indicator`,
or pass `indicator_variable`, `function`, and `equality` to this class.
The controlling variable accepts a variable ID, a standalone
{class}`~ommx.DecisionVariable`, or an {class}`~ommx.AttachedDecisionVariable`.
These accepted forms are exposed as the `VariableIDLike` type alias.
Only its ID is stored. The enclosing Instance validates that the referenced
variables exist and that the controlling variable is Binary when inserting it.

**Construct and evaluate a conditional limit**

The following model limits the amount to five only when the facility is enabled:

```pycon
>>> from ommx import Equality, IndicatorConstraintID, Instance
>>> instance = Instance.maximize()
>>> enabled = instance.new_binary("enabled")
>>> amount = instance.new_continuous("amount", lower=0, upper=10)
>>> instance.objective = amount
>>> condition = (amount <= 5).with_indicator(enabled)
>>> assert condition.indicator_variable_id == enabled.id
>>> assert condition.equality == Equality.LessThanOrEqualToZero
>>> attached = instance.add_indicator_constraint(condition)
>>> cid = attached.constraint_id
>>> assert isinstance(cid, IndicatorConstraintID)
>>> off = instance.evaluate({enabled.id: 0, amount.id: 6})
>>> on = instance.evaluate({enabled.id: 1, amount.id: 5})
>>> violated = instance.evaluate({enabled.id: 1, amount.id: 6})
>>> assert off.feasible and on.feasible and not violated.feasible
>>> assert not off.constraints_df(kind="indicator").loc[cid, "indicator_active"]
>>> assert on.constraints_df(kind="indicator").loc[cid, "indicator_active"]

```

The `indicator_active` column distinguishes an inactive condition from an active
condition whose body is satisfied. Indicator results do not have a dual variable.
See {meth}`~ommx.Solution.constraints_df` for the precise columns and violation
semantics, and {meth}`~ommx.Instance.evaluate` for numerical tolerances and invalid
activation values.

**Identity, transformation, and solver use**

{meth}`~ommx.Instance.add_indicator_constraint` returns an attached handle. The
constructor above creates an independent value; context changes on it do not
edit a constraint already inserted into a host. Alternatively, provide explicit
IDs through the `indicator_constraints` mapping of
{meth}`~ommx.Instance.from_components`. Constraint numbers must be distinct
across all active and removed families, as described by {class}`~ommx.Instance`.

{meth}`~ommx.Instance.relax_indicator_constraint` moves a condition out of the
active formulation while retaining it for evaluation.
{meth}`~ommx.Instance.restore_indicator_constraint` makes it active again, subject
to the current variable dependencies and fixed values.

A backend may receive this structure directly or require regular constraints.
For explicit Big-M lowering, use {meth}`~ommx.Instance.convert_indicator_to_constraint`;
its finite-bound and numerical conditions are part of that operation's contract.
The {class}`PySCIPOpt Adapter <ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter>`
documents native handling and its complete solving examples.
