# Instance Operation Algebra

This note is a design analysis for issue #970. It intentionally starts from the
mathematical operations on `Instance`, not from the current
`ConstraintCollection` API shape. The point is to name the actions first, then
decide which implementation object may carry them.

## Base Object

An OMMX `Instance` is a row-indexed optimization problem over a variable space:

```text
I = (sense, X, f0, C, A, N, L, P)
```

where:

- `X` is the decision-variable table, including domain data such as kind and
  bounds;
- `f0 in R[X]` is the objective function;
- `C = (C_reg, C_ind, C_onehot, C_sos1)` is the family-indexed constraint
  state;
- `A` is root-owned assignment state, including fixed values and substitution
  assignments;
- `N` is the named-function table;
- `L` is modeling-label and context sidecar state;
- `P` is transformation provenance.

Each constraint family is represented by row-indexed lifecycle storage:

```text
C_tau = Active_tau + Removed_tau + Context_tau
```

This storage is a component of `Instance`, not the mathematical problem by
itself. It can own row identity, active/removed disjointness, and sidecar key
alignment for one family. It cannot own operations whose meaning depends on
`X`, `f0`, `A`, `N`, another constraint family, or global provenance.

The regular-constraint payload is an expression predicate:

```text
f in R[X]
Constraint = (f, equality)
```

Special constraints are predicates over the same variable space:

```text
OneHot(S)       = exactly one variable in S is 1
SOS1(S)         = at most one variable in S is non-zero
Indicator(y, p) = if y = 1 then predicate p holds
```

## Operation Classes on Instance

### 1. Expression Algebra Actions

A map on the expression algebra induces an action on the objective and on any
constraint payload that contains expressions.

```text
phi: R[X] -> R[X']
Phi_phi: I(X) -> I'(X')
```

Examples:

- substituting dependent variables;
- evaluating fixed variables;
- reducing binary powers;
- rewriting a regular constraint body during slack introduction.

This is not a raw mutation of one constraint row. The expression map may need
to preserve or change the variable space, update assignment state, and decide
whether a constraint remains meaningful. Therefore the operation is naturally
an `Instance` action. A family storage component may only receive the resulting
row action.

### 2. Assignment-Induced Quotients

A partial assignment `sigma` induces an evaluation or quotient map:

```text
ev_sigma: R[X] -> R[X \ dom(sigma)]
```

The induced operation on `Instance` can affect the objective, active
constraints, removed constraints, named functions, and root-owned assignment
state.

```text
(I, sigma) -> I_sigma
```

Depending on the evaluated predicate, one row may become:

```text
active(c')
removed(c, reason)
infeasible certificate
derived assignment
```

This operation is an `Instance` normalization. The constraint-family storage
does not know which variables are fixed, which removed constraints may still
refer to fixed variables, or how derived assignments affect other rows.

### 3. Variable-Space Extensions

Some transformations extend the variable space:

```text
i: X -> X + S
Ext_i: I(X) -> I'(X + S)
```

Slack introduction is the main example:

```text
f(x) <= 0 -> f(x) + b s <= 0
f(x) <= 0 -> a f(x) + s = 0
```

This operation adds a decision variable, adds variable labels, and rewrites a
constraint predicate. It may be semantics-preserving only together with the
projection back to the original variable space:

```text
pi_X(Sol(I')) = Sol(I)
```

That statement is about the whole `Instance`, not about one collection row.

### 4. Constraint-Family Morphisms

Capability reduction maps predicates in one family into predicates in another
family.

```text
rho_tau: Predicate_tau(X) -> list<Predicate_reg(X)>
```

Examples:

```text
OneHot(S)       -> Constraint(sum S - 1 = 0)
Indicator(y, p) -> list<Constraint>
SOS1(S)         -> list<Constraint>
```

The induced action is not an endomorphism of one family storage component:

```text
I(C_tau, C_reg) -> I'(Removed_tau, C_reg + generated rows)
```

It consumes or removes rows in the source family, inserts rows in the regular
family, and pushes context/provenance through the morphism. The source and
target row actions are implementation details of one `Instance` operation.

### 5. Lifecycle Actions

Relaxing and restoring constraints are lifecycle actions on the feasible set of
an `Instance`.

```text
relax:   Active_tau(id, p) -> Removed_tau(id, p, reason)
restore: Removed_tau(id, p, reason) -> Active_tau(id, p')
```

`relax` generally weakens the feasible set:

```text
Sol(relax(I)) superset Sol(I)
```

`restore` is not always a strict inverse. If the `Instance` accumulated fixed
assignments, substitutions, or variable-space changes while the row was
removed, the restored predicate may be a normalized predicate `p'`.

The row move itself is family-local storage work. The decision that this
lifecycle action is valid, and any restore-time normalization, belongs to the
`Instance`.

### 6. Propagation as a Rewrite System

Unit propagation is a rewrite system over an instance plus assignment state:

```text
(I, sigma) -> (I', sigma')
```

It may derive new assignments, rewrite predicates, consume rows, or convert an
indicator predicate into a regular predicate. Algebraically this combines:

- an assignment-induced quotient;
- lifecycle actions;
- constraint-family morphisms;
- monotone growth of derived assignment state.

This is a global `Instance` action because the result of one row can affect the
normal form of other rows.

### 7. Sidecar and Provenance Pushforward

Modeling context is not part of the mathematical predicate, but operations on
an `Instance` induce maps on sidecar rows:

```text
Context(source row) -> Context(target row)
Provenance(source row) -> Provenance(target row)
```

For a family morphism, this is a pushforward along the generated row mapping.
For lifecycle actions, it preserves row identity across active and removed
states. The family storage owns sidecar consistency, but only the `Instance`
operation knows the source-to-target map.

## Current Operations Interpreted as Instance Actions

| Current operation | Algebraic action on `Instance` |
| --- | --- |
| substitution | expression algebra action plus assignment-state normalization |
| partial evaluation | assignment-induced quotient |
| reduce binary power | expression algebra action over objective and predicates |
| slack conversion | variable-space extension plus predicate rewrite |
| one-hot / indicator / SOS1 conversion | constraint-family morphism plus sidecar pushforward |
| relax | lifecycle action that weakens the feasible set |
| restore | lifecycle action plus normalization under current instance state |
| unit propagation | rewrite system over instance and assignment state |
| context setters | sidecar action constrained by existing instance rows |
| insert constraint(s) | extension of the row-indexed predicate state after host validation |

## Ownership Consequence

The relevant owner for these operations is `Instance`. `ConstraintCollection`
is only a component that realizes family-local row effects:

- insert a fresh row or apply a lifecycle-preserving replacement already
  validated by the `Instance`;
- move a row between active and removed lifecycle components;
- keep active/removed IDs disjoint inside one family;
- keep context keys aligned with rows in that family.

Leaking `&mut T::Created` from the family storage is therefore not just an
encapsulation problem. It allows crate-local code to perform part of an
`Instance` action while bypassing the object that owns the variable space,
assignment state, cross-family effects, and sidecar pushforward.

The design question should be asked in this order:

1. Which algebraic action on `Instance` is this operation?
2. Which parts of the `Instance` tuple does the action read or modify?
3. Which family-local row effects are needed to realize the action?
4. Which storage primitive can perform exactly those row effects without
   granting semantic mutation authority to the caller?
