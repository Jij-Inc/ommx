---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: Python 3 (ipykernel)
  language: python
  name: python3
---

# Variable roles, reconstruction, and defaulting

You do not necessarily need to supply values for every variable retained by an Instance each time. **Supply values for used variables, reconstruct fixed and dependent variables, and fill omitted irrelevant variables with defaults.** This distinction matters when reading transformed candidates in terms of original variables or seeing unused variables in results.

This page assumes that you know an [Instance](./instance.md) combines variables, an objective, and constraints, and that `evaluate()` evaluates a candidate. [What transformations preserve](./transformation_guarantees.md) separately covers conditions for preserving the original optimization problem.

(retained-and-used-variables)=
## Retained and used variables

Fixing a variable or expressing it through other variables can make it unnecessary for the solver to choose its value. OMMX retains its definition so results remain readable in terms of original variables.

```{code-cell} ipython3
from ommx import Instance

instance = Instance.maximize()
x = instance.new_binary("x")
y = instance.new_binary("y")
instance.objective = x + y
instance.add_constraint(x * y == 0)

# partial_evaluateは元のInstanceを変更せず、固定した問題を返す。
fixed = instance.partial_evaluate({x.id: 1})

assert {v.id for v in fixed.decision_variables} == {x.id, y.id}
assert {v.id for v in fixed.used_decision_variables} == {y.id}

fixed_solution = fixed.evaluate({y.id: 0})
assert fixed_solution.state.entries == {x.id: 1.0, y.id: 0.0}
assert fixed_solution.objective == 1
```

The definition of `x` is retained, but only `y` needs an assigned value in the fixed problem. The Instance uses its recorded value 1 for `x` without requiring you to supply it again.

{attr}`~ommx.Instance.decision_variables` returns all retained variables. {attr}`~ommx.Instance.used_decision_variables` returns variables appearing in the current objective and active constraints. Active means currently imposed constraints, including ordinary, Indicator, OneHot, and SOS1 constraints. An Indicator's Binary control variable is also used.

The development SDK additionally provides {attr}`~ommx.Instance.used_decision_variable_ids` for the IDs of the same set. It is not available in public SDK 3.0.0b7 used by this Book's execution environment, so the example extracts IDs from `used_decision_variables`.

(instance-variable-roles)=
## Four roles

Each variable in an Instance has one of these roles. This classification is separate from kinds such as Binary and Integer.

| Role | Meaning | Value needed for evaluation |
|---|---|---|
| used | Appears in the current objective or active constraints | Supplied by the caller |
| fixed | Has a recorded fixed value | Use the recorded value |
| dependent | Expressed as a function of other variables | Compute from the recorded expression |
| irrelevant | None of the above; its value need not be selected for the current problem | Fill with a default if omitted |

Inspect one variable with {meth}`~ommx.Instance.decision_variable_role` or all variables with {meth}`~ommx.Instance.decision_variable_roles`. Roles are relative to an Instance and can change through transformations.

## Reconstruct fixed and dependent values from the Instance

Fixed variables use recorded fixed values. Dependent variables use recorded dependency expressions. For example, replace an Integer variable `count` with twice a Binary variable `flag`.

```{code-cell} ipython3
encoded = Instance.minimize()
count = encoded.new_integer("count", lower=0, upper=2)
flag = encoded.new_binary("flag")
encoded.objective = count + flag

encoded.substitute({count.id: 2 * flag})
assert {v.id for v in encoded.used_decision_variables} == {flag.id}

restored = encoded.evaluate({flag.id: 1})
assert restored.state.entries[count.id] == 2.0
assert restored.objective == 3.0
```

The values obtained from `count=2*flag` are within `count`'s domain. However, `count=1` cannot be represented, so this substitution is not equivalent to the whole original integer problem. Nor does {meth}`~ommx.Instance.substitute` automatically turn the replaced variable's domain into new constraints. **Being able to calculate a value back does not establish equivalence before and after substitution.** See [Transformation guarantees](./transformation_guarantees.md) when choosing a custom substitution.

If you also supply fixed or dependent values, they are checked for consistency with the recorded value or dependency; they do not override reconstruction rules. See {meth}`~ommx.Instance.populate_state` for rounding, tolerances, and inconsistent-input handling.

## Defaults for irrelevant variables do not reconstruct the original candidate

You can declare variables unused by the objective and constraints.

```{code-cell} ipython3
reporting = Instance.minimize()
chosen = reporting.new_binary("chosen")
unused = reporting.new_integer("unused", lower=2, upper=5)
reporting.objective = chosen

completed = reporting.evaluate({chosen.id: 0})
assert completed.state.entries[unused.id] == 2.0

# 手元の候補に値があるなら、省略せずに渡す。
explicit = reporting.evaluate({chosen.id: 0, unused.id: 4})
assert explicit.state.entries[unused.id] == 4.0
```

`unused` does not affect the current objective or constraints, but its value is not uniquely determined. The default 2 is neither selected by a solver nor reconstructed from the original candidate.

For Binary, Integer, and Continuous variables, the default is the value closest to zero within the bounds. For SemiInteger and SemiContinuous, it is zero. These semi-domains allow zero in addition to the specified range. See {ref}`Variable domains <decision-variable-domains>` for kinds and bounds.

## What is evaluated after reconstruction and defaulting?

{meth}`~ommx.Instance.populate_state` reconstructs and fills values; {meth}`~ommx.Instance.evaluate` then evaluates objectives and constraints. Missing used-variable values are not replaced with defaults. Successful reconstruction or defaulting alone does not establish satisfaction of all constraints.

Appearing only in a named function, removed constraint (retained for checking, not currently imposed), or output objective (retained for reporting separately from the current objective) does not itself make a variable used. If an irrelevant variable appears there and is omitted, those expressions are evaluated at its default. Supply the value to inspect a particular candidate.

The usual result-reading entry is [State-to-Solution evaluation](./solution.md). For retained constraints and objectives, see [Transformed problems and removed constraints](./removed_constraints.md).
