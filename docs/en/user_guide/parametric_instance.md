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

# ommx.ParametricInstance

{class}`~ommx.ParametricInstance` is a class that represents mathematical models similar to {class}`~ommx.Instance`. It also supports parameters (via {class}`~ommx.Parameter`) in addition to decision variables. By assigning values to these parameters, you can create an `ommx.Instance`. Because the resulting `ommx.Instance` keeps the IDs of decision variables and constraints from `ommx.ParametricInstance`, it is helpful when you need to handle a series of models where only some coefficients of the objective function or constraints change.

Consider the following knapsack problem.

$$
\begin{aligned}
\text{maximize} \quad & \sum_{i=1}^{N} p_i x_i \\
\text{subject to} \quad & \sum_{i=1}^{N} w_i x_i \leq W \\
& x_i \in \{0, 1\} \quad (i=1, 2, \ldots, N)
\end{aligned}
$$

Here, $N$ is the number of items, $p_i$ is the value of item i, $w_i$ is the weight of item i, and $W$ is the knapsack's capacity. The variable $x_i$ is binary and indicates whether item i is included in the knapsack. In `ommx.Instance`, fixed values were used for $p_i$ and $w_i$, but here they are treated as parameters.

```{code-cell} ipython3
from ommx import ParametricInstance, DecisionVariable, Parameter, Instance

N = 6
x = [DecisionVariable.binary(id=i, name="x", subscripts=[i]) for i in range(N)]

p = [Parameter(i +   N, name="Profit", subscripts=[i]) for i in range(N)]
w = [Parameter(i + 2*N, name="Weight", subscripts=[i]) for i in range(N)]
W =  Parameter(    3*N, name="Capacity")
```

`ommx.Parameter` also has an ID and uses the same numbering as `ommx.DecisionVariable`, so please ensure there are no duplicates. Like decision variables, parameters can have names and subscripts. They can also be used with operators such as `+` and `<=` to create `ommx.Function` or `ommx.Constraint` objects.

```{code-cell} ipython3
objective = sum(p[i] * x[i] for i in range(N))
constraint = sum(w[i] * x[i] for i in range(N)) <= W
```

Now let’s combine these elements into an `ommx.ParametricInstance` that represents the knapsack problem.

```{code-cell} ipython3
parametric_instance = ParametricInstance.from_components(
    decision_variables=x,
    parameters=p + w + [W],
    objective=objective,
    constraints={0: constraint},
    sense=Instance.MAXIMIZE,
)
```

Like `ommx.Instance`, you can view the decision variables and constraints as DataFrames through the `decision_variables_df()` and `constraints_df()` methods. In addition, `ommx.ParametricInstance` has a `parameters_df()` method for viewing parameter information in a DataFrame.

```{code-cell} ipython3
parametric_instance.parameters_df()
```

Next, let’s assign specific values to the parameters. Use `ParametricInstance.with_parameters`, which takes a dictionary mapping each `ommx.Parameter` ID to its corresponding value.

```{code-cell} ipython3
p_values = { x.id: value for x, value in zip(p, [10, 13, 18, 31, 7, 15]) }
w_values = { x.id: value for x, value in zip(w, [11, 15, 20, 35, 10, 33]) }
W_value = { W.id: 47 }

instance = parametric_instance.with_parameters({**p_values, **w_values, **W_value})
```

````{note}
`ommx.ParametricInstance` cannot handle parameters that change the number of decision variables or parameters (for example, a variable $N$). If you need this functionality, please use a more advanced modeler such as [JijModeling](https://jij-inc.github.io/JijModeling-Tutorials/ja/introduction.html).
````
