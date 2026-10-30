Require exactly one member of a nonempty set of Binary variables to equal one.

All other members must equal zero, so the mathematical condition is
$\sum_i x_i=1$. This is a structural constraint: it stores member IDs rather than
an explicit Function. Keeping the selection structure allows transformations and
result inspection to retain its meaning.

Each `variables` entry accepts a variable ID, a standalone
{class}`~ommx.DecisionVariable`, or an {class}`~ommx.AttachedDecisionVariable`.
These accepted forms are exposed as the `VariableIDLike` type alias.
Only their IDs are stored; duplicate entries identify the same member. The
constructor rejects an empty set. The enclosing Instance validates that every
referenced variable exists and is Binary when inserting the constraint.

**Construct and evaluate a choice**

```pycon
>>> from ommx import Instance, OneHotConstraint, OneHotConstraintID
>>> instance = Instance.maximize()
>>> choices = [instance.new_binary("choice", subscripts=[i]) for i in range(3)]
>>> instance.objective = sum(value * x for value, x in zip([5, 10, 3], choices))
>>> condition = OneHotConstraint(variables=choices)
>>> assert condition.variables == [x.id for x in choices]
>>> attached = instance.add_one_hot_constraint(condition)
>>> cid = attached.constraint_id
>>> assert isinstance(cid, OneHotConstraintID)
>>> chosen = instance.evaluate({choices[0].id: 0, choices[1].id: 1, choices[2].id: 0})
>>> assert chosen.feasible and chosen.objective == 10
>>> assert not instance.evaluate({x.id: 0 for x in choices}).feasible
>>> assert not instance.evaluate({x.id: 1 for x in choices}).feasible
>>> rows = chosen.constraints_df(kind="one_hot")
>>> assert list(rows.index) == [cid]
>>> assert rows.loc[cid, "violation"] == 0.0

```

Unlike {class}`~ommx.Sos1Constraint`, this condition does not allow selecting
no member. Numerical evaluation and reported violations use the contracts of
{meth}`~ommx.Instance.evaluate` and {meth}`~ommx.Solution.constraints_df`.

**Identity and lowering**

{meth}`~ommx.Instance.add_one_hot_constraint` returns an attached handle. The
constructor creates an independent value; context changes on it do not edit a
constraint already inserted into a host. For explicit IDs, use the
`one_hot_constraints` mapping of {meth}`~ommx.Instance.from_components`.
Constraint numbers share the active/removed cross-family space described by
{class}`~ommx.Instance`.

{meth}`~ommx.Instance.convert_one_hot_to_constraint` lowers the condition to a
regular equality, assigns the equality a fresh ID, and retains the original
OneHot for evaluation. Generated constraints carry provenance back to that
original; the method's example shows how to inspect the relationship.
{meth}`~ommx.Instance.lower_special_constraints` selects whole families to lower.
For automatic preparation and execution, consult the selected Adapter's
Reference, such as {class}`~ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter`.
