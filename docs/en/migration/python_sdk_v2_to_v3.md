(python-sdk-v2-to-v3-migration-guide)=
# Python SDK v2 to v3 Migration Guide

This guide compares [Python SDK 2.9.4](https://pypi.org/project/ommx/2.9.4/) with the v3 API on `main`, the revision described by this documentation. Keep your v2 environment and follow the [installation guide](../getting_started/install.md) in a separate environment. Save a representative model and candidate so you can compare their objective value and feasibility after migration.

## Find the changes that affect your code

- **If you build models**, change where you assign constraint numbers. The [side-by-side example](#migration-compare-model) follows the same model through construction, evaluation, and result lookup.
- **If you analyze results**, distinguish a signed constraint value from a nonnegative violation. Call DataFrame accessors as methods and select the constraint family. See {class}`~ommx.Solution` and {class}`~ommx.SampleSet` for executable examples.
- **If you call a solver**, decide whether to use the adapter's recommended preparation or specify your own transformations. Find your backend in the [adapter catalogue](../api/adapters).
- **If you read saved data**, use `Instance.from_v1_bytes(data)` for payloads produced by `Instance.to_bytes()` in 2.9.4. For a `.ommx` archive, follow the [Artifact migration](#13-artifact-api-archive-becomes-an-exchange-format).

### Features you can already use in 2.9.4

You can import seven classes directly from `ommx` in 2.9.4: `Function`, `Constraint`, `DecisionVariable`, `Instance`, `ParametricInstance`, `Solution`, and `SampleSet`. You can switch these imports before migrating model construction and result handling. SCIP also continues to accept initial values keyed by variable name and subscripts through `initial_state`.

Version 2.9.4 already preserves annotations in model and result payloads. In v3, replace direct edits to `annotations` with `add_user_annotation` or a metadata property such as `title`. When exchanging models with 2.9.4, choose a representation its SDK can read. For example, its protobuf schema includes finite-domain variables, but the 2.x SDK rejects those variables when loading the model. See {meth}`~ommx.DecisionVariable.finite_domain` for the v3 representation.

## 1. Import changes

### 1.1 Protobuf submodules are gone (`3.0.0a1`, [#776](https://github.com/Jij-Inc/ommx/pull/776))

Every `ommx.v1.*_pb2` module and `ommx.v1.annotation` is removed. Import SDK classes from top-level `ommx`.

**Before (v2.9.4)**:
```python
from ommx.v1.constraint_pb2 import Constraint, Equality
from ommx.v1.function_pb2 import Function
from ommx.v1.linear_pb2 import Linear
from ommx.v1.solution_pb2 import State
```

**After (v3)**:
```python
from ommx import Constraint, Equality, Function, Kind, Linear, Sense, State
```

The `.from_protobuf()` / `.to_protobuf()` bridge methods on `Constraint`, `RemovedConstraint`, `DecisionVariable`, etc. are removed along with the protobuf objects they produced. Use explicit versioned bytes APIs on whole serialization roots instead. Prefer `to_v2_bytes` / `from_v2_bytes` when producing new serialized bytes; use `to_v1_bytes` / `from_v1_bytes` only when you need to interoperate with legacy v1 payloads.

### 1.2 Constraint-hint helper types removed (`3.0.0a1`, [#776](https://github.com/Jij-Inc/ommx/pull/776); `3.0.0a2`, [#790](https://github.com/Jij-Inc/ommx/pull/790), [#798](https://github.com/Jij-Inc/ommx/pull/798))

`ConstraintHints`, `OneHot`, `Sos1`, and the `Parameters` wrapper are no longer exported. They are superseded by the first-class constraint types (`OneHotConstraint`, `Sos1Constraint`, `IndicatorConstraint`) and plain `dict[int, float]` for parameter substitution.

Legacy v1 `ConstraintHints` remain advisory metadata when read by v3. `Instance.from_v1_bytes(...)` and `ParametricInstance.from_v1_bytes(...)` ignore them and preserve the regular source constraints; they do not automatically promote hints to first-class special constraints. When that capability is required, construct the corresponding first-class constraints from trusted modeling input rather than from the ignored hint alone.

**Before (v2.9.4)**:
```python
from ommx.v1 import OneHot, Sos1, ConstraintHints, Parameters
```

**After (v3)**:
```python
from ommx import OneHotConstraint, Sos1Constraint, IndicatorConstraint
# Parameters is gone — pass a plain dict[int, float] to ParametricInstance.with_parameters
```

## 2. Removal of `.raw` and `from_raw` / `from_protobuf` / `to_protobuf` (`3.0.0a1`, [#770](https://github.com/Jij-Inc/ommx/pull/770), [#771](https://github.com/Jij-Inc/ommx/pull/771), [#774](https://github.com/Jij-Inc/ommx/pull/774), [#775](https://github.com/Jij-Inc/ommx/pull/775))

v2 deprecated `.raw`, v3 removes it. All migrated classes are direct Rust types; there is no separate underlying object.

**Affected classes**: `Linear`, `Quadratic`, `Polynomial`, `Function`, `NamedFunction`, `DecisionVariable`, `Parameter`, `Instance`, `ParametricInstance`, `Solution`, `SampleSet`, `Constraint`, `RemovedConstraint`, `Bound`, `DecisionVariableAnalysis`.

**Before (v2.9.4)**:
```python
linear.raw.linear_terms
instance.raw.sense
solution.raw.optimality = Optimality.Optimal
constraint.raw.id
Linear.from_raw(rust_linear)
Constraint.from_protobuf(pb_constraint)
dv.to_protobuf()
```

**After (v3)**:
```python
linear.linear_terms
instance.sense
solution.optimality = Optimality.Optimal
# constraint.id is gone — see §3
Linear(terms={0: 1.0}, constant=0.0)
instance.to_v2_bytes()          # serialize whole Instance / Solution / SampleSet as ommx.v2
```

The dataclass-style constructors (`Instance(raw=..., annotations=...)`) are also gone — `Instance`, `Solution`, `SampleSet` are no longer Python `@dataclass`es. Construct through `Instance.from_components(...)` etc., set OMMX metadata through dedicated properties such as `instance.title`, and store user annotations with `instance.add_user_annotation(...)` or `instance.replace_annotations(...)`. The `annotations` property is a read-only projection in v3.

`Constraint`, `EvaluatedConstraint`, `SampledConstraint`, and `RemovedConstraint` no longer have `to_bytes` / `from_bytes` — a single constraint cannot meaningfully carry its ID on its own now that IDs live in the enclosing dict, so the per-constraint serialisation API was removed. Serialise the containing `Instance` / `Solution` / `SampleSet` instead. (3.0.0a3 extends the same removal to the rest of the non-top-level types — `Function` / `Linear` / `Quadratic` / `Polynomial`, `Parameter`, `NamedFunction` family, `DecisionVariable` family — see §12.)

## 3. Constraint IDs moved out of the `Constraint` object

### 3.1 No more `id` / `set_id()` / `id=` kwarg (`3.0.0a2`, [#806](https://github.com/Jij-Inc/ommx/pull/806))

In v2, `Constraint(id=7, ...)` and `(x <= 1).set_id(7)` assign a number to a regular constraint. In v3 a newly created constraint has no ID. Choose its number with the mapping key in `Instance.from_components`, or let an existing model allocate one when adding the constraint (§3.3).

Use the corresponding mapping key for a OneHot or SOS1 constraint as well. Its `variables` argument accepts variable IDs, `DecisionVariable` objects, or `AttachedDecisionVariable` objects. Include each referenced variable in the model's `decision_variables`.

(migration-compare-model)=
### 3.2 Compare the same model and candidate in both SDKs

Give a binary variable an upper limit and evaluate the candidate $x=0$.

**v2.9.4**

```python
from ommx import DecisionVariable, Instance

x = DecisionVariable.binary(0)
limit = (x <= 1).set_id(7).add_name("limit")
model = Instance.from_components(
    decision_variables=[x], objective=x, constraints=[limit], sense=Instance.MAXIMIZE,
)
result = model.evaluate({0: 0})
assert result.extract_constraints("limit")[()] == -1.0
```

**v3 (`main`)**

```python
from ommx import ConstraintID, DecisionVariable, Instance, Sense

x = DecisionVariable.binary(0)
model = Instance.from_components(
    decision_variables=[x], objective=x, constraints={7: x <= 1}, sense=Sense.Maximize,
)
model.constraints[ConstraintID(7)].set_name("limit")
result = model.evaluate({0: 0})
assert result.extract_constraints("limit")[()] == 0.0
assert result.get_constraint_value(ConstraintID(7)) == -1.0
```

The constraint number is 7 in both versions. In v2 you assign it to the expression. In v3 you supply it as a mapping key when building the model. The candidate meets the same limit in both. The v2 lookup returns the signed value $x-1=-1$ and the v3 lookup returns zero violation. Use {meth}`~ommx.Solution.get_constraint_value` if your analysis needs the signed value.

### 3.3 Global ID-counter helpers removed (`3.0.0a2`, [#806](https://github.com/Jij-Inc/ommx/pull/806))

These module-level names are gone from `ommx._ommx_rust`:

- `CONSTRAINT_ID_COUNTER`
- `next_constraint_id()`
- `set_constraint_id_counter(...)`
- `update_constraint_id_counter(...)`
- `get_constraint_id_counter()`

For a new regular constraint `c`, `instance.add_constraint(c).constraint_id` adds the constraint and returns its assigned `ConstraintID`.

## 4. Constructing and looking up constraints by ID

Assign constraint numbers from a single sequence per model, including retained removed constraints. If your code restarts numbering for each family, allocate distinct numbers across the model instead. Constructors and protobuf loading reject cross-family collisions.

### 4.1 `Instance.from_components(constraints=...)` expects a `dict[int, Constraint]` (`3.0.0a2`, [#806](https://github.com/Jij-Inc/ommx/pull/806))

All constraint-valued arguments are keyed by ID. `decision_variables` stays a `Sequence[DecisionVariable]`.

**Before (v2.9.4)**:
```python
Instance.from_components(
    sense=Instance.MINIMIZE,
    objective=obj,
    decision_variables=[x0, x1],
    constraints=[c0, c1],                 # list; IDs came from Constraint.id
    constraint_hints=ConstraintHints()    # optional hints
)
```

**After (v3)**:
```python
Instance.from_components(
    sense=Sense.Minimize,
    objective=obj,
    decision_variables=[x0, x1],
    constraints={0: c0, 1: c1},           # dict keyed by constraint ID
    indicator_constraints={10: ic},       # all structural-constraint args are dicts
    one_hot_constraints={20: oh},
    sos1_constraints={30: sc},
)
```

All arguments are keyword-only. `ParametricInstance.from_components` takes the same `constraints: Mapping[int, Constraint]` shape.

(42-constraint-accessors-on-instance--parametricinstance--solution-return-dicts)=
### 4.2 Constraint access uses typed-ID collections

**Before (v2.9.4)**:
```python
for c in instance.constraints:              # list[Constraint]
    print(c.id, c.function)

for rc in instance.removed_constraints:     # list[RemovedConstraint]
    ...

for ec in solution.constraints:             # list[EvaluatedConstraint]
    print(ec.id, ec.evaluated_value)

hints = instance.constraint_hints           # one_hot_constraints / sos1_constraints inside
for oh in hints.one_hot_constraints:
    ...
```

**After (v3)**:
```python
from ommx import ConstraintID, OneHotConstraintID

for cid in instance.constraints:  # all families, typed IDs
    if isinstance(cid, ConstraintID):
        print(cid, instance.constraints[cid].function)
    elif isinstance(cid, OneHotConstraintID):
        print(cid, instance.constraints[cid].variables)

for cid, removed in instance.removed_constraints.items():
    print(cid, removed.removed_reason)

for cid in solution.constraints:
    if isinstance(cid, ConstraintID):
        print(cid, solution.constraints[cid].evaluated_value)
```

In v3, `.constraints` is a live collection over all constraint families.
`Instance` and `ParametricInstance` return attached handles for active rows;
`.removed_constraints` returns removed snapshots. `Solution` and `SampleSet`
return individual result snapshots, including retained removed rows. See the
[typed collection migration](#typed-constraint-ids-and-live-collections).
`SampleSet.decision_variables` and `.named_functions` remain lists.

## 5. Renames, signature changes, and behavior changes

### 5.1 `write_mps` → `save_mps` (`3.0.0a1`, [#775](https://github.com/Jij-Inc/ommx/pull/775))

```python
# v2.9.4
instance.write_mps("out.mps.gz")

# v3
instance.save_mps("out.mps.gz")                 # compress=True by default
instance.save_mps("out.mps", compress=False)
```

### 5.2 `Instance.used_decision_variable_ids()` → `Instance.required_ids()` (`3.0.0a2`, [#806](https://github.com/Jij-Inc/ommx/pull/806))

```python
# v2.9.4
instance.used_decision_variable_ids()
func.used_decision_variable_ids()               # on Function as well

# v3
instance.required_ids()
func.required_ids()
```

(`used_decision_variable_ids()` is still the name on `EvaluatedConstraint`, `SampledConstraint`, `EvaluatedDecisionVariable`, `EvaluatedNamedFunction`, `SampledNamedFunction`.)

### 5.3 `Parameter.new(id=...)` → `Parameter(id, ...)` (`3.0.0a1`, [#770](https://github.com/Jij-Inc/ommx/pull/770))

The `.new` factory is removed; the `id` argument is positional.

```python
# v2.9.4
p = Parameter.new(id=3, name="w", subscripts=[0])

# v3
p = Parameter(3, name="w", subscripts=[0])
```

### 5.4 `ParametricInstance.with_parameters` takes a plain dict (`3.0.0a1`, [#774](https://github.com/Jij-Inc/ommx/pull/774))

The `Parameters(entries=...)` wrapper is gone.

```python
# v2.9.4
from ommx.v1 import Parameters
pi.with_parameters(Parameters(entries={p.id: 1.0}))

# v3
pi.with_parameters({p.id: 1.0})
```

### 5.5 `Linear(terms=..., constant=...)` always takes `dict[int, float]` (`3.0.0a1`, [#770](https://github.com/Jij-Inc/ommx/pull/770), [#776](https://github.com/Jij-Inc/ommx/pull/776))

v2.9.4 had a protobuf form (`Linear(terms=[Linear.Term(id=j, coefficient=c) for ...], constant=-b)`) via `linear_pb2`. In v3 `terms` is always `dict[int, float]` and `Linear.Term` does not exist.

```python
# v2.9.4 (protobuf path)
from ommx.v1.linear_pb2 import Linear
Linear(
    terms=[Linear.Term(id=j, coefficient=c) for j, c in enumerate(row)],
    constant=-b,
)

# v3
from ommx import Linear
Linear(terms={int(j): float(c) for j, c in enumerate(row)}, constant=float(-b))
```

### 5.6 `to_qubo()` / `to_hubo()` preserve the input objective in evaluated output (`3.0.0`, [#1167](https://github.com/Jij-Inc/ommx/pull/1167))

The driver methods remain available and still mutate their input. In v3, the
mutated `Instance` keeps the minimization energy sent to the QUBO or HUBO
solver as its active objective, while `evaluate()` and `evaluate_samples()`
retain the objective semantics that the instance exposed before conversion.
Python SDK v2 instead restored the active sense and evaluated the final
penalized energy, mixing solver input with user-facing output.

Code that reads `Instance.objective` therefore sees the solver energy; code
that consumes `Solution` or `SampleSet` sees the preserved input objective.
The executable postconditions are documented on {meth}`~ommx.Instance.to_qubo`,
{meth}`~ommx.Instance.to_hubo`, {meth}`~ommx.Instance.evaluate`, and
{meth}`~ommx.Instance.evaluate_samples`.

An `Instance` or `ParametricInstance` with an explicit output objective cannot
be represented losslessly by the v1 wire format. Its `to_v1_bytes()` method
raises `RuntimeError`; use `to_v2_bytes()` for these models.

The same pipeline can be run explicitly with
{meth}`~ommx.Instance.prepare` followed by
{meth}`~ommx.Instance.as_qubo_format` or
{meth}`~ommx.Instance.as_hubo_format`. The matching target classes and editable
policies are provided by {meth}`~ommx.InstanceClass.qubo`,
{meth}`~ommx.InstanceClass.hubo`,
{meth}`~ommx.PreparationPolicy.for_qubo`, and
{meth}`~ommx.PreparationPolicy.for_hubo`.

### 5.7 Adapter `solve()` / `sample()` prepare inputs automatically (`3.0.0`, [#1166](https://github.com/Jij-Inc/ommx/pull/1166))

For the usual workflow, pass the original Instance directly to `solve()` or
`sample()`. The Adapter applies its recommended Preparation without modifying
that Instance:

```python
from ommx_highs_adapter import OMMXHighsAdapter

solution = OMMXHighsAdapter.solve(instance)
```

When the application needs a custom Preparation policy, prepare the Instance
in place and use the preparation-free API:

```python
policy = OMMXHighsAdapter.recommended_preparation_policy()
# Customize policy here.
instance.prepare(OMMXHighsAdapter.INPUT_CLASS, policy)
solution = OMMXHighsAdapter.solve_without_preparation(instance)
```

Custom Adapter implementations must provide `solve_without_preparation()` or
`sample_without_preparation()`. Declare every additional option with an
explicit typed signature rather than a catch-all `**kwargs`. An easy method may
forward an option whose meaning survives Preparation. When an option depends on
the exact prepared Adapter input and the Adapter defines no way to transport it
through Preparation, the concrete Adapter may instead expose that option only
on its preparation-free method.

HiGHS and Python-MIP no longer attach dual values when the Adapter input has an
`output_objective`.

(constraint-violation-migration)=
### 5.8 Constraint violation metrics ([#1213](https://github.com/Jij-Inc/ommx/pull/1213))

Rename `solution.total_violation_l1()` to {meth}`~ommx.Solution.total_violation`.
`total_violation_l2()` is removed. The total is the sum of one nonnegative scalar
per constraint, including removed constraints and all special constraint kinds.
Regular equality and inequality violations retain their existing definitions.

```python
from ommx import ConstraintKind, OneHotConstraintID
solution.total_violation()
solution.constraint_violation(OneHotConstraintID(30))
solution.constraints_df(kind=ConstraintKind.Sos1)[["feasible", "violation"]]
```

Indicator constraints contribute the inner violation when active and zero otherwise.
OneHot uses the minimum sum of absolute changes to make exactly one member 1 and
the rest 0. SOS1 uses the minimum sum of absolute changes to leave at most one
nonzero member. These definitions do not depend on Big-M lowering.

Every constraint is feasible exactly when `violation <= atol`. For OneHot and
SOS1, tolerance applies to the total absolute change, rather than to each member
independently. For example, SOS1 values `(1, 0.00006, 0.00006)` have violation
`0.00012` and are infeasible with `atol=0.0001`, although each small member is
individually within tolerance of zero.

Variable bounds and kinds are checked separately. Discrete-value canonicalization
happens before constraint evaluation; the metric uses the resulting values.
Solution feasibility requires each constraint to pass its own threshold, not
the total violation to be within one shared threshold.

SampleSet feasibility and best-feasible selection also check variable bounds
and kinds at the saved tolerance, matching the extracted Solution. A sample can
therefore have zero `total_violation()` and still be infeasible because a
variable is outside its domain.

Constraint-level queries require an explicit tolerance. Replace
`evaluated.feasible` with `evaluated.is_feasible(atol=...)` and
`sampled.feasible` with `sampled.feasible(atol=...)` (a sample-ID-to-bool map).
Solution/SampleSet keep their existing feasibility properties and expose
`feasibility_atol` so extracted constraints can use the same threshold:

```python
from ommx import ConstraintID
evaluated = solution.constraints[ConstraintID(1)]
evaluated.is_feasible(atol=solution.feasibility_atol)
evaluated.is_feasible(atol=1e-8)  # A separate query on the same stored violation
```

Changing the query tolerance does not repeat input canonicalization or activation
classification. Tolerances are retained only with tolerance-dependent decisions:
regular constraint values retain none, while special-constraint activation
records retain the conditions used to classify them. The separation of tolerance
roles and the persisted evaluation context is tracked in
[issue #1180](https://github.com/Jij-Inc/ommx/issues/1180).

The v2 protobuf format retains `feasible` flags/maps and `feasibility_atol` on
Solution/SampleSet. Consumers can read the result and its tolerance without an
OMMX SDK. The protobuf schema is unchanged. When loading the data, the SDK
recomputes OneHot/SOS1 violations from the saved variable values and validates
the flags using the persisted tolerance.

The legacy v1 format cannot store the tolerance or native special constraints.
`to_v1_bytes()` recomputes feasibility from the retained regular constraints and
variable values using the SDK's default tolerance, as `from_v1_bytes()` does.
Use the same default tolerance when writing and reading v1 data. This conversion
may change feasibility; use v2 to preserve the original tolerance and special
constraints.

Partial evaluation rejects a fixing when eliminating an approximately zero
structural member could change feasibility under the sum of member errors and
the constraint must remain active. Exact-zero elimination remains supported.
Evaluate the complete state, or lower special constraints before partial evaluation; lowering
has its own violation metrics as described below.
Lowering need not preserve the total: retained originals and generated constraints
each contribute. See {meth}`~ommx.Solution.total_violation` for the full definitions.

## 6. Return-type changes

### 6.1 `Constraint.name` / `Constraint.description` are `Optional[str]` (`3.0.0a1`, [#770](https://github.com/Jij-Inc/ommx/pull/770), [#771](https://github.com/Jij-Inc/ommx/pull/771))

For a regular constraint, both 2.9.4 and v3 return `None` when the name or description is absent. Code that already checks `constraint.name is None` can keep that check.

### 6.2 `Linear.terms` / `Quadratic.terms` / `Polynomial.terms` are methods, not properties (`3.0.0a2`, [#806](https://github.com/Jij-Inc/ommx/pull/806))

Only `Function.terms` remains a property. The three building-block types switched to methods.

```python
# v2.9.4
linear.terms                                     # property
quadratic.terms                                  # property
polynomial.terms                                 # property

# v3
linear.terms()                                   # method call
quadratic.terms()
polynomial.terms()
```

`Linear.linear_terms`, `Quadratic.linear_terms` / `quadratic_terms`, and `Polynomial.constant_term` stay properties.

### 6.3 Working with `DecisionVariable.kind`

The initial v3 alpha migration in `3.0.0a1` ([#770](https://github.com/Jij-Inc/ommx/pull/770)) exposed protobuf integer codes through the `DecisionVariable` constructor and `kind` properties. The current constructor accepts a `Kind` value, and detached, attached, evaluated, and sampled variables return that type from `.kind`.

Compare against known constants such as `Kind.Binary` and `Kind.Integer`. New SDKs can add kinds, so handle any remaining value explicitly. `Kind` does not support iteration or `Enum.__members__`. Existing aliases such as `DecisionVariable.BINARY` still refer to the corresponding kind.

The same compatibility policy applies to `Constraint.EQUAL_TO_ZERO` / `LESS_THAN_OR_EQUAL_TO_ZERO` and `Instance.MINIMIZE` / `MAXIMIZE`: they remain typed aliases, while new code should use `Equality.*` and `Sense.*`.

```python
from ommx import Bound, DecisionVariable, Kind

var = DecisionVariable(0, Kind.Binary, Bound(0, 1))
assert var.kind == Kind.Binary

# Typed compatibility alias.
assert DecisionVariable.BINARY == Kind.Binary
```

Each known kind compares equal to its protobuf integer code and has the same hash.

```python
assert Kind.Binary == 1
assert hash(Kind.Binary) == hash(1)
```

### 6.4 `SampleSet.sample_ids` changed from list-property to set-method (`3.0.0a1`, [#775](https://github.com/Jij-Inc/ommx/pull/775))

```python
# v2.9.4
ids: list[int] = sample_set.sample_ids           # @property

# v3
ids: set[int]  = sample_set.sample_ids()         # method
ids: list[int] = sample_set.sample_ids_list      # separate property when you need a list
```

### 6.5 Evaluation input errors raise `ValueError`, not `RuntimeError` (`3.0.0a1`, [#770](https://github.com/Jij-Inc/ommx/pull/770))

Every `.evaluate(state)` method on `Linear`, `Quadratic`, `Polynomial`, `Function`, `Constraint`, `NamedFunction`, and `Instance` now raises `ValueError` (e.g. `ValueError: state is missing required variable IDs: {VariableID(2)}`) when the state is missing a required decision-variable ID or the atol is invalid. Partial evaluation intentionally accepts missing IDs, but raises `ValueError` for invalid provided entries, invalid tolerance, or inconsistent or unverifiable dependent-variable assertions. For example, `Instance.evaluate` with a missing required variable raises `RuntimeError` in 2.9.4. Update `except` clauses accordingly.

```python
# v2.9.4
try:
    linear.evaluate({1: 1})
except RuntimeError as e:
    ...

# v3
try:
    linear.evaluate({1: 1})
except ValueError as e:
    ...
```

### 6.6 `ParametricInstance.parameters` returns `list[Parameter]`, use `parameters_df()` for the DataFrame (`3.0.0a1`, [#774](https://github.com/Jij-Inc/ommx/pull/774); `3.0.0a3`, [#846](https://github.com/Jij-Inc/ommx/pull/846))

The DataFrame view moved to a separate `_df` accessor, mirroring `decision_variables` / `decision_variables_df()` and `constraints` / `constraints_df(kind=ConstraintKind.Regular)`. The bare `parameters` attribute is now an ordered `list[Parameter]`. The split itself landed in 3.0.0a1 (#774) when `ParametricInstance` became a Rust re-export; the `_df` accessor flipped from a `#[getter]` property to a method call in 3.0.0a3 (#846), at which point every `*_df` on `Instance` / `ParametricInstance` / `Solution` / `SampleSet` requires parentheses (see §9 below).

```python
# v2.9.4 (DataFrame view)
parametric_instance.parameters            # -> pandas.DataFrame

# v3
parametric_instance.parameters            # -> list[Parameter]
parametric_instance.parameters_df()       # -> pandas.DataFrame  (method, not property)
```

### 6.7 `Function.degree()` / `num_terms()` are optional for composed functions (`3.0.0`, [#1158](https://github.com/Jij-Inc/ommx/pull/1158))

In v2.9.4 every `Function` was a polynomial, so `degree()` and `num_terms()`
always returned `int`. A v3 `Function` can also represent a composed scalar
expression such as an absolute value, pointwise minimum or maximum, division,
or signed integer power. These two methods return `None` for that representation.
The polynomial coefficient properties (`terms`, `linear_terms`,
`quadratic_terms`, and `constant_term`) and `content_factor()` raise `TypeError`
for a composed function.

```python
# v2.9.4
degree: int = function.degree()
terms = function.terms

# v3
degree: int | None = function.degree()
if degree is None:
    # Inspect or evaluate the composed expression without treating it as a
    # coefficient map.
    value = function.evaluate(state, atol=1e-6)
else:
    terms = function.terms
```

Use explicit multiplication when you need a non-negative integer power to
remain an expanded polynomial. `function.powi(n)` and `function**n` deliberately
construct composed power expressions, including when `n >= 0`.

## 7. Removed helpers (`3.0.0a1`, [#770](https://github.com/Jij-Inc/ommx/pull/770), [#776](https://github.com/Jij-Inc/ommx/pull/776), [#782](https://github.com/Jij-Inc/ommx/pull/782); `3.0.0a2`, [#798](https://github.com/Jij-Inc/ommx/pull/798); `3.0.0`, [#1087](https://github.com/Jij-Inc/ommx/pull/1087))

- `Linear.from_object(x)` — construct via `Linear.single_term(...)`, `Linear.constant(...)`, or the arithmetic operators.
- `Linear.equals_to(other)` — use `linear.almost_equal(other, atol=...)`. (Available on every expression type.)
- `instance.constraint_hints` — replaced by the all-family `instance.constraints` collection.
- `Parameters` / `OneHot` / `Sos1` / `ConstraintHints` — see §1.2.
- `Artifact` low-level types (`ArtifactArchive`, `ArtifactDir`, `ArtifactArchiveBuilder`, `ArtifactDirBuilder`) — replaced by unified `Artifact` / `ArtifactDraft`.
- `ommx_openjij_adapter.response_to_samples(response)` — use `decode_to_samples(response)`.
- `ommx_openjij_adapter.sample_qubo_sa(...)` — use
  `OMMXOpenJijSAAdapter.sample(instance)`. The replacement leaves `instance`
  unchanged and returns an evaluated `SampleSet`, rather than raw `Samples`.

In v2, the OpenJij Adapter constructor, `sample()`, and `solve()` accepted
`uniform_penalty_weight`, `penalty_weights`, and
`inequality_integer_slack_max_range` directly. In v3 these choices moved to the
recommended `PreparationPolicy`:

- `uniform_penalty_weight` and `penalty_weights` map to `policy.fixed_penalty`;
- `inequality_integer_slack_max_range` maps to `policy.integer_slack`.

Direct constructor calls remain preparation-free and must receive an Instance
that has already been prepared for `OMMXOpenJijSAAdapter.INPUT_CLASS`.

OpenJij `initial_state` is defined over the solver-variable representation of
that exact prepared input; dictionary keys are those variable IDs. In v3 it is
accepted by the exact-input constructor, `sample_without_preparation()`, and
`solve_without_preparation()`, but not by the easy methods.

The usual `sample()` / `solve()` workflow is described in §5.7. When a model
requires a fixed penalty, the application must choose its magnitude; v3 no
longer supplies v2's implicit uniform weight of `1.0`. Apply the customized
policy to the Instance, then call the preparation-free method:

```python
from ommx import FixedPenaltyPreparation
from ommx_openjij_adapter import OMMXOpenJijSAAdapter

policy = OMMXOpenJijSAAdapter.recommended_preparation_policy()
policy.fixed_penalty = (
    FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(weight=20.0)
)
source.prepare(OMMXOpenJijSAAdapter.INPUT_CLASS, policy)
samples = OMMXOpenJijSAAdapter.sample_without_preparation(source)
```

`prepare()` modifies `source` in place. The returned `SampleSet` reports the
objective and sense exposed by `source` before Preparation (§5.6). The
[OpenJij tutorial](../tutorial/tsp_sampling_with_openjij_adapter) covers policy
details and penalty selection.

```python
# v2.9.4
from ommx.artifact import ArtifactArchive, ArtifactDir
archive = ArtifactArchive.from_oci_archive(path)
dir_art = ArtifactDir.from_oci_dir(path)

# v3
from ommx.artifact import Artifact
artifact = Artifact.import_archive("path/to/file.ommx")   # imports a .ommx archive into the Local Registry
manifest = Artifact.inspect_archive("path/to/file.ommx")  # read-only manifest / layer descriptor view
artifact = Artifact.load("ghcr.io/jij-inc/ommx/...")      # remote registry
```

## 8. Migrate constraint metadata setters

Rename v2's `add_name` and `add_description` to `set_name` and `set_description`. Use `set_parameters` to replace the entire mapping and `add_parameters` to update individual keys. On a detached constraint each setter mutates the receiver and returns a clone. Make separate calls on the same object. After construction, update the attached row using its typed ID.

## 9. DataFrame accessors are methods, with `kind=` / `include=` / `removed=` (`3.0.0a3`, [#846](https://github.com/Jij-Inc/ommx/pull/846), [#847](https://github.com/Jij-Inc/ommx/pull/847))

Every `*_df` accessor on `Instance` / `ParametricInstance` / `Solution` / `SampleSet` is a method call now, and the per-kind family on each host (`constraints_df`, `indicator_constraints_df`, `one_hot_constraints_df`, `sos1_constraints_df`, plus the parallel `removed_*_constraints_df` and `*_removed_reasons_df` families) collapsed into one `constraints_df(kind=...)` per host. Optional column families are gated by an `include=` parameter.

```python
# v2.9.4
df = instance.constraints_df             # property, regular constraints only
df = instance.removed_constraints_df     # separate active vs. removed
df = solution.constraints_df

# v3
from ommx import ConstraintKind

df = instance.constraints_df(kind=ConstraintKind.Regular)           # method; kind is required
df = instance.constraints_df(kind=ConstraintKind.Indicator)
df = instance.constraints_df(kind=ConstraintKind.Regular, removed=True)
                                         # active + removed merged in id order
df = solution.constraints_df(kind=ConstraintKind.Regular)           # no removed= (no active/removed
                                         # distinction at the evaluated stage)
```

`kind` is a required keyword argument accepting `ConstraintKind.Regular`,
`ConstraintKind.Indicator`, `ConstraintKind.OneHot`, or `ConstraintKind.Sos1`.
String selectors raise `TypeError`. Solution / SampleSet have no `removed=`
parameter — at the evaluated / sampled stage every row is materialized regardless
of how it was lifecycled, and reason data is gated by `"removed_reason"` in `include=` instead.

`include` accepts a `Sequence[str]` of `"label"` / `"parameters"` / `"removed_reason"` (singular). The default (`None`) preserves the v2 wide shape (`("label", "parameters")`); `include=[]` drops every optional column family.

```python
from ommx import ConstraintKind

# Default — v2-equivalent shape (label + parameters columns)
df = instance.constraints_df(kind=ConstraintKind.Regular)

# Core only — drop label and parameters
df = instance.constraints_df(kind=ConstraintKind.Regular, include=[])

# Active + removed in one DataFrame; reason columns auto-added
df = instance.constraints_df(kind=ConstraintKind.Regular, removed=True)
# columns include: equality, function_type, used_ids,
#                  name, subscripts, description, parameters.{key},
#                  removed_reason, removed_reason.{key}

# decision_variables_df takes include= but no kind= or removed=
df = instance.decision_variables_df()
df = instance.decision_variables_df(include=[])
```

`"removed_reason"` is a unit flag — it gates both the `removed_reason` column and the `removed_reason.{key}` parameter columns together. The `removed_reason` column is **schema-stable**: when the flag is on it always appears in the resulting DataFrame, NA-filled if no row carries a reason, so downstream code that branches on schema doesn't need to special-case empty data.

The wide `constraints_df(kind=ConstraintKind.Regular)` index column was renamed from unqualified `id` to `{kind}_constraint_id` (`regular_constraint_id`, `indicator_constraint_id`, `one_hot_constraint_id`, `sos1_constraint_id`). `decision_variables_df()` keeps `id` as its index name (only one variable ID space, so disambiguation isn't load-bearing); the long-format variable sidecars in §10 do use `variable_id`. Constraint indexes now contain family-specific ID objects. Constraint numbers must be unique across all families, including removed constraints, within each instance. The ID type still identifies the constraint family.

## 10. Long-format sidecar DataFrames (`3.0.0a3`, [#846](https://github.com/Jij-Inc/ommx/pull/846))

`Instance` / `ParametricInstance` / `Solution` / `SampleSet` gained six long-format / id-indexed sidecar DataFrame methods that read directly from the SoA label/context stores:

```python
from ommx import ConstraintKind

# Constraint-side — kind= dispatches across the four constraint families
instance.constraint_context_df(kind=ConstraintKind.Regular)
                                          # name, subscripts, description
                                          # index: regular_constraint_id
instance.constraint_parameters_df(kind=ConstraintKind.Regular)
                                          # columns: regular_constraint_id, key, value
instance.constraint_provenance_df(kind=ConstraintKind.Regular)
                                          # columns: regular_constraint_id, step,
                                          #          source_kind, source_id
instance.constraint_removed_reasons_df(kind=ConstraintKind.Regular)
                                          # columns: regular_constraint_id, reason,
                                          #          key, value

# Variable-side — single ID space, no kind=
instance.variable_labels_df()
instance.variable_parameters_df()
```

Use these for tidy-data joins / aggregation; reach for the wide `constraints_df(kind=ConstraintKind.Regular)` (with `include=`) when you want one row per id with columns alongside.

`provenance` is intentionally not folded into `constraints_df(kind=ConstraintKind.Regular)` via `include=`: chains have variable length, and a wide pivot would either explode the column space or produce an object-dtype list column. Pivot the long-format `constraint_provenance_df(kind=ConstraintKind.Regular)` yourself if you need a wide view.

## 11. Constraint and variable accessors return `AttachedX` write-through handles (`3.0.0a3`, [#849](https://github.com/Jij-Inc/ommx/pull/849), [#850](https://github.com/Jij-Inc/ommx/pull/850), [#852](https://github.com/Jij-Inc/ommx/pull/852))

The dict / list accessors that previously returned snapshot wrapper objects now return `AttachedX` write-through handles bound to the parent host (`Instance` or `ParametricInstance`). Reads pull live from the host's SoA stores; label/context setters write back through to them.

```python
# v2.9.4 — id-keyed lookup via get_constraint_by_id; snapshot wrapper,
# mutation didn't propagate to the instance
c = instance.get_constraint_by_id(5)
c.add_name("balance")                              # mutated the local snapshot
print(instance.get_constraint_by_id(5).name)       # still None — fresh snapshot

# v3 — collection lookup returns a write-through handle
from ommx import ConstraintID

c = instance.constraints[ConstraintID(5)]                        # AttachedConstraint (live)
c.set_name("balance")                              # writes through to the SoA store
print(instance.constraints[ConstraintID(5)].name)                # "balance"
```

Affected return types (the column for `3.0.0a2` shows the earlier snapshot dictionaries; the current API uses collections):

| Accessor | v2.9.4 | 3.0.0a2 | Current v3 |
|---|---|---|---|
| `instance.constraints` | `list[Constraint]` | `dict[int, Constraint]` | `ConstraintCollection` |
| `instance.indicator_constraints` | — (no indicator type) | `dict[int, IndicatorConstraint]` | Use `constraints[IndicatorConstraintID(...)]` |
| `instance.one_hot_constraints` | via `constraint_hints` (legacy `OneHot`) | `dict[int, OneHotConstraint]` | Use `constraints[OneHotConstraintID(...)]` |
| `instance.sos1_constraints` | via `constraint_hints` (legacy `Sos1`) | `dict[int, Sos1Constraint]` | Use `constraints[Sos1ConstraintID(...)]` |
| `instance.decision_variables` | `list[DecisionVariable]` | `list[DecisionVariable]` | `list[AttachedDecisionVariable]` |

The list → dict shape change happened in 3.0.0a2, and 3.0.0a3 introduced `AttachedX` write-through handles. The current `ConstraintCollection` retains that attached behavior and unifies all families under typed-ID lookup. The same behavior applies on `ParametricInstance`. Solution / SampleSet evaluated / sampled values remain individual snapshots fetched from their owning result.

The snapshot wrapper types (`Constraint`, `IndicatorConstraint`, `OneHotConstraint`, `Sos1Constraint`, `DecisionVariable`) are unchanged in shape and remain the modeling-input type — operator overloading (`x + y == 1`), expression building, and `Instance.from_components(constraints={...})` all keep accepting / returning them. New `add_*` entry points consume snapshots and return the matching attached handle:

```python
c = (x[0] + x[1] == 1).set_name("balance")     # Constraint snapshot

attached = instance.add_constraint(c)          # -> AttachedConstraint
attached.set_subscripts([0])                   # writes through

# Single-id lookup also returns an attached handle
print(instance.constraints[attached.constraint_id].name)   # "balance"

# attached_decision_variable(id) is the dedicated lookup for variables
av = instance.attached_decision_variable(0)
av.set_name("x_0")
```

`AttachedX` exposes `.detach()` to materialize an independent snapshot when you need one (e.g. to send through `from_components`, ship as part of a whole root via `to_v1_bytes` / `to_v2_bytes`, or hand off to code that expects the modeling type). `AttachedDecisionVariable` participates in arithmetic via `ToFunction` (only its id is consumed, no host borrow is taken), so existing expression-building code keeps working without `.detach()`.

Two `AttachedX` instances pointing at the same id observe the same state — a write through one is visible through any other and through the next `*_df` call. The host stays alive as long as any `AttachedX` references it (the handle holds a refcounted `Py<Instance>` / `Py<ParametricInstance>`); drop the handles to release the host.

### 11.1 Fixed decision-variable values live on the owning instance ([#959](https://github.com/Jij-Inc/ommx/pull/959))

Detached `DecisionVariable` objects remain modeling snapshots for the variable definition and label, but they no longer carry owner-side fixed-value state. Fixed values produced by `partial_evaluate(...)` or parsed from legacy protobuf `substituted_value` fields are stored on the owning `Instance` / `ParametricInstance`, not inside the detached variable object. As a result, `DecisionVariable.substituted_value` is no longer available on detached variables.

Use the owner when you need fixed-value state:

```python
fixed = instance.fixed_decision_variables()

attached = instance.attached_decision_variable(1)
assert attached.substituted_value == fixed.get(1)

df = instance.decision_variables_df()
print(df["substituted_value"])
```

`decision_variables_df()` on `Instance` and `ParametricInstance` continues to include the `substituted_value` column, but that value is populated from the owner-side fixed-value table. If you call `.detach()` on an `AttachedDecisionVariable`, the result is a modeling snapshot and no longer carries the owner-side fixed value.

### 11.2 Direct replacements for `decision_variable_analysis()`

The Python SDK no longer exposes the old analysis object shape. Query the owning `Instance` directly for the role or role-derived sets you need:

```python
roles = instance.decision_variable_roles()
role = instance.decision_variable_role(1)

fixed = instance.fixed_decision_variables()
dependent = instance.dependent_decision_variable_ids()
irrelevant = instance.irrelevant_decision_variable_ids()

df = instance.decision_variables_df()
print(df["state_role"])
```

For adapter code that only needs solver input variables, use `instance.used_decision_variables`. Use the role helpers above when migrating code that previously inspected `decision_variable_analysis()` for fixed, dependent, or irrelevant variables.

## 12. `to_bytes` / `from_bytes` removed from non-top-level types (`3.0.0a3`, [#845](https://github.com/Jij-Inc/ommx/pull/845))

Element-level bytes serialisation is removed from these types:

- `Function`, `Linear`, `Quadratic`, `Polynomial`
- `Parameter`
- `NamedFunction`, `EvaluatedNamedFunction`, `SampledNamedFunction`
- `DecisionVariable`, `EvaluatedDecisionVariable`, `SampledDecisionVariable`

(The `Constraint` family — `Constraint`, `EvaluatedConstraint`, `SampledConstraint`, `RemovedConstraint` — already lost `to_bytes` / `from_bytes` in 3.0.0a2 along with the `Constraint.id` field; see §2.)

These methods originally existed to ferry values across the Python ↔ Rust boundary back when the Python SDK had its own protobuf-based wrapper layer. With the v3 transition to direct PyO3 re-exports the boundary is gone, so element-level bytes round-trips no longer serve a purpose. Persist or exchange data through the **container types** instead. When you are creating a new serialized byte payload, use the v2 bytes API by default; reserve v1 for compatibility with existing v1 consumers or files:

- `Instance.to_v2_bytes()` / `Instance.from_v2_bytes(...)` (and the same on `ParametricInstance`, `Solution`, `SampleSet`) for new payloads
- `Instance.to_v1_bytes()` / `Instance.from_v1_bytes(...)` only when you must keep the legacy v1 wire format
- `State.to_v1_bytes()` / `Samples.to_v1_bytes()` / `Parameters.to_v1_bytes()` for the cross-`evaluate` DTOs

Top-level root aliases that did not name the protobuf version are also removed:
replace `Instance.to_bytes()` / `Instance.from_bytes(...)` (and the same on
`ParametricInstance`, `Solution`, and `SampleSet`) with `to_v2_bytes()` /
`from_v2_bytes(...)` unless the surrounding protocol explicitly requires v1.

```python
# Before (2.9.4 / 3.0.0a2)
blob = my_function.to_bytes()
f    = Function.from_bytes(blob)

dv_blob = decision_variable.to_bytes()
dv      = DecisionVariable.from_bytes(dv_blob)

# After (3.0.0a3) — wrap in the enclosing container and round-trip that.
# Use v2 bytes for newly serialized payloads.
instance      = Instance.from_components(
    sense=Sense.Minimize,
    objective=my_function,
    decision_variables=[decision_variable],
    constraints={},
)
instance_blob = instance.to_v2_bytes()
restored      = Instance.from_v2_bytes(instance_blob)
my_function   = restored.objective
decision_variable = restored.decision_variables[0].detach()
```

(13-artifact-api-archive-becomes-an-exchange-format)=
## 13. Artifact API: archive becomes an exchange format

v3 redraws the artifact API around a single canonical store — the SQLite Local Registry — and treats `.ommx` files purely as an exchange format. Every artifact goes through `ArtifactDraft` and lands in the registry; the archive file is produced as an explicit export afterward. The v2 split between "archive build" and "registry build" is gone, along with the v2 in-place "read archive without touching the registry" path.

For PR references see [#872](https://github.com/Jij-Inc/ommx/pull/872).

### 13.1 `ArtifactBuilder.new_archive` / `new_archive_unnamed` removed; use `ArtifactDraft.new` + `Artifact.save(path)`

`ArtifactBuilder.new_archive(path, image_name)` and `ArtifactBuilder.new_archive_unnamed(path)` are gone. The "produce a `.ommx` file" step is now a separate `Artifact.save(path)` call after `commit()`.

**Before (v2 / v3-alpha pre-#872)**:
```python
from ommx.artifact import ArtifactBuilder

builder = ArtifactBuilder.new_archive("my_instance.ommx", "ghcr.io/jij-inc/ommx/demo:v1")
builder.add_instance(instance)
artifact = builder.build()    # writes the .ommx file as a side effect
```

**After (v3, ≥ #872)**:
```python
from ommx.artifact import ArtifactDraft

draft = ArtifactDraft.new("ghcr.io/jij-inc/ommx/demo:v1")
draft.add_instance(instance)
artifact = draft.commit()                # lands in the user's SQLite Local Registry
artifact.save("my_instance.ommx")         # explicit export
```

`Artifact.save(path)` is the new method that emits a `.ommx` file. The path argument carries no naming information; the resulting archive's `org.opencontainers.image.ref.name` annotation is the artifact's registry image name. `save()` errors out with `Output file already exists: ...` if the path is occupied; delete the file first or pick a different name.

### 13.2 `ArtifactBuilder.new_archive_unnamed` → `ArtifactDraft.new_anonymous`

`new_archive_unnamed` is replaced by `new_anonymous`, which takes no path and synthesizes an OMMX-local image name of the form `<registry-id8>.ommx.local/anonymous:<local-timestamp>-<nonce>` (e.g. `99ea32f6.ommx.local/anonymous:20260512T124922-c2eb4f21f7e6`). Components:

- `<registry-id8>` — first 8 hex chars of a random UUID generated once when the SQLite Local Registry is created. Identifies which registry produced the artifact.
- `<local-timestamp>` — `YYYYMMDDTHHMMSS` in the caller's local time zone (no timezone marker; OCI tag syntax forbids `+` and using a fixed UTC marker would defeat the at-a-glance readability of the date).
- `<nonce>` — 12-hex (48-bit) random suffix, so concurrent / scripted anonymous commits (MINTO-style workflows) never collide on the same wall-clock second.

The hostname `<registry-id8>.ommx.local` uses the `.local` mDNS link-local TLD (RFC 6762), so an accidental `ommx push` of an anonymous artifact does **not** leak to a real remote registry.

**Before (v2 / v3-alpha pre-#872)**:
```python
builder = ArtifactBuilder.new_archive_unnamed("my_instance.ommx")
builder.add_instance(instance)
artifact = builder.build()
print(artifact.image_name)        # None
```

**After (v3, ≥ #872)**:
```python
draft = ArtifactDraft.new_anonymous()
draft.add_instance(instance)
artifact = draft.commit()
artifact.save("my_instance.ommx")
print(artifact.image_name)        # "99ea32f6.ommx.local/anonymous:20260512T124922-c2eb4f21f7e6"
```

Two behavioural shifts:

1. `image_name` is now a (synthesized) string, never `None`. v2 anonymous archives surfaced `None`; in v3 the SQLite Local Registry needs a key for every artifact so the draft synthesizes one. Code that branched on `image_name is None` to detect "unnamed archive" needs to switch to checking the `.ommx.local/anonymous:` substring or — better — call `ArtifactDraft.new(image_name)` with an explicit name when you care about identity.
2. Anonymous artifacts accumulate in the local registry. Preview the refs with `ommx prune-anonymous` before deciding which to delete.

**Timezone caveat**: the timestamp portion is the **draft's local time**, not UTC. If you ship an anonymous archive to someone in another timezone, the recipient reads the same digits as their own local time — the time component loses absolute meaning across machines. If absolute time matters, use `ArtifactDraft.new(image_name)` with an explicit name.

### 13.3 `Artifact.load_archive` removed; pick `import_archive` (write) or `inspect_archive` (read-only)

v2's `Artifact.load_archive(file)` opened a `.ommx` archive in place with no side effect on the local registry. v3 splits that contract into two named methods with explicit semantics, and the v2 name raises a migration error so an upgrade cannot silently write into the SQLite Local Registry:

- {func}`Artifact.import_archive(file) <ommx.artifact.Artifact.import_archive>` — imports the archive into the user's persistent SQLite Local Registry under the archive's `org.opencontainers.image.ref.name` annotation, returns a full {class}`~ommx.artifact.Artifact` handle (you can read every layer via `get_layer` / `get_blob`). The v3 replacement of `load_archive`'s "I want to use this archive" path.
- {func}`Artifact.inspect_archive(file) <ommx.artifact.Artifact.inspect_archive>` — reads only the manifest + layer descriptors without writing into the registry. Returns a new lightweight {class}`ArchiveManifest <ommx.artifact.ArchiveManifest>` with `image_name` / `manifest_digest` / `layers` / `annotations`. Use this for "what is in this archive" without committing to an import. Blob bodies are not accessible from `ArchiveManifest`; import first if you need them.

**Before (v2)**:
```python
# In-memory open, no registry side effect; layer access works.
artifact = Artifact.load_archive("my_instance.ommx")
print(artifact.image_name)
print(artifact.instance)
```

**After (v3) — full import** (matches the v2 "I will work with this archive" use):
```python
# Imports into ~/Library/Application Support/org.ommx.ommx/
# (or $OMMX_LOCAL_REGISTRY_ROOT). Subsequent Artifact.load(image_name)
# calls resolve from SQLite without re-importing.
artifact = Artifact.import_archive("my_instance.ommx")
print(artifact.image_name)
print(artifact.instance)
```

**After (v3) — read-only inspect** (no registry write):
```python
manifest = Artifact.inspect_archive("my_instance.ommx")
print(manifest.image_name)
for layer in manifest.layers:
    print(layer.media_type)
```

Calling `Artifact.load_archive(...)` in v3 raises a `RuntimeError` whose message names both replacements and explains the semantic shift.

If the archive's `index.json` descriptor lacks an `org.opencontainers.image.ref.name` annotation — the shape v2's `ArtifactBuilder.new_archive_unnamed(path)` produced — `import_archive` does **not** refuse the import. It synthesizes a fresh anonymous name (`<registry-id8>.ommx.local/anonymous:<local-timestamp>-<nonce>`, the same shape §13.2 documents) against the destination registry's `registry_id` and registers the archive under that name. v2 archives without a ref annotation therefore continue to load on upgrade. Anonymous archives produced by `ArtifactDraft.new_anonymous` already carry their synthesized name and re-import under that name unchanged.

`ommx inspect <archive>` (the CLI command) is the CLI equivalent of `Artifact.inspect_archive` — both read the manifest without touching the SQLite Local Registry.

### 13.4 `ommx push <archive>` removed; load first, then push by name

The CLI no longer accepts an archive file or OCI Image Layout directory as the argument to `push`. v3 pushes always source from the SQLite Local Registry. The migration is the explicit two-step pattern documented above:

**Before**:
```bash
ommx push my_instance.ommx
```

**After**:
```bash
ommx load my_instance.ommx
ommx push <image_name>
```

Running the old form prints a migration hint and exits non-zero.

### 13.5 Review anonymous refs before cleanup

`ommx prune-anonymous` lists candidates by default. Add `--delete` after reviewing the selection to remove refs. Reclaiming unreachable blobs is a separate `ommx gc` operation. See [Manage the Local Registry](../how_to/manage_local_registry.md) for selection, retention, and deletion options.

## 14. Convenience additions (not breaking)

### 14.1 `DecisionVariable.binary` / `integer` / `continuous` accept `lower` / `upper` kwargs

```python
# v3
x = DecisionVariable.integer(1, lower=0, upper=10)
y = DecisionVariable.continuous(2, lower=-1.0, upper=1.0)
z = DecisionVariable.integer(3)                  # unbounded
```

### 14.2 `DecisionVariable.equals_to(other)` for object equality

Because `==` creates a `Constraint`, v3 adds an explicit `equals_to` method for `bool` equality.

```python
x = DecisionVariable.integer(1, lower=0, upper=10)
y = DecisionVariable.integer(1, lower=0, upper=10)

c = x == y          # Constraint (not a bool)
x.equals_to(y)      # True
x.id == y.id        # True
```

### 14.3 `Parameter` supports the same operators as `DecisionVariable`

```python
from ommx import Parameter, DecisionVariable

p = Parameter(1, name="param1")
x = DecisionVariable.integer(2, lower=0, upper=10)

expr = x + p      # Linear
expr = x * p      # Quadratic
expr = 2 * p + 3  # Linear
```

## Migration checklist

- [ ] Import SDK domain classes from top-level `ommx`, for example `from ommx import DecisionVariable, Instance`.
- [ ] Remove all `.raw`, `from_raw(...)`, `from_protobuf(...)`, `to_protobuf(...)` usage; use `from_v2_bytes` / `to_v2_bytes` for newly serialized top-level payloads, `from_v1_bytes` / `to_v1_bytes` for legacy v1 interop, or direct properties.
- [ ] Replace `Constraint(id=N, ...)` / `.set_id(N)` / `(expr <= 0).set_id(N)` with `{N: (expr <= 0)}` in the `constraints=` dict.
- [ ] To read constraint IDs alongside their rows, iterate over `instance.constraints.items()`.
- [ ] Remove any use of `next_constraint_id()` / `set_constraint_id_counter(...)` / related counter helpers.
- [ ] Update `Instance.from_components(constraints=[...])` to pass a `dict[int, Constraint]`. Same for `indicator_constraints`, `one_hot_constraints`, `sos1_constraints`.
- [ ] Replace `constraint_hints` reads with `instance.constraints`, using typed IDs to distinguish constraint families.
- [ ] Rename `write_mps(...)` → `save_mps(...)`.
- [ ] Rename `instance.used_decision_variable_ids()` / `function.used_decision_variable_ids()` → `required_ids()`.
- [ ] Replace `Parameter.new(id=...)` with `Parameter(id, ...)`.
- [ ] Replace `pi.with_parameters(Parameters(entries={...}))` with `pi.with_parameters({...})`.
- [ ] Use Adapter `solve()` / `sample()` directly for recommended Preparation; when customizing the policy, call `Instance.prepare()` and then `solve_without_preparation()` / `sample_without_preparation()`. Add the preparation-free method to custom Adapters, and declare every Adapter-specific option with an explicit typed signature. A concrete Adapter may restrict an option to the preparation-free API when it depends on the exact prepared input and no transport through Preparation is defined.
- [ ] Update code that used `Linear.terms` / `Quadratic.terms` / `Polynomial.terms` as a property — they are methods now.
- [ ] `SampleSet.sample_ids` is a method returning `set[int]`; use `sample_set.sample_ids_list` if you need a `list`.
- [ ] Change `except RuntimeError` around `.evaluate(...)` / `.partial_evaluate(...)` calls to `except ValueError`.
- [ ] Switch `parametric_instance.parameters` DataFrame reads to `parametric_instance.parameters_df()` (now a method; `.parameters` returns `list[Parameter]`).
- [ ] Update `Function.degree()` / `num_terms()` annotations to accept `None`, and guard polynomial coefficient-property access when a function may be composed.
- [ ] Replace the old scalar metadata setters with `set_name` and `set_description`.
- [ ] Replace `ArtifactArchive` / `ArtifactDir` usage with `Artifact.import_archive(...)` for code that needs archive contents, `Artifact.inspect_archive(...)` for read-only archive inspection, or `Artifact.load(...)` for remote / registry refs.
- [ ] Remove any `Linear.from_object(...)` / `Linear.equals_to(...)` calls.
- [ ] Add parentheses to every `*_df` access — `instance.constraints_df` → `instance.constraints_df(kind=ConstraintKind.Regular)` etc. (every `*_df` accessor is a method now).
- [ ] Replace prerelease-only per-family accessors and v2 `removed_constraints_df` reads with `constraints_df(kind=..., removed=...)` on the same host.
- [ ] If you depended on the unqualified `id` index column on a wide constraint `*_df`, switch to the kind-qualified `{kind}_constraint_id` name. `decision_variables_df()` keeps `id` (only one variable ID space); long-format variable sidecars use `variable_id`.
- [ ] To edit a constraint already in a model, update its attached row through a typed ID.
- [ ] Update return-type annotations / static analysis for `instance.constraints` etc. to expect `AttachedX` (`ConstraintCollection`, `list[AttachedDecisionVariable]`, …). Call `.detach()` if you need an independent snapshot.
- [ ] Replace detached `DecisionVariable.substituted_value` reads with owner-side queries: `Instance.fixed_decision_variables()`, `instance.attached_decision_variable(id).substituted_value`, or `instance.decision_variables_df()["substituted_value"]`.
- [ ] Replace `decision_variable_analysis()` reads with `decision_variable_roles()`, `decision_variable_role(id)`, `fixed_decision_variables()`, `dependent_decision_variable_ids()`, `irrelevant_decision_variable_ids()`, or `decision_variables_df()["state_role"]`.
- [ ] Replace element-level `to_bytes()` / `from_bytes()` calls on `Function` / `Linear` / `Quadratic` / `Polynomial` / `Parameter` / the `NamedFunction` family / the `DecisionVariable` family with whole-`Instance` / `Solution` / `SampleSet` round-trips. Use `to_v2_bytes()` for newly serialized top-level payloads; keep `to_v1_bytes()` only for legacy v1 interop or the `State` / `Samples` / `Parameters` DTOs used by evaluate plumbing. See §12.
- [ ] Replace `ArtifactBuilder.new_archive(path, image_name).build()` with `ArtifactDraft.new(image_name).commit()` + `artifact.save(path)`. See §13.1.
- [ ] Replace `ArtifactBuilder.new_archive_unnamed(path).build()` with `ArtifactDraft.new_anonymous().commit()` + `artifact.save(path)`. Audit code that branched on `artifact.image_name is None` — anonymous artifacts now have a synthesized `<...>.ommx.local/anonymous:...` name. See §13.2.
- [ ] Replace `Artifact.load_archive(file)` with `Artifact.import_archive(file)` (registry-write semantics, returns a full handle) for code that wants to use the archive's contents. Use `Artifact.inspect_archive(file)` for the side-effect-free read of the manifest / layer descriptors (returns an `ArchiveManifest`). The v3 `load_archive` raises a migration error pointing at both. See §13.3.
- [ ] Update any `ommx push <archive-file>` invocation to the two-step `ommx load <file>` + `ommx push <image_name>` flow. See §13.4.
- [ ] If you automate anonymous Artifact cleanup, distinguish previewing refs from deleting them.

(typed-constraint-ids-and-live-collections)=
## Typed constraint IDs and live collections

All four root types expose `.constraints` as a live, read-only collection of all
constraint families. Replace `instance.constraints[5]` with
`instance.constraints[ConstraintID(5)]`; use `IndicatorConstraintID`,
`OneHotConstraintID`, or `Sos1ConstraintID` for the other families. The old
family-specific properties are consolidated into `.constraints` and
`.removed_constraints`. `get_constraint_by_id`, `get_removed_constraint_by_id`,
and result `constraint_ids` are replaced by indexing and key iteration.
There is no `get_constraints` method.

```python
from ommx import ConstraintKind

row = instance.constraints.of_kind(ConstraintKind.Regular)[5]  # AttachedConstraint
snapshot = row.detach()  # Explicit independent Constraint copy
removed = instance.removed_constraints.of_kind(ConstraintKind.Regular)[7]  # RemovedConstraint snapshot
```

The metadata methods `constraint_context_df`, `constraint_parameters_df`,
`constraint_provenance_df`, and `constraint_removed_reasons_df` require
keyword-only `kind`, just like `constraints_df`. They no longer default to regular
constraints. This applies to `Instance`, `ParametricInstance`, `Solution`, and `SampleSet`.

`SampleSet.summary_with_constraints` is removed. Use `samples.summary` for sample
objectives and overall feasibility, and `samples.constraints` or
`samples.constraints_df(kind=...)` for per-constraint results.

The collection property does not materialize rows. Model accesses return
`Attached*` handles; result accesses return individual snapshots. Retained
collection views observe owner mutations. `dict(owner.constraints)` copies the mapping but model rows in it are still attached. Call `row.detach()` on each active model row you want to keep as an independent snapshot. Use `constraints.of_kind(ConstraintKind.Sos1)` when
inspecting one family. `constraints_df(*, kind=...)` requires `kind` on every root.
Models select active rows by default and support `removed=True`; results include
retained removed rows. See {class}`collection semantics <ommx.ConstraintCollection>`.

For heterogeneous enumeration, use `ConstraintIDBase` and `ConstraintView`
instead of a union of the currently known families. Adding a special family
does not change these public return types. Concrete-ID indexing retains its
specific return type. Consumers that translate or solve models must reject
unsupported families explicitly; filtering them out would relax the problem.

Family-specific method arguments and `from_components` keys still accept
integers and promote them, rejecting IDs from another family. Mixed-family collection
indexing, membership, and `get` require typed IDs; views selected with `of_kind`
promote integers to the selected family. IDs remain typed in returned
values, diagnostics, provenance, and DataFrame indexes. They have no `int()` or
integer-index conversion. `constraint_violation(id)` infers the family from its
typed ID and has no `kind` argument. Variable and sample IDs remain integers.

`extract_constraints(name)` now searches every family. On `Instance` and
`ParametricInstance` it returns attached objects keyed by subscript tuples; on
`Solution` and `SampleSet` it returns nonnegative violations. **Regular constraints
now return violations instead of signed function values.** Parameters are ignored,
as in variable extraction; duplicate subscripts across families raise `ValueError`.

For signed regular values, use `solution.get_constraint_value(id)` or `row.evaluated_value`. See {meth}`name-based extraction <ommx.Solution.extract_constraints>`.

## Extensible variable and constraint kinds

`Kind`, `ConstraintKind`, and `ProvenanceKind` are immutable value classes
with known constants. Their type hints no longer describe exhaustive Enums.
Keep an explicit rejecting fallback when dispatching on these values, even
when all currently known constants have been handled; `assert_never` cannot
establish complete support. Comparisons, hashing, integer conversion and
`Kind.from_pb()` / `to_pb()` remain available. Unknown protobuf kinds are rejected.

Common constraint collections continue to expose `ConstraintIDBase` and
`ConstraintView`, with precise return types for concrete family IDs. New kinds
and families do not imply support by an existing adapter or transformation.
The V3 extension contract covers numeric scalar domains. Non-scalar values
require a separate design for value storage and evaluation.

Use `ConstraintKind` for all constraint families, including `ConstraintKind.Regular`.
It replaces the former `SpecialConstraintKind` selector and string selectors:
`collection.of_kind(ConstraintKind.Sos1)` and
`model.constraints_df(kind=ConstraintKind.Sos1)`. The ID type carried by a known
constant preserves precise family-view return types; callers normally do not
need to write the type parameter. `active_constraint_kinds` includes Regular,
while `active_special_constraint_kinds` excludes it. Requesting Regular in
`lower_special_constraints` is rejected before any conversion is applied.
