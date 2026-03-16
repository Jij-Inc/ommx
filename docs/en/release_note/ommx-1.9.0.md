---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: .venv
  language: python
  name: python3
---

```{warning}
This document was written for the OMMX Python SDK 1.9.0 release and is not compatible with Python SDK 2.0.0 or later.
```

+++

# OMMX Python SDK 1.9.0

+++

This release significantly enhances the conversion functionality from `ommx.v1.Instance` to QUBO, with added support for **inequality constraints** and **integer variables**. Additionally, a new Driver API `to_qubo` has been introduced to simplify the QUBO conversion process.

+++

## ✨ New Features

+++

### Integer variable log-encoding ([#363](https://github.com/Jij-Inc/ommx/pull/363), [#260](https://github.com/Jij-Inc/ommx/pull/260))

Integer variables $x$ are encoded using binary variables $b_i$ as follows:

$$
x = \sum_{i=0}^{m-2} 2^i b_i + (u - l - 2^{m-1} + 1) b_{m-1} + l
$$

This allows optimization problems with integer variables to be handled by QUBO solvers that can only deal with binary variables.

While QUBO solvers return only binary variables, `Instance.evaluate` or `evaluate_samples` automatically restore these integer variables and return them as `ommx.v1.Solution` or `ommx.v1.SampleSet`.

```{code-cell} ipython3
# Example of integer variable log encoding
from ommx.v1 import Instance, DecisionVariable

# Define a problem with three integer variables
x = [
    DecisionVariable.integer(i, lower=0, upper=3, name="x", subscripts=[i])
    for i in range(3)
]
instance = Instance.from_components(
    decision_variables=x,
    objective=sum(x),
    constraints=[],
    sense=Instance.MAXIMIZE,
)
print("Objective function before conversion:", instance.objective)

# Log encode only x0 and x2
instance.log_encode({0, 2})
print("\nObjective function after conversion:", instance.objective)

# Check the generated binary variables
print("\nDecision variable list:")
print(instance.decision_variables[["kind", "lower", "upper", "name", "subscripts"]])

# Restore integer variables from binary variables
print("\nInteger variable restoration:")
solution = instance.evaluate({
    1: 2,          # x1 = 2
    3: 0, 4: 1,    # x0 = x3 + 2*x4 = 0 + 2*1 = 2
    5: 0, 6: 0     # x2 = x5 + 2*x6 = 0 + 2*0 = 0
})
print(solution.extract_decision_variables("x"))
```

### Support for inequality constraints

Two methods have been implemented to convert problems with inequality constraints $ f(x) \leq 0 $ to QUBO:

+++

#### Conversion to equality constraints using integer slack variables ([#366](https://github.com/Jij-Inc/ommx/pull/366))

In this method, the coefficients of the inequality constraint are first represented as rational numbers, and then multiplied by an appropriate rational number $a > 0$ to convert all coefficients of $a f(x)$ to integers. Next, an integer slack variable $s$ is introduced to transform the inequality constraint into an equality constraint $ f(x) + s/a = 0$. The converted equality constraint is then added to the QUBO objective function as a penalty term using existing techniques.

This method can always be applied, but if there are non-divisible coefficients in the polynomial, `a` may become very large, and consequently, the range of `s` may also expand, potentially making it impractical. Therefore, the API allows users to input the upper limit for the range of `s`. The `to_qubo` function described later uses this method by default.

```{code-cell} ipython3
# Example of converting inequality constraints to equality constraints
from ommx.v1 import Instance, DecisionVariable

# Problem with inequality constraint x0 + 2*x1 <= 5
x = [
    DecisionVariable.integer(i, lower=0, upper=3, name="x", subscripts=[i])
    for i in range(3)
]
instance = Instance.from_components(
    decision_variables=x,
    objective=sum(x),
    constraints=[
        (x[0] + 2*x[1] <= 5).set_id(0)   # Set constraint ID
    ],
    sense=Instance.MAXIMIZE,
)
print("Constraint before conversion:", instance.get_constraints()[0])

# Convert inequality constraint to equality constraint
instance.convert_inequality_to_equality_with_integer_slack(
    constraint_id=0,
    max_integer_range=32
)
print("\nConstraint after conversion:", instance.get_constraints()[0])

# Check the added slack variable
print("\nDecision variable list:")
print(instance.decision_variables[["kind", "lower", "upper", "name", "subscripts"]])
```

