# Table Operation Algebra

This note follows `instance_operation_algebra.md`. The root operation owner is
still `Instance`, `ParametricInstance`, `Solution`, or `SampleSet`. Tables are
only storage components that realize row-local effects requested by the root
object.

The goal is to derive the operations tables need from the mathematical actions
on the root object, rather than from the current call sites.

## General Rule

For a root object operation:

```text
O: R -> R'
```

a table operation should represent only the table-local effect of `O`:

```text
effect_T(O): T -> T'
```

The table may enforce invariants it can know from its own rows and sidecars.
The table must not own semantic facts that require another component of the
root object.

Therefore table operations should be limited to:

- constructing a table while validating row/sidecar key consistency;
- reading rows, keys, and sidecars;
- inserting a row whose host-level validity has already been checked;
- replacing a row while preserving table identity and sidecars;
- applying a by-value rewrite plan atomically;
- moving rows between lifecycle components owned by the table;
- setting sidecars for rows owned by the table;
- consuming the table at serialization/conversion boundaries.

They should not expose raw `&mut` access to row payloads across owner
boundaries. A raw mutable reference lets the caller perform part of a root
operation without the root object that owns the relevant semantics.

## DecisionVariableTable

Mathematical object:

```text
X = { variable_id -> domain row }
```

At the created stage, `DecisionVariableTable` also owns fixed-value sparse
columns and modeling labels. Evaluated and sampled stages own evaluated or
sampled rows plus labels.

### Table-Level Invariants

- Every label ID is a table-owned variable ID.
- Every created-stage fixed value is keyed by a table-owned variable ID.
- A fixed value is finite and satisfies the row's kind/bound under the supplied
  tolerance.
- The table key is the source of truth for `VariableID`.

### Host-Level Invariants

The enclosing root object owns:

- whether variable IDs are disjoint from parameter IDs;
- whether an ID is used, fixed, or dependent;
- whether expressions and constraints reference only allowed IDs;
- whether removing or replacing a variable preserves the problem semantics.

### Required Operations

The table should support:

- construct from rows, labels, and stage columns;
- read keys, rows, labels, and stage columns;
- insert a fresh created row with label and optional fixed value;
- set or ensure a fixed value for an existing row;
- set a label for an existing row;
- intersect a row domain with an additional bound, preserving fixed-value
  consistency;
- atomically apply a host-computed batch of row replacements.

The table should not expose a general replace operation as a casual map update.
Replacing a decision-variable row can change the meaning of objective and
constraint predicates. If replacement is needed, it should be represented as a
host-validated operation, not as arbitrary table mutation.

The table should not own deletion. Removing a variable requires a root-level
operation that proves every reference has been eliminated or rewritten.

## ParameterTable

Mathematical object:

```text
P = { parameter_id }
```

OMMX uses `VariableID` for parameter IDs because algebraic expressions do not
carry a separate parameter namespace. `ParametricInstance` owns the relation
between `X` and `P`.

### Table-Level Invariants

- Every label ID is a table-owned parameter ID.
- The table key set is the source of truth for parameter IDs.

### Host-Level Invariants

The enclosing `ParametricInstance` owns:

- disjointness between decision-variable IDs and parameter IDs;
- whether expression IDs are in `X union P`;
- whether structural variable positions such as indicator variables, one-hot
  members, and SOS1 members use decision-variable IDs rather than parameters;
- substitution of parameter values into a concrete `Instance`.

### Required Operations

The table should support:

- construct from IDs and labels;
- read keys and labels;
- insert a fresh parameter ID with label;
- set a label for an existing parameter ID;
- consume into IDs and labels at conversion boundaries.

It should not store parameter values. Values are part of the
`ParametricInstance -> Instance` specialization operation.

## NamedFunctionTable

Mathematical object:

```text
N = { named_function_id -> expression row }
```

The payload may be a created, evaluated, or sampled named function depending on
the root object stage.

### Table-Level Invariants

- Every label ID is a table-owned named-function ID.
- The table key is the source of truth for `NamedFunctionID`.

### Host-Level Invariants

The enclosing root object owns:

- whether expression rows reference known variables or parameters;
- whether evaluated or sampled rows match the root object's state/sample IDs;
- whether named functions participate in the used/fixed/dependent variable
  partition.

### Required Operations

The table should support:

- construct from rows and labels;
- read keys, rows, and labels;
- insert a fresh row with label;
- replace an existing row after host validation;
- set a label for an existing row;
- atomically rewrite rows by value while preserving row IDs and labels;
- consume into rows and labels at serialization/conversion boundaries.

The table should not expose raw mutable access to expression rows. Expression
substitution, partial evaluation, and sampling/evaluation semantics are root
operations that merely induce named-function row replacements.

## ConstraintCollection

Mathematical object for one constraint family:

```text
C_tau = Active_tau + Removed_tau + Context_tau
```

This is family-local lifecycle storage. It is a component of `Instance`, not an
optimization problem by itself.

### Table-Level Invariants

