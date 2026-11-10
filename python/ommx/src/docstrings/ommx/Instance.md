A unit of OMMX's intermediate representation for optimization problems.

An Instance holds both the current problem to solve and the information needed
to reconstruct and interpret its results against the user's input. These two
roles are called **Active Formulation** and **Interpretation Context**.

**Active Formulation** is the currently effective optimization problem:

- {attr}`~ommx.Instance.objective` and {attr}`~ommx.Instance.sense` define
  what is minimized or maximized.
- {attr}`~ommx.Instance.active_decision_variables` supplies the variables and
  their domains.
- {attr}`~ommx.Instance.active_constraints` supplies all active regular and special
  constraints.

Regular constraints have the form ``f(x) == 0`` or ``f(x) <= 0``;
``f(x) >= 0`` is represented as ``-f(x) <= 0``. Special constraints express
structure directly, such as conditional constraints
({class}`~ommx.IndicatorConstraint`), exactly one selected binary variable
({class}`~ommx.OneHotConstraint`), or at most one nonzero variable
({class}`~ommx.Sos1Constraint`). They are conditions of the problem, not
instructions to a particular solver. See {class}`~ommx.InstanceClass` for
the separate question of which representations a solver adapter accepts.

**Interpretation Context** is the information retained to recover variable
values and evaluate or explain results in terms of the user's input. It includes
fixed values, decision-variable dependencies, {attr}`~ommx.Instance.inactive_constraints`,
an {attr}`~ommx.Instance.output_objective`, {class}`~ommx.NamedFunction` expressions,
and relevant modeling labels and transformation provenance.

These names describe conceptual roles within the intermediate representation,
not separate SDK types or disjoint copies of the model. A retained variable
definition can serve both roles. Interpretation Context need not contain an
independent, complete copy of the original problem.

**Implementation contract:** {attr}`~ommx.Instance.active_decision_variables`
must contain exactly the variables referenced by the active objective and
every active constraint family, including structural uses such as an
Indicator's controlling variable. {meth}`~ommx.Instance.required_ids` returns
the same set of IDs. References occurring only in Interpretation Context must
not contribute to this set. Maintaining this correspondence is the
responsibility of the Instance implementation: construction, successful
mutations and transformations, and extensions with new constraint families
must preserve it. Adapters use `active_decision_variables` and `active_constraints`
as the variables and constraints of this formulation. The Instance derives the
active variable set; adapters do not need to collect variable references themselves.

**Transformation principle**

Transformations used to prepare a problem for solving normally rewrite the
Active Formulation and retain or update the necessary Interpretation Context
on the same Instance, or on the returned transformed Instance. For example:

- {meth}`~ommx.Instance.log_encode` changes the solver's variables and retains
  the expressions needed to reconstruct the encoded values. Named functions
  keep their original expressions and use those reconstructed values.
- {meth}`~ommx.Instance.partial_evaluate` fixes variables and simplifies active
  expressions. Named functions and existing inactive constraints retain their
  expressions and use the saved fixed values during evaluation.
- {meth}`~ommx.Instance.lower_special_constraints` retains source constraints
  and records the provenance of generated constraints.
- {meth}`~ommx.Instance.convert_active_objective` preserves the objective and
  sense used for output evaluation.
- {meth}`~ommx.Instance.reduce_binary_power` reduces Active Formulation
  expressions and captures the original objective when needed for output
  evaluation. It leaves existing Interpretation Context expressions unchanged.

An expression being evaluable by OMMX does not by itself make it a target of an
Active Formulation rewrite. Interpretation Context can nevertheless require
updates: {meth}`~ommx.ParametricInstance.with_parameters` specializes
parameter-bearing expressions in both roles, including inactive constraints,
output objectives, named functions, and dependency expressions.

Operations that explicitly change the user's problem or output semantics have
a different contract. {meth}`~ommx.Instance.as_minimization_problem` and
{meth}`~ommx.Instance.as_maximization_problem` convert both active and output
objectives; assigning a new {attr}`~ommx.Instance.objective` also rebases output
evaluation.

Consult each operation's rewrite scope and preservation guarantees. The
principle above does not imply that all current operations leave context
expressions unchanged or retain every source detail:

- {meth}`~ommx.Instance.convert_inequality_to_equality_with_integer_slack` and
  {meth}`~ommx.Instance.add_integer_slack_to_inequality` replace a rewritten
  source row in place; they do not retain that original row separately.
- {meth}`~ommx.Instance.tighten_bounds_simultaneously_once` updates the retained
  variable bounds without retaining their previous values as original-domain
  information.

Use the transformed Instance to interpret a candidate: {meth}`~ommx.Instance.populate_state`
reconstructs values, and {meth}`~ommx.Instance.evaluate` evaluates the populated
state. Reconstruction, feasibility, and optimality preservation are distinct.
Retaining Interpretation Context does not establish equivalence between the
original and active problems.

**Build and evaluate a problem**