#### Adding integer slack variables to inequality constraints ([#369](https://github.com/Jij-Inc/ommx/pull/369), [#368](https://github.com/Jij-Inc/ommx/pull/368))

When the above method cannot be applied, an alternative approach is used where integer slack variables $s$ are added to inequality constraints in the form $f(x) + b s \leq 0$. When converting to QUBO, these are added as penalty terms in the form $|f(x) + b s|^2$. Compared to simply adding $|f(x)|^2$, this approach prevents unfairly favoring $f(x) = 0$.

Additionally, `Instance.penalty_method` and `uniform_penalty_method` now accept inequality constraints, handling them in the same way as equality constraints by simply adding them as $|f(x)|^2$.

```{code-cell} ipython3
# Example of adding slack variables to inequality constraints
from ommx.v1 import Instance, DecisionVariable

# Problem with inequality constraint x0 + 2*x1 <= 4
x = [
    DecisionVariable.integer(i, lower=0, upper=3, name="x", subscripts=[i])
    for i in range(3)
]
instance = Instance.from_components(
    decision_variables=x,
    objective=sum(x),
    constraints=[
        (x[0] + 2*x[1] <= 4).set_id(0)   # Set constraint ID
    ],
    sense=Instance.MAXIMIZE,
)
print("Constraint before conversion:", instance.get_constraints()[0])

# Add slack variable to inequality constraint
b = instance.add_integer_slack_to_inequality(
    constraint_id=0,
    slack_upper_bound=2
)
print(f"\nSlack variable coefficient: {b}")
print("Constraint after conversion:", instance.get_constraints()[0])

# Check the added slack variable
print("\nDecision variable list:")
print(instance.decision_variables[["kind", "lower", "upper", "name", "subscripts"]])
```

### Addition of QUBO conversion Driver API `to_qubo` ([#370](https://github.com/Jij-Inc/ommx/pull/370))

A Driver API `to_qubo` has been added that performs a series of operations required for converting from `ommx.v1.Instance` to QUBO (integer variable conversion, inequality constraint conversion, penalty term addition, etc.) in one go. This allows users to obtain QUBO easily without having to be aware of complex conversion steps.

The `to_qubo` function internally executes the following steps in the appropriate order:
1. Convert constraints and objective functions containing integer variables to binary variable representations (e.g., Log Encoding)
2. Convert inequality constraints to equality constraints (default) or to a form suitable for the Penalty Method
3. Convert equality constraints and objective functions to QUBO format
4. Generate an `interpret` function to map QUBO solutions back to the original problem variables

Note that when calling `instance.to_qubo`, the `instance` will be modified.

```{code-cell} ipython3
# Example of using the to_qubo Driver API
from ommx.v1 import Instance, DecisionVariable

# Problem with integer variables and inequality constraint
x = [DecisionVariable.integer(i, lower=0, upper=2, name="x", subscripts=[i]) for i in range(2)]
instance = Instance.from_components(
    decision_variables=x,
    objective=sum(x),
    constraints=[(x[0] + 2*x[1] <= 3).set_id(0)],
    sense=Instance.MAXIMIZE,
)

print("Original problem:")
print(f"Objective function: {instance.objective}")
print(f"Constraint: {instance.get_constraints()[0]}")
print(f"Variables: {[f'{v.name}{v.subscripts}' for v in instance.get_decision_variables()]}")

# Convert to QUBO
qubo, offset = instance.to_qubo()

print("\nAfter QUBO conversion:")
print(f"Offset: {offset}")
print(f"Number of QUBO terms: {len(qubo)}")

# Show only a few terms due to the large number
print("\nSome QUBO terms:")
items = list(qubo.items())[:5]
for (i, j), coeff in items:
    print(f"Q[{i},{j}] = {coeff}")

# Check the converted variables
print("\nVariables after conversion:")
print(instance.decision_variables[["kind", "name", "subscripts"]])

# Confirm that constraints have been removed
print("\nConstraints after conversion:")
print(f"Remaining constraints: {instance.get_constraints()}")
print(f"Removed constraints: {instance.get_removed_constraints()}")
```

## 🐛 Bug Fixes

## 🛠️ Other Changes and Improvements

## 💬 Feedback

With these new features, ommx becomes a powerful tool for converting a wider range of optimization problems to QUBO format and solving them with various QUBO solvers. Try out `ommx` 1.9.0!

Please submit any feedback or bug reports to [GitHub Issues](https://github.com/Jij-Inc/ommx/issues).
