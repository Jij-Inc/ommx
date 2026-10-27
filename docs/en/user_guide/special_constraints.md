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

# Special-constraint semantics and evaluation

OMMX supports Indicator, OneHot, and SOS1 constraints in addition to ordinary equalities and inequalities. **Their satisfaction conditions are defined independently of a solver and can be evaluated without calling one.** A special constraint is a condition of the problem, not an instruction to a particular solver.

| Constraint | Meaning |
|---|---|
| {class}`~ommx.IndicatorConstraint` | Enforce an equality or inequality only when a Binary variable is 1 |
| {class}`~ommx.OneHotConstraint` | Exactly one of the Binary variables is 1 |
| {class}`~ommx.Sos1Constraint` | At most one of the variables is nonzero |

This page uses [Instance and State evaluation](./solution.md) to examine these meanings. For solving a problem, see the [Adapter Tutorial](../tutorial/solve_with_ommx_adapter).

## Indicator: a conditional constraint

For a Binary variable $z$, require $f(x)\leq0$ or $f(x)=0$ only when $z=1$. When $z=0$, the Indicator constraint is satisfied regardless of the expression's value. This does not disable variable domains or other constraints.

The following example means “limit the amount to 5 only when the facility is enabled.”

```{code-cell} ipython3
from ommx import Instance

indicator = Instance.maximize()
enabled = indicator.new_binary("enabled")
amount = indicator.new_continuous("amount", lower=0, upper=10)
indicator.objective = amount
indicator.add_indicator_constraint((amount <= 5).with_indicator(enabled))

assert indicator.evaluate({enabled.id: 0, amount.id: 6}).feasible
assert indicator.evaluate({enabled.id: 1, amount.id: 5}).feasible
assert not indicator.evaluate({enabled.id: 1, amount.id: 6}).feasible
```

{meth}`~ommx.Constraint.with_indicator` specifies the control variable for an ordinary constraint. This is a one-way implication: $z=1$ implies that the constraint holds. Satisfaction of the constraint does not imply $z=1$.

## OneHot: choose exactly one

For Binary variables $x_0,\ldots,x_{n-1}$, require $\sum_i x_i=1$. Selecting none or selecting more than one is infeasible.

```{code-cell} ipython3
from ommx import OneHotConstraint

one_hot = Instance.maximize()
choices = [one_hot.new_binary("choice", subscripts=[i]) for i in range(3)]
one_hot.objective = sum(v * x for v, x in zip([5, 10, 3], choices))
one_hot.add_one_hot_constraint(OneHotConstraint(variables=choices))

chosen = one_hot.evaluate({choices[0].id: 0, choices[1].id: 1, choices[2].id: 0})
assert chosen.feasible
assert chosen.objective == 10
assert not one_hot.evaluate({x.id: 0 for x in choices}).feasible
```

Mathematically, this is the same condition as an ordinary equality. Keeping it as OneHot retains the “choose exactly one” structure for transformations and for checking their results. Every referenced variable must be Binary.

(sos1-constraint)=
## SOS1: at most one nonzero variable

SOS1 requires at most one of its variables to be nonzero. The variables need not be Binary, and assigning zero to all of them satisfies the constraint. It does not require the nonzero value to be 1.

```{code-cell} ipython3
from ommx import Sos1Constraint

sos1 = Instance.maximize()
amounts = [
    sos1.new_continuous("amount", lower=0, upper=10, subscripts=[i])
    for i in range(3)
]
sos1.objective = sum(amounts)
sos1.add_sos1_constraint(Sos1Constraint(variables=amounts))

assert sos1.evaluate({x.id: 0 for x in amounts}).feasible
assert sos1.evaluate({amounts[0].id: 7, amounts[1].id: 0, amounts[2].id: 0}).feasible
assert not sos1.evaluate({amounts[0].id: 7, amounts[1].id: 1, amounts[2].id: 0}).feasible
```

Unlike OneHot, SOS1 can express both “select none” and “allocate a positive amount to only one.” Variables may also allow negative values: the test concerns whether a value is nonzero, regardless of its sign. Numerical evaluation uses a tolerance, as with other constraints.

(sos1-big-m-formulation)=
## Lowering to ordinary constraints and promotion to special constraints

Even when a solver does not support a special constraint directly, ordinary constraints may express the same OMMX-defined condition. This conversion is called lowering. For example, OneHot can be lowered to an equality, and Indicator or SOS1 constraints with finite bounds can be lowered to ordinary Big-M constraints.

Separate solver support from constraint meaning. Whether passed directly or lowered, the starting point is the condition defined above. Required bounds and numerical conditions depend on the operation; choosing an arbitrarily large coefficient is not sufficient.

The reverse operation, promotion, checks that a specified set of ordinary constraints expresses SOS1 and represents it as a special constraint. See the API Reference for formulas, generated objects, and exact acceptance conditions.

- Indicator lowering: {meth}`~ommx.Instance.convert_indicator_to_constraint`
- OneHot lowering: {meth}`~ommx.Instance.convert_one_hot_to_constraint`
- SOS1 Big-M lowering: {meth}`~ommx.Instance.convert_sos1_to_constraints`
- Promotion from ordinary constraints to SOS1: {meth}`~ommx.Instance.promote_sos1_big_m`

Lowering retains the original special constraint as removed, and generated ordinary constraints can refer to their source. [Transformed problems and removed constraints](./removed_constraints.md) explains how to check results against those retained conditions.

## Constraint IDs and evaluation results

Ordinary, Indicator, OneHot, and SOS1 constraints each have their own constraint-ID space. For example, ordinary constraint ID 0 and OneHot constraint ID 0 identify different constraints. Variables referenced by any of them use decision-variable IDs within the same Instance.

Select a constraint family in a Solution or SampleSet DataFrame with `kind=`.

| Constraint family | `kind=` |
|---|---|
| Ordinary | `"regular"` (default) |
| Indicator | `"indicator"` |
| OneHot | `"one_hot"` |
| SOS1 | `"sos1"` |

```{code-cell} ipython3
chosen.constraints_df(kind="one_hot")
```

For Indicator constraints, `indicator_active` distinguishes a zero control variable from a control value of 1 for which the body was checked. The expression value of an ordinary constraint and the violation measures of special constraints have different meanings. See {meth}`~ommx.Solution.constraints_df` for each DataFrame's column definitions.
