---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: ommx-update-books (3.9.23)
  language: python
  name: python3
---

# ommx.v1.Instance

[`ommx.v1.Instance`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance) is a data structure for describing the optimization problem itself (mathematical model). It consists of the following components:

- Decision variables ([`decision_variables`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.decision_variables))
- Objective function ([`objective`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.objective))
- Constraints ([`constraints`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.constraints))
- Maximization/Minimization ([`sense`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.sense))

For example, let's consider a simple optimization problem:

$$
\begin{aligned}
\max \quad & x + y \\
\text{subject to} \quad & x y  = 0 \\
& x, y \in \{0, 1\}
\end{aligned}
$$

The corresponding `ommx.v1.Instance` is as follows.

```{code-cell} ipython3
from ommx.v1 import Instance, DecisionVariable

x = DecisionVariable.binary(1, name='x')
y = DecisionVariable.binary(2, name='y')

instance = Instance.from_components(
    decision_variables=[x, y],
    objective=x + y,
    constraints={0: x * y == 0},
    sense=Instance.MAXIMIZE
)
```

Each of these components has a corresponding property. The objective function is converted into the form of [`ommx.v1.Function`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Function), as explained in the previous section.

```{code-cell} ipython3
instance.objective
```

`sense` is set to `Instance.MAXIMIZE` for maximization problems or `Instance.MINIMIZE` for minimization problems.

```{code-cell} ipython3
instance.sense == Instance.MAXIMIZE
```

## Decision Variables

Decision variables and constraints can be obtained in the form of [`pandas.DataFrame`](https://pandas.pydata.org/pandas-docs/stable/reference/frame.html).

```{code-cell} ipython3
instance.decision_variables_df()
```

First, `kind`, `lower`, and `upper` are essential information for the mathematical model.

- `kind` specifies the type of decision variable, which can be Binary, Integer, Continuous, SemiInteger, or SemiContinuous.
- `lower` and `upper` are the lower and upper bounds of the decision variable. For Binary variables, this range is $[0, 1]$.

Additionally, OMMX is designed to handle metadata that may be needed when integrating mathematical optimization into practical data analysis. While this metadata does not affect the mathematical model itself, it is useful for data analysis and visualization.

- `name` is a human-readable name for the decision variable. In OMMX, decision variables are always identified by ID, so this `name` may be duplicated. It is intended to be used in combination with `subscripts`, which is described later.
- `description` is a more detailed explanation of the decision variable.
- When dealing with many mathematical optimization problems, decision variables are often handled as multidimensional arrays. For example, it is common to consider constraints with subscripts like $x_i + y_i \leq 1, \forall i \in [1, N]$. In this case, `x` and `y` are the names of the decision variables, so they are stored in `name`, and the part corresponding to $i$ is stored in `subscripts`. `subscripts` is a list of integers, but if the subscript cannot be represented as an integer, there is a `parameters` property that allows storage in the form of `dict[str, str]`.

If you need a list of [`ommx.v1.DecisionVariable`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.DecisionVariable) directly, you can use the [`decision_variables`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.decision_variables) property.

```{code-cell} ipython3
for v in instance.decision_variables:
    print(f"{v.id=}, {v.name=}")
```

To obtain `ommx.v1.DecisionVariable` from the ID of the decision variable, you can use the [`get_decision_variable_by_id`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.get_decision_variable_by_id) method.

```{code-cell} ipython3
x1 = instance.get_decision_variable_by_id(1)
print(f"{x1.id=}, {x1.name=}")
```

## Constraints
Next, let's look at the constraints.

```{code-cell} ipython3
instance.constraints_df()
```

In OMMX, constraints are also managed by ID, and this ID is independent of the decision variable ID. The ID is assigned when a constraint is attached to an `Instance`: the key you use in the `constraints` dictionary passed to [`Instance.from_components`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.from_components) becomes the constraint ID.

The essential information for constraints is `equality`. `equality` indicates whether the constraint is an equality constraint ([`Constraint.EQUAL_TO_ZERO`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Constraint.EQUAL_TO_ZERO)) or an inequality constraint ([`Constraint.LESS_THAN_OR_EQUAL_TO_ZERO`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Constraint.LESS_THAN_OR_EQUAL_TO_ZERO)). Note that constraints of the type $f(x) \geq 0$ are treated as $-f(x) \leq 0$.

Constraints can also store metadata similar to decision variables. You can use `name`, `description`, `subscripts`, and `parameters`. Use `set_name`, `set_description`, `set_subscripts`, and `set_parameters` to replace those metadata fields. Use `add_subscripts`, `add_parameter`, and `add_parameters` when you want to append or merge entries instead.

```{code-cell} ipython3
c = (x * y == 0).set_name("prod-zero")
print(f"{c.name=}")
```

You can also use the [`constraints`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.constraints) property to directly obtain a `dict[int, ommx.v1.Constraint]` keyed by constraint ID. To obtain an `ommx.v1.Constraint` by its ID, use the [`get_constraint_by_id`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.get_constraint_by_id) method.

```{code-cell} ipython3
for cid, c in instance.constraints.items():
    print(f"id={cid}: {c}")
```

## Symbolic substitution

`Instance.substitute` replaces decision variables with function expressions in the objective and active constraints. This is useful for transformations such as binary encodings, where an integer variable is removed and represented by newly introduced binary variables.

This operation is an algebraic rewrite. It does not automatically translate the substituted variable's `kind`, `lower`, or `upper` into constraints on the replacement expression. For example, if `x1` is binary and you substitute `x1` with `x2 + x3`, OMMX does not add the constraints `0 <= x2 + x3` and `x2 + x3 <= 1`. If `x1` is integer, OMMX also does not add a constraint that the replacement expression must be integral.

The substituted variable is recorded as a dependent variable, so its value can be reconstructed when evaluating a solution. Its bound and kind are checked by `Solution.feasible()`, but they are not passed to solvers as constraints on the replacement expression. In other words, `substitute` does not by itself guarantee an equivalent optimization model.

This is intentional. Some transformations, such as relaxing a constraint, deliberately change the model. Other transformations, such as log encoding or a custom binary encoding, are valid because the encoding itself is constructed to preserve the original variable's domain.

If a general substitution must preserve the model, add the necessary constraints explicitly. A common conservative pattern is to keep the original variable and add a linking equality instead of eliminating it:

```python
instance.add_constraint(x1 - (x2 + x3) == 0)
```

If you do eliminate `x1` with `substitute`, add any required bound constraints on the replacement expression yourself:

```python
expr = x2 + x3
instance.substitute({1: expr})
instance.add_constraint(expr >= 0)
instance.add_constraint(expr <= 1)
```