- Active and removed IDs are disjoint within one family.
- Every context ID is either active or removed in that family.
- Row identity is preserved when a row moves between active and removed states.
- Context remains attached to the row ID unless the root operation explicitly
  pushes it forward to generated rows.

### Host-Level Invariants

The enclosing `Instance` or `ParametricInstance` owns:

- whether constraint payloads reference known and allowed IDs;
- whether indicator, one-hot, and SOS1 structural variable requirements hold;
- whether a lifecycle action is semantically valid;
- whether restore requires substitution or partial evaluation under current
  instance state;
- family morphisms that generate rows in another constraint family;
- provenance pushforward from source rows to generated rows.

### Required Operations

The collection should support:

- construct from active rows, removed rows, and context;
- read active rows, removed rows, and context;
- allocate or report an unused ID within the family;
- insert a fresh active row with context after host validation;
- replace an active row by ID after host validation;
- replace a removed row by ID while preserving its removal reason;
- replace a row while preserving its current lifecycle component;
- atomically rewrite active rows by value into either active rows or removed
  rows;
- move an active row to removed with a host-supplied reason;
- restore a removed row through a host-supplied normalizer;
- set context for an owned row;
- consume into active rows, removed rows, and context.

The important restore operation is:

```text
restore_removed_with(id, normalize):
    Removed(id, p, reason) -> Active(id, normalize(p, reason, context))
```

This keeps the lifecycle action atomic from the collection's point of view
while leaving semantic normalization to the root object. The current pattern of
restoring first and then taking `&mut` to normalize the active row splits one
root operation across two authorities.

### Operations It Should Not Expose

The collection should not expose:

- mutable references to active payloads;
- mutable iteration over active payloads;
- arbitrary active-map or removed-map mutation;
- semantic operations such as substitute, partial-evaluate, propagate, slack
  conversion, or capability reduction.

Those are all `Instance` operations. The collection should only apply their
precomputed row effects.

## EvaluatedCollection and SampledCollection

Mathematical object:

```text
E_tau = { id -> evaluated predicate result } + RemovedReasons + Context
S_tau = { id -> sampled predicate result } + RemovedReasons + Context
```

These are result tables used by `Solution` and `SampleSet`.

### Table-Level Invariants

- Removed-reason IDs refer to existing evaluated or sampled rows.
- Context IDs refer to existing evaluated or sampled rows.
- Sampled rows have internally consistent sample side maps.

### Host-Level Invariants

The enclosing `Solution` or `SampleSet` owns:

- consistency with the decision-variable table;
- consistency with named-function rows;
- global sample-ID consistency;
- feasibility interpretation across constraint families.

### Required Operations

These tables should mostly be immutable after construction. They should support:

- construct from rows, removed reasons, and context;
- read rows, removed reasons, and context;
- feasibility and removed-state queries;
- sample-ID validation for sampled rows;
- used-variable validation against a host-supplied variable ID set;
- consume into rows, removed reasons, and context.

If a post-construction update is needed, prefer a by-value replacement method
with explicit owner context over raw mutable row access.

## Sidecar Stores

`ModelingLabelStore` and `ConstraintContextStore` are sparse column stores, not
root domain owners.

They may support low-level column operations such as:

- insert or replace one sidecar value;
- collect sidecar fields for one row;
- merge or push forward sidecars according to a host-supplied row mapping;
- prune entries according to an owner-supplied ID set.

They cannot decide whether an ID is valid by themselves. Table owners validate
sidecar IDs against their row keys, and root owners decide semantic mappings
between tables.

## Current Instance Operations to Table Effects

| Root operation | Table-local effects |
| --- | --- |
| `set_objective` | no table mutation; host validates expression references |
| `add_decision_variable` | `DecisionVariableTable` fresh insert |
| `set_fixed_value` | `DecisionVariableTable` fixed-value column update after host role check |
| `insert_constraint` | `ConstraintCollection` replace preserving lifecycle after host validation |
| `add_constraint` | `ConstraintCollection` fresh active insert with context |
| substitution | host computes expression rewrite; tables receive row replacements |
| partial evaluation | host computes quotient; collections receive active rewrites and removals |
| reduce binary power | host computes expression action; tables receive row replacements |
| slack conversion | decision-variable fresh insert plus regular-constraint row replacement |
| relax | constraint-family lifecycle move active to removed |
| restore | constraint-family restore through host normalization |
| one-hot / indicator / SOS1 conversion | source-family lifecycle move plus target-family generated insert and context pushforward |
| unit propagation | active row rewrite, lifecycle moves, generated regular rows, and assignment-state growth |

## Design Consequence

The target shape is not "make every current `&mut` operation safe." The target
shape is:

1. define the root operation on `Instance`, `ParametricInstance`, `Solution`,
   or `SampleSet`;
2. compute the data the root operation must read and change;
3. reduce that operation to table-local row effects;
4. expose only the table primitive needed for those row effects.

If a table primitive still lets a caller mutate payloads without naming the row
effect being performed, it is probably too broad.