This problem maximizes ``x + y`` subject to ``x * y == 0``, with binary
variables. {meth}`~ommx.Instance.maximize` starts an empty maximization
problem; use {meth}`~ommx.Instance.minimize` for minimization.

```pycon
>>> from ommx import ConstraintKind, Instance
>>> instance = Instance.maximize()
>>> x = instance.new_binary("x")
>>> y = instance.new_binary("y")
>>> instance.objective = x + y
>>> exclusive = instance.add_constraint(x * y == 0, "exclusive")
>>> solution = instance.evaluate({x.id: 1, y.id: 0})
>>> assert solution.feasible()
>>> assert solution.objective == 1.0

```

The ``new_*`` methods and {meth}`~ommx.Instance.add_constraint` assign IDs
automatically and return attached handles. IDs remain accessible through
``x.id`` and ``exclusive.constraint_id``. {meth}`~ommx.Instance.evaluate`
accepts a {class}`~ommx.State` or an ID-to-value dictionary and returns a
{class}`~ommx.Solution`. Evaluation checks the supplied candidate; it does
not search for a better one or establish optimality.

Use {meth}`~ommx.Instance.from_components` when components already have
explicit IDs. The Instance validates that expressions and special
constraints refer to variables defined in that Instance.

```pycon
>>> from ommx import ConstraintKind, ConstraintID, DecisionVariable, Instance, Sense
>>> a = DecisionVariable.binary(10, name="a")
>>> assembled = Instance.from_components(
...     decision_variables=[a],
...     objective=a,
...     active_constraints={7: a <= 1},
...     sense=Sense.Maximize,
... )
>>> assert ConstraintID(7) in assembled.constraints
>>> assert assembled.evaluate({a.id: 1}).feasible()

```

**Variable domains and modeling labels**

A variable's ``kind``, ``lower``, and ``upper`` define its domain. Binary
variables take 0 or 1, Integer variables take integers within their bounds,
and Continuous variables take real values within their bounds.
SemiInteger and SemiContinuous additionally allow zero even when zero is
outside the specified interval.

```pycon
>>> from ommx import ConstraintKind, Instance
>>> typed = Instance.minimize()
>>> flag = typed.new_binary("flag")
>>> count = typed.new_integer("count", lower=0, upper=10)
>>> amount = typed.new_continuous("amount", lower=0)
>>> batch = typed.new_semi_integer("batch", lower=2, upper=10)
>>> rate = typed.new_semi_continuous("rate", lower=0.5, upper=4)

```

Here ``batch`` permits 0 and the integers from 2 through 10; ``rate``
permits 0 and real values from 0.5 through 4. Each ``new_*`` method
documents its bound normalization, tolerance, and validation rules. An
invalid definition is rejected before adding the variable or its label.

Variables and constraints also carry ``name``, ``subscripts``,
``parameters``, and ``description`` labels. For example, ``name="x"``
and ``subscripts=[i]`` describe a source-model variable ``x_i``;
``parameters`` stores string-valued indices. Labels support display and
analysis without changing the mathematical conditions. Names need not be
unique: identity is determined by IDs, not names or labels.
All ``new_*`` methods and {meth}`~ommx.Instance.add_constraint` accept
these labels. For ``add_constraint``, omitted labels preserve the input
constraint's labels.

{meth}`~ommx.Instance.decision_variables_df` collects definitions and
labels into a pandas DataFrame. {meth}`~ommx.Instance.format_function` and
{meth}`~ommx.Instance.display_function` use the Instance's variable labels
to display expressions; a Function alone retains only variable IDs.
After evaluation, {meth}`~ommx.Solution.extract_decision_variables` uses
names and subscripts to retrieve values for application-level analysis.

**Constraint identity and access**

Constraint numbers are unique across all families in one Instance,
including inactive constraints. Their numbering is separate from variable
IDs. Supply distinct numbers across the regular and special-constraint
mappings passed to {meth}`~ommx.Instance.from_components`.
The Instance allocates new constraint numbers above the maximum across all
active and inactive families. Lowering and promotion allocate fresh numbers
for generated constraints while retained originals keep their IDs.
{class}`~ommx.ParametricInstance` follows the same rule, and evaluation
preserves those IDs in {class}`~ommx.Solution` and {class}`~ommx.SampleSet`.
Protobuf input with cross-family duplicate numbers is rejected without
automatic renumbering, including data from earlier v3 beta versions that
used separate number spaces.

{attr}`~ommx.Instance.constraints` is a live collection of all constraint
families and activeness states. A typed ID identifies the family and the number,
but not activeness. Bare integers are rejected by this heterogeneous collection
even though constraint numbers are globally unique within the Instance. Obtaining
the collection does not collect rows; each row is fetched when accessed.
{attr}`~ommx.Instance.active_constraints` exposes active attached handles;
{attr}`~ommx.Instance.inactive_constraints` exposes snapshots of retained
inactive constraints.

```pycon
>>> from ommx import ConstraintKind, ConstraintID, Instance
>>> model = Instance.minimize()
>>> selected = model.new_binary("selected")
>>> added = model.add_constraint(selected == 1, "required")
>>> assert added.constraint_id == ConstraintID(0)
>>> assert model.constraints[added.constraint_id].name == "required"
>>> assert list(model.constraints) == [added.constraint_id]

```

Use {meth}`~ommx.Instance.constraints_df` with an explicit ``kind``, such
as ``kind=ConstraintKind.Regular``, to collect one family into a DataFrame. Collection
lookup and iteration contracts belong to {class}`~ommx.ConstraintCollection`
and {class}`~ommx.ActiveConstraintCollection`.

**Edit the Instance through attached handles**

{attr}`~ommx.Instance.decision_variables`,
{meth}`~ommx.Instance.attached_decision_variable`, and active constraint
lookups return attached handles. Their label setters update the owning
Instance. The following example updates a variable and a regular constraint
after construction; the same operations work on a loaded Instance.
``set_name``, ``set_description``, ``set_subscripts``, and ``set_parameters``
replace their fields; ``add_subscripts`` appends indices and
``add_parameter`` / ``add_parameters`` merge parameter entries.

```pycon
>>> from ommx import ConstraintKind, Instance
>>> labeled = Instance.maximize()
>>> choice = labeled.new_binary("choice")
>>> labeled.objective = choice
>>> rule = labeled.add_constraint(choice <= 1, "limit")
>>> variable = labeled.attached_decision_variable(choice.id)
>>> constraint = labeled.active_constraints[rule.constraint_id]
>>> variable.set_name("selected")
>>> variable.set_subscripts([0])
>>> variable.set_description("Whether product 0 is selected")
>>> constraint.set_name("selection_limit")
>>> assert labeled.attached_decision_variable(choice.id).name == "selected"
>>> assert labeled.active_constraints[rule.constraint_id].name == "selection_limit"

```

Changing labels preserves IDs and mathematical conditions. Set labels
before evaluation when the resulting Solution should carry them. By
contrast, ``detach()`` and
{meth}`~ommx.Instance.get_decision_variable_by_id` return independent
snapshots. Editing a snapshot or a DataFrame does not write back to the
Instance.

```pycon
>>> snapshot = constraint.detach()
>>> _ = snapshot.set_name("snapshot_only")
>>> table = labeled.decision_variables_df()
>>> table.loc[choice.id, "name"] = "table_only"
>>> assert labeled.constraints[rule.constraint_id].name == "selection_limit"
>>> assert labeled.attached_decision_variable(choice.id).name == "selected"

```

**Retained variables and output evaluation**

{attr}`~ommx.Instance.decision_variables` contains every retained variable.
{attr}`~ommx.Instance.active_decision_variables` selects the variables of
the Active Formulation. Fixed and dependent variables
remain defined so that evaluation can reconstruct their values, although
the solver no longer chooses those values directly.

```pycon
>>> from ommx import ConstraintKind, Instance
>>> original = Instance.maximize()
>>> x = original.new_binary("x")
>>> y = original.new_binary("y")
>>> original.objective = x + y
>>> fixed = original.partial_evaluate({x.id: 1})
>>> assert {v.id for v in fixed.decision_variables} == {x.id, y.id}
>>> assert {v.id for v in fixed.active_decision_variables} == {y.id}
>>> assert fixed.evaluate({y.id: 0}).state.entries == {x.id: 1.0, y.id: 0.0}

```

Interpretation Context also includes information used only to interpret results:
{class}`~ommx.NamedFunction` expressions for reporting metrics, inactive
constraints for checking original conditions, and an
{class}`~ommx.OutputObjective` separate from the current solver-facing
objective. A variable appearing only in this information is not thereby
an active variable. {meth}`~ommx.Instance.populate_state` restores fixed and
dependent values before evaluation. An otherwise irrelevant variable is
assigned a default when omitted; that default is not a reconstructed
candidate value. Supply its value explicitly when it matters to the output.

Reconstruction does not by itself establish feasibility or equivalence
between the original and transformed optimization problems. Read the
guarantees of each transformation, such as {meth}`~ommx.Instance.substitute`,
and the output semantics of {meth}`~ommx.Instance.evaluate`.

For example, changing only the active objective can preserve the objective
reported after evaluation, including when all solver-input variables have
been fixed:

```pycon
>>> from ommx import ConstraintKind, DecisionVariable, Instance, Sense
>>> x = DecisionVariable.binary(0)
>>> instance = Instance.from_components(
...     decision_variables=[x],
...     objective=3 * x,
...     active_constraints={},
...     sense=Sense.Maximize,
... )
>>> assert instance.convert_active_objective(Sense.Minimize)
>>> fixed = instance.partial_evaluate({0: 1})
>>> assert fixed.sense == Sense.Minimize
>>> assert fixed.objective.evaluate({}) == -3.0
>>> assert fixed.required_ids() == set()
>>> assert fixed.active_decision_variables == []
>>> assert fixed.populate_state({}).entries == {0: 1.0}
>>> solution = fixed.evaluate({})
>>> assert (solution.sense, solution.objective) == (Sense.Maximize, 3.0)

```
