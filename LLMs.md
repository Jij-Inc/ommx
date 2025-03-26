# OMMX Documentation for AI Assistants

# Table of Contents
- [Introduction](#introduction)
- [Switch Language](#switch-language)
  - [日本語](https://jij-inc.github.io/ommx/ja/)
- [Tutorial](#tutorial)
  - [Solve With Ommx Adapter](#solve-with-ommx-adapter)
  - [Tsp Sampling With Openjij Adapter](#tsp-sampling-with-openjij-adapter)
  - [Switching Adapters](#switching-adapters)
  - [Share In Ommx Artifact](#share-in-ommx-artifact)
  - [Download Miplib Instance](#download-miplib-instance)
  - [Implement Adapter](#implement-adapter)
- [User Guide](#user-guide)
  - [Function](#function)
  - [Instance](#instance)
  - [Parametric Instance](#parametric-instance)
  - [Solution](#solution)
  - [Sample Set](#sample-set)
- [API Reference](#api-reference)
  - [OMMX Message Schema](https://jij-inc.github.io/ommx/protobuf.html)
  - [OMMX Rust SDK](https://jij-inc.github.io/ommx/rust/ommx/index.html)
  - [OMMX Python SDK](https://jij-inc.github.io/ommx/python/ommx/autoapi/index.html)
- [Release Note](#release-note)
  - [Ommx-1.9.0](#ommx-1.9.0)
  - [Ommx-1.8.0](#ommx-1.8.0)
  - [Ommx-1.7.0](#ommx-1.7.0)
  - [Ommx-1.6.0](#ommx-1.6.0)
  - [Ommx-1.5.0](#ommx-1.5.0)

-------------

## Introduction

### Introduction


OMMX (Open Mathematical prograMming eXchange) is an open data format and SDK designed to simplify data exchange between software and people when applying mathematical optimization to real-world problems.

## Data Exchange in Mathematical Optimization

When applying mathematical optimization to practical use cases, a large amount of data is often generated, requiring both effective management and sharing. Unlike the research phase of optimization, the application phase is divided into multiple stages, each necessitating specialized tools. Consequently, data must be converted to formats appropriate for each tool, making the overall process increasingly complex. By establishing one common format, it becomes easier to integrate multiple tools through a single conversion path to and from that format.


Moreover, these tasks are typically carried out by separate individuals and teams, requiring data handoffs. Metadata is critical in these handoffs to clarify the data’s meaning and intention. For example, if a solution file for an optimization problem lacks details regarding which problem was solved, which solver was used, or what settings were chosen, the file cannot be reused or validated effectively. Standardized metadata helps streamline collaboration and data handling.

## Components of OMMX

To address these data exchange challenges, OMMX was developed. It consists of four main components:

- OMMX Message  
  A data format, independent of programming languages and OS, for exchanging information among software

- OMMX Artifact  
  A package format with metadata that is convenient for exchanging data among people

- OMMX SDK  
  A framework for efficiently creating and manipulating OMMX Messages and OMMX Artifacts

- OMMX Adapters  
  Tools for converting between solver-specific formats and OMMX

### OMMX Message

OMMX Message is a data format defined with [Protocol Buffers](https://protobuf.dev/) to ensure language-agnostic and OS-independent data exchange. It encapsulates schemas for optimization problems ([`ommx.v1.Instance`](./user_guide/instance.ipynb)) and solutions ([`ommx.v1.Solution`](./user_guide/solution.ipynb)). Protocol Buffers allow automatic generation of libraries in many languages, which OMMX SDK provides, especially for Python and Rust.

Data structures such as `ommx.v1.Instance` are called Messages, and each Message has multiple fields. For example, `ommx.v1.Instance` has the following fields (some are omitted for simplicity):

```protobuf
message Instance {
  // Decision variables
  repeated DecisionVariable decision_variables = 2;
  // Objective function
  Function objective = 3;
  // Constraints
  repeated Constraint constraints = 4;
  // Maximization or minimization
  Sense sense = 5;
}
```

Messages such as `ommx.v1.DecisionVariable` representing decision variables and `ommx.v1.Function` representing mathematical functions used as objective functions and constraints are defined under the namespace `ommx.v1`. A list of Messages defined in OMMX is summarized in [OMMX Message Schema](https://jij-inc.github.io/ommx/protobuf.html).

Some solvers can directly read `ommx.v1.Instance`. For those that cannot, OMMX Adapters can be used to convert OMMX Message data into formats the solvers can handle. This makes it simpler to integrate various tools that support OMMX.



### OMMX Artifact

OMMX Artifact is a metadata-rich package format based on the [OCI (Open Container Initiative)](https://opencontainers.org/) standard. An OCI Artifact manages its content as layers and a manifest, assigning a specific [Media Type](https://www.iana.org/assignments/media-types/media-types.xhtml) to each layer. OMMX defines its own Media Types (e.g., `application/org.ommx.v1.instance`), and when these formats are included in OCI Artifacts, they are called OMMX Artifacts.

In OCI Artifact, the contents of the package are managed in units called layers. A single container contains multiple layers and metadata called a Manifest. When reading a container, the Manifest is first checked, and the necessary data is extracted by reading the layers based on that information. Each layer is saved as binary data (BLOB) with metadata called [Media Type](https://www.iana.org/assignments/media-types/media-types.xhtml). For example, when saving a PDF file, the Media Type `application/pdf` is attached, so software reading OCI Artifacts can recognize it as a PDF file by looking at the Media Type.

One major benefit of OCI Artifact compatibility is that standard container registries, such as [DockerHub](https://hub.docker.com/) or [GitHub Container Registry](https://docs.github.com/ja/packages/working-with-a-github-packages-registry/working-with-the-container-registry), can be used to store and distribute data. OMMX uses this mechanism to share large datasets like [MIPLIB 2017](https://miplib.zib.de/), made available at [GitHub Container Registry](https://github.com/Jij-Inc/ommx/pkgs/container/ommx%2Fmiplib2017). For additional details, see [Download MIPLIB Instances](./tutorial/download_miplib_instance.ipynb).





-------------

## Tutorial

### Solve With Ommx Adapter


OMMX provides OMMX Adapter software to enable interoperability with existing mathematical optimization tools. By using OMMX Adapter, you can convert optimization problems expressed in OMMX schemas into formats acceptable to other optimization tools, and convert the resulting data from those tools back into OMMX schemas.

Here, we introduce how to solve a 0-1 Knapsack Problem via OMMX PySCIPOpt Adapter.

## Installing the Required Libraries

First, install OMMX PySCIPOpt Adapter with:

```
pip install ommx-pyscipopt-adapter
```

## Two Steps for Running the Optimization



To solve the 0-1 Knapsack Problem through the OMMX PySCIPOpt Adapter, follow these two steps:

1. Prepare the 0-1 Knapsack problem instance.
2. Run the optimization via OMMX Adapter.

In Step 1, we create an `ommx.v1.Instance` object defined in the OMMX Message Instance schema. There are several ways to generate this object, but in this guide, we'll illustrate how to write it directly using the OMMX Python SDK.

```{tip}
There are four ways to prepare an `ommx.v1.Instance`:
1. Write `ommx.v1.Instance` directly with the OMMX Python SDK.
2. Convert an MPS file to `ommx.v1.Instance` using the OMMX Python SDK.
3. Convert a problem instance from a different optimization tool into `ommx.v1.Instance` using an OMMX Adapter.
4. Export `ommx.v1.Instance` from JijModeling.
```

In Step 2, we convert `ommx.v1.Instance` into a PySCIPOpt `Model` object and run optimization with SCIP. The result is obtained as an `ommx.v1.Solution` object defined by the OMMX Message Solution schema.

### Step 1: Preparing a 0-1 Knapsack Problem Instance

The 0-1 Knapsack problem is formulated as:

$$
\begin{align*}
\mathrm{maximize} \quad & \sum_{i=0}^{N-1} v_i x_i \\
\mathrm{s.t.} \quad & \sum_{i=0}^{n-1} w_i x_i - W \leq 0, \\
& x_{i} \in \{ 0, 1\} 
\end{align*}
$$

We set the following data as parameters for this model.


```python
# Data for 0-1 Knapsack Problem
v = [10, 13, 18, 31, 7, 15]   # Values of each item
w = [11, 25, 20, 35, 10, 33] # Weights of each item
W = 47  # Capacity of the knapsack
N = len(v)  # Total number of items
```

Below is an example code using the OMMX Python SDK to describe this problem instance.


```python
from ommx.v1 import Instance, DecisionVariable

# Define decision variables
x = [
    # Define binary variable x_i
    DecisionVariable.binary(
        # Specify the ID of the decision variable
        id=i,
        # Specify the name of the decision variable
        name="x",
        # Specify the subscript of the decision variable
        subscripts=[i],
    )
    # Prepare binary variables for the number of items
    for i in range(N)
]

# Define the objective function
objective = sum(v[i] * x[i] for i in range(N))

# Define the constraint
constraint = sum(w[i] * x[i] for i in range(N)) - W <= 0
# Specify the name of the constraint
constraint.add_name("Weight limit")

# Create an instance
instance = Instance.from_components(
    # Register all decision variables included in the instance
    decision_variables=x,
    # Register the objective function
    objective=objective,
    # Register all constraints
    constraints=[constraint],
    # Specify that it is a maximization problem
    sense=Instance.MAXIMIZE,
)
```

### Step 2: Running Optimization with OMMX Adapter

To optimize the instance prepared in Step 1, we convert it to a PySCIPOpt `Model` and run SCIP optimization via the OMMX PySCIPOpt Adapter.


```python
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

# Obtain an ommx.v1.Solution objection through a PySCIPOpt model.
solution = OMMXPySCIPOptAdapter.solve(instance)
```

The variable `solution` is an `ommx.v1.Solution` object that holds the results returned by SCIP.

## Analyzing the Results

From the `solution` in Step 2, we can check:

- The optimal solution (which items to pick to maximize total value)
- The optimal value (maximum total value)
- The status of constraints (how close we are to the knapsack weight limit)

We can do this with various properties in the `ommx.v1.Solution` class.

### Analyzing the Optimal Solution

The `decision_variables` property returns a `pandas.DataFrame` containing information on each variable, such as ID, type, name, and value:



```python
solution.decision_variables
```

Using this `pandas.DataFrame`, for example, you can easily create a table in pandas that shows which items are included in the knapsack.


```python
import pandas as pd

df = solution.decision_variables
pd.DataFrame.from_dict(
    {
        "Item number": df.index,
        "Include in knapsack?": df["value"].apply(lambda x: "Include" if x == 1.0 else "Exclude"),
    }
)
```

From this analysis, we see that choosing items 0 and 3 maximizes the total value while satisfying the knapsack’s weight constraint.

### Analyzing the Optimal Value

`objective` stores the best value found. In this case, it should match the sum of items 0 and 3.


```python
import numpy as np
# The expected value is the sum of the values of items 0 and 3
expected = v[0] + v[3]
assert np.isclose(solution.objective, expected)
```

### Analyzing Constraints

The `constraints` property returns a `pandas.DataFrame` that includes details about each constraint’s equality or inequality, its left-hand-side value (`"value"`), name, and more.


```python
solution.constraints
```

Specifically, The `"value"` is helpful for understanding how much slack remains in each constraint. Here, item 0 weighs $11$, item 3 weighs $35$, and the knapsack’s capacity is $47$. Therefore, for the weight constraint 

$$
\begin{align*}
\sum_{i=0}^{n-1} w_i x_i - W \leq 0
\end{align*}
$$
the left-hand side "value" is $-1$, indicating there is exactly 1 unit of slack under the capacity.



-------------

### Tsp Sampling With Openjij Adapter


Here, we explain how to convert a problem to QUBO and perform sampling using the Traveling Salesman Problem as an example.



The Traveling Salesman Problem (TSP) is about finding a route for a salesman to visit multiple cities in sequence. Given the travel costs between cities, we seek to find the path that minimizes the total cost. Let's consider the following city arrangement:


```python
# From ulysses16.tsp in TSPLIB
ulysses16_points = [
    (38.24, 20.42),
    (39.57, 26.15),
    (40.56, 25.32),
    (36.26, 23.12),
    (33.48, 10.54),
    (37.56, 12.19),
    (38.42, 13.11),
    (37.52, 20.44),
    (41.23, 9.10),
    (41.17, 13.05),
    (36.08, -5.21),
    (38.47, 15.13),
    (38.15, 15.35),
    (37.51, 15.17),
    (35.49, 14.32),
    (39.36, 19.56),
]
```

Let's plot the locations of the cities.


```python
%matplotlib inline
from matplotlib import pyplot as plt

x_coords, y_coords = zip(*ulysses16_points)
plt.scatter(x_coords, y_coords)
plt.xlabel('X Coordinate')
plt.ylabel('Y Coordinate')
plt.title('Ulysses16 Points')
plt.show()
```

Let's consider distance as the cost. We'll calculate the distance $d(i, j)$ between city $i$ and city $j$.


```python
def distance(x, y):
    return ((x[0] - y[0])**2 + (x[1] - y[1])**2)**0.5

# Number of cities
N = len(ulysses16_points)
# Distance between each pair of cities
d = [[distance(ulysses16_points[i], ulysses16_points[j]) for i in range(N)] for j in range(N)]
```

Using this, we can formulate TSP as follows. First, let's represent whether we are at city $i$ at time $t$ with a binary variable $x_{t, i}$. Then, we seek $x_{t, i}$ that satisfies the following constraints. The distance traveled by the salesman is given by:

$$
\sum_{t=0}^{N-1} \sum_{i, j = 0}^{N-1} d(i, j) x_{t, i} x_{(t+1 \% N), j}
$$

However, $x_{t, i}$ cannot be chosen freely and must satisfy two constraints: at each time $t$, the salesman can only be in one city, and each city must be visited exactly once:

$$
\sum_{i=0}^{N-1} x_{t, i} = 1, \quad \sum_{t=0}^{N-1} x_{t, i} = 1
$$

Combining these, TSP can be formulated as a constrained optimization problem:

$$
\begin{align*}
\min \quad & \sum_{t=0}^{N-1} \sum_{i, j = 0}^{N-1} d(i, j) x_{t, i} x_{(t+1 \% N), j} \\
\text{s.t.} \quad & \sum_{i=0}^{N-1} x_{t, i} = 1 \quad (\forall t = 0, \ldots, N-1) \\
\quad & \sum_{t=0}^{N-1} x_{t, i} = 1 \quad (\forall i = 0, \ldots, N-1)
\end{align*}
$$

The corresponding `ommx.v1.Instance` can be created as follows:


```python
from ommx.v1 import DecisionVariable, Instance

x = [[
        DecisionVariable.binary(
            i + N * t,  # Decision variable ID
            name="x",           # Name of the decision variable, used when extracting solutions
            subscripts=[t, i])  # Subscripts of the decision variable, used when extracting solutions
        for i in range(N)
    ]
    for t in range(N)
]

objective = sum(
    d[i][j] * x[t][i] * x[(t+1) % N][j]
    for i in range(N)
    for j in range(N)
    for t in range(N)
)
place_constraint = [
    (sum(x[t][i] for i in range(N)) == 1)
        .set_id(t)  # type: ignore
        .add_name("place")
        .add_subscripts([t])
    for t in range(N)
]
time_constraint = [
    (sum(x[t][i] for t in range(N)) == 1)
        .set_id(i + N)  # type: ignore
        .add_name("time")
        .add_subscripts([i])
    for i in range(N)
]

instance = Instance.from_components(
    decision_variables=[x[t][i] for i in range(N) for t in range(N)],
    objective=objective,
    constraints=place_constraint + time_constraint,
    sense=Instance.MINIMIZE
)
```

The variable names and subscripts added to `DecisionVariable.binary` during creation will be used later when interpreting the obtained samples.


## Sampling with OpenJij

To sample the QUBO described by `ommx.v1.Instance` using OpenJij, use the `ommx-openjij-adapter`.


```python
from ommx_openjij_adapter import OMMXOpenJijSAAdapter

sample_set = OMMXOpenJijSAAdapter.sample(instance, num_reads=16, uniform_penalty_weight=20.0)
sample_set.summary
```

[`OMMXOpenJijSAAdapter.sample`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx_openjij_adapter/index.html#ommx_openjij_adapter.OMMXOpenJijSAAdapter.sample) returns [`ommx.v1.SampleSet`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.SampleSet), which stores the evaluated objective function values and constraint violations in addition to the decision variable values of samples. The `SampleSet.summary` property is used to display summary information. `feasible` indicates the feasibility to **the original problem** before conversion to QUBO. This is calculated using the information stored in `removed_constraints` of the `qubo` instance.

To view the feasibility for each constraint, use the `summary_with_constraints` property.


```python
sample_set.summary_with_constraints
```

For more detailed information, you can use the `SampleSet.decision_variables` and `SampleSet.constraints` properties.


```python
sample_set.decision_variables.head(2)
```


```python
sample_set.constraints.head(2)
```

To obtain the samples, use the `SampleSet.extract_decision_variables` method. This interprets the samples using the `name` and `subscripts` registered when creating `ommx.v1.DecisionVariables`. For example, to get the value of the decision variable named `x` with `sample_id=1`, use the following to obtain it in the form of `dict[subscripts, value]`.


```python
sample_id = 1
x = sample_set.extract_decision_variables("x", sample_id)
t = 2
i = 3
x[(t, i)]
```

Since we obtained a sample for $x_{t, i}$, we convert this into a TSP path. This depends on the formulation used, so you need to write the processing yourself.


```python
def sample_to_path(sample: dict[tuple[int, ...], float]) -> list[int]:
    path = []
    for t in range(N):
        for i in range(N):
            if sample[(t, i)] == 1:
                path.append(i)
    return path
```

Let's display this. First, we obtain the IDs of samples that are feasible for the original problem.


```python
feasible_ids = sample_set.summary.query("feasible == True").index
feasible_ids
```

Let's display the optimized paths for these samples.


```python
fig, axie = plt.subplots(3, 3, figsize=(12, 12))

for i, ax in enumerate(axie.flatten()):
    if i >= len(feasible_ids):
        break
    s = feasible_ids[i]
    x = sample_set.extract_decision_variables("x", s)
    path = sample_to_path(x)
    xs = [ulysses16_points[i][0] for i in path] + [ulysses16_points[path[0]][0]]
    ys = [ulysses16_points[i][1] for i in path] + [ulysses16_points[path[0]][1]]
    ax.plot(xs, ys, marker='o')
    ax.set_title(f"Sample {s}, objective={sample_set.objectives[s]:.2f}")

plt.tight_layout()
plt.show()
```



-------------

### Switching Adapters

Solve with multiple adapters and compare the results
======================================================

Since the OMMX Adapter provides a unified API, you can solve the same problem using multiple solvers and compare the results. Let's consider a simple knapsack problem as an example:

$$
\begin{align*}
\mathrm{maximize} \quad & \sum_{i=0}^{N-1} v_i x_i \\
\mathrm{s.t.} \quad & \sum_{i=0}^{n-1} w_i x_i - W \leq 0, \\
& x_{i} \in \{ 0, 1\} 
\end{align*}
$$


```python
from ommx.v1 import Instance, DecisionVariable

v = [10, 13, 18, 31, 7, 15]
w = [11, 25, 20, 35, 10, 33]
W = 47
N = len(v)

x = [
    DecisionVariable.binary(
        id=i,
        name="x",
        subscripts=[i],
    )
    for i in range(N)
]
instance = Instance.from_components(
    decision_variables=x,
    objective=sum(v[i] * x[i] for i in range(N)),
    constraints=[sum(w[i] * x[i] for i in range(N)) - W <= 0],
    sense=Instance.MAXIMIZE,
)
```

## Solve with multiple adapters

Here, we will use the following OSS solvers with corresponding adapters, which are developed as a part of OMMX Python SDK:

| Package name | PyPI | Backend |
|:--- |:--- |:--- |
| `ommx-python-mip-adapter` | [](https://pypi.org/project/ommx-python-mip-adapter/) | [CBC](https://github.com/coin-or/Cbc) via [Python-MIP](https://github.com/coin-or/python-mip) |
| `ommx-pyscipopt-adapter` | [](https://pypi.org/project/ommx-pyscipopt-adapter/) | [SCIP](https://github.com/scipopt/scip) via [PySCIPOpt](https://github.com/scipopt/PySCIPOpt) |
| `ommx-highs-adapter` | [](https://pypi.org/project/ommx-highs-adapter/) | [HiGHS](https://github.com/ERGO-Code/HiGHS) |

For non-OSS solvers, the following adapters also developed as separated repositories:

| Package name | PyPI | Backend |
|:--- |:--- |:--- |
| [ommx-gurobipy-adapter](https://github.com/Jij-Inc/ommx-gurobipy-adapter) | [](https://pypi.org/project/ommx-gurobipy-adapter/) | [Gurobi](https://www.gurobi.com/) |
| [ommx-fixstars-amplify-adapter](https://github.com/Jij-Inc/ommx-fixstars-amplify-adapter) | [](https://pypi.org/project/ommx-fixstars-amplify-adapter/) | [Fixstars Amplify](https://amplify.fixstars.com/ja/docs/amplify/v1/index.html#) |

Here, let's solve the knapsack problem with OSS solvers, Highs, Python-MIP (CBC), and SCIP.


```python
from ommx_python_mip_adapter import OMMXPythonMIPAdapter
from ommx_pyscipopt_adapter  import OMMXPySCIPOptAdapter
from ommx_highs_adapter      import OMMXHighsAdapter

# List of adapters to use
adapters = {
    "highs": OMMXHighsAdapter,
    "cbc": OMMXPythonMIPAdapter,
    "scip": OMMXPySCIPOptAdapter,
}

# Solve the problem using each adapter
solutions = {
    name: adapter.solve(instance) for name, adapter in adapters.items()
}
```

## Compare the results

Since this knapsack problem is simple, all solvers will find the optimal solution.


```python
from matplotlib import pyplot as plt

marks = {
    "highs": "o",
    "cbc": "x",
    "scip": "+",
}

for name, solution in solutions.items():
    x = solution.extract_decision_variables("x")
    subscripts = [key[0] for key in x.keys()]
    plt.plot(subscripts, x.values(), marks[name], label=name)

plt.legend()
```

It would be convenient to concatenate the `pandas.DataFrame` obtained with `decision_variables` when analyzing the results of multiple solvers.


```python
import pandas

decision_variables = pandas.concat([
    solution.decision_variables.assign(solver=solver)
    for solver, solution in solutions.items()
])
decision_variables
```



-------------

### Share In Ommx Artifact


In mathematical optimization workflows, it is important to generate and manage a variety of data. Properly handling these data ensures reproducible computational results and allows teams to share information efficiently.

OMMX provides a straightforward and efficient way to manage different data types. Specifically, it defines a data format called an OMMX Artifact, which lets you store, organize, and share various optimization data through the OMMX SDK.

## Preparation: Data to Share

First, let's prepare the data we want to share. We will create an `ommx.v1.Instance` representing the 0-1 knapsack problem and solve it using SCIP. We will also share the results of our optimization analysis. Details are omitted for brevity.


```python
from ommx.v1 import Instance, DecisionVariable, Constraint
from ommx_pyscipopt_adapter.adapter import OMMXPySCIPOptAdapter
import pandas as pd

# Prepare data for the 0-1 knapsack problem
data = {
    # Values of each item
    "v": [10, 13, 18, 31, 7, 15],
    # Weights of each item
    "w": [11, 15, 20, 35, 10, 33],
    # Knapsack capacity
    "W": 47,
    # Total number of items
    "N": 6,
}

# Define decision variables
x = [
    # Define binary variable x_i
    DecisionVariable.binary(
        # Specify the ID of the decision variable
        id=i,
        # Specify the name of the decision variable
        name="x",
        # Specify the subscript of the decision variable
        subscripts=[i],
    )
    # Prepare num_items binary variables
    for i in range(data["N"])
]

# Define the objective function
objective = sum(data["v"][i] * x[i] for i in range(data["N"]))

# Define constraints
constraint = Constraint(
    # Name of the constraint
    name = "Weight Limit",
    # Specify the left-hand side of the constraint
    function=sum(data["w"][i] * x[i] for i in range(data["N"])) - data["W"],
    # Specify equality constraint (==0) or inequality constraint (<=0)
    equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO,
)

# Create an instance
instance = Instance.from_components(
    # Register all decision variables included in the instance
    decision_variables=x,
    # Register the objective function
    objective=objective,
    # Register all constraints
    constraints=[constraint],
    # Specify that it is a maximization problem
    sense=Instance.MAXIMIZE,
)

# Solve with SCIP
solution = OMMXPySCIPOptAdapter.solve(instance)

# Analyze the optimal solution
df_vars = solution.decision_variables
df = pd.DataFrame.from_dict(
    {
        "Item Number": df_vars.index,
        "Put in Knapsack?": df_vars["value"].apply(lambda x: "Yes" if x == 1.0 else "No"),
    }
)
```


```python
from myst_nb import glue

glue("instance", instance, display=False)
glue("solution", solution, display=False)
glue("data", data, display=False)
glue("df", df, display=False)
```

```{list-table}
:header-rows: 1
:widths: 5 30 10

* - Variable Name
  - Description
  - Value
* - `instance`
  - `ommx.v1.Instance` object representing the 0-1 knapsack problem
  - ````{toggle}
    ```{glue:} instance
    ```
    ````
* - `solution`
  - `ommx.v1.Solution` object containing the results of solving the 0-1 knapsack problem with SCIP
  - ````{toggle}
    ```{glue:} solution
    ```
    ````
* - `data`
  - Input data for the 0-1 knapsack problem
  - ```{glue:} data
    ```
* - `df`
  - `pandas.DataFrame` object representing the optimal solution of the 0-1 knapsack problem
  - {glue:}`df`
```

## Creating an OMMX Artifact as a File

OMMX Artifacts can be managed as files or by assigning them container-like names. Here, we'll show how to save the data as a file. Using the OMMX SDK, we'll store the data in a new file called `my_instance.ommx`. First, we need an `ArtifactBuilder`.


```python
import os
from ommx.artifact import ArtifactBuilder

# Specify the name of the OMMX Artifact file
filename = "my_instance.ommx"

# If the file already exists, remove it
if os.path.exists(filename):
    os.remove(filename)

# 1. Create a builder to create the OMMX Artifact file
builder = ArtifactBuilder.new_archive_unnamed(filename)
```

[`ArtifactBuilder`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.ArtifactBuilder) has several constructors, allowing you to choose whether to manage it by name like a container or as an archive file. If you use a container registry to push and pull like a container, a name is required, but if you use an archive file, a name is not necessary. Here, we use `ArtifactBuilder.new_archive_unnamed` to manage it as an archive file.

| Constructor | Description |
| --- | --- |
| [`ArtifactBuilder.new`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.ArtifactBuilder.new) | Manage by name like a container |
| [`ArtifactBuilder.new_archive`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.ArtifactBuilder.new_archive) | Manage as both an archive file and a container |
| [`ArtifactBuilder.new_archive_unnamed`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.ArtifactBuilder.new_archive_unnamed) | Manage as an archive file |
| [`ArtifactBuilder.for_github`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.ArtifactBuilder.for_github) | Determine the container name according to the GitHub Container Registry |

Regardless of the initialization method, you can save `ommx.v1.Instance` and other data in the same way. Let's add the data prepared above.


```python
# Add ommx.v1.Instance object
desc_instance = builder.add_instance(instance)

# Add ommx.v1.Solution object
desc_solution = builder.add_solution(solution)

# Add pandas.DataFrame object
desc_df = builder.add_dataframe(df, title="Optimal Solution of Knapsack Problem")

# Add an object that can be converted to JSON
desc_json = builder.add_json(data, title="Data of Knapsack Problem")
```

In OMMX Artifacts, data is stored in layers, each with a dedicated media type. Functions like `add_instance` automatically set these media types and add layers. These functions return a `Description` object with information about each created layer.


```python
desc_json.to_dict()
```

The part added as `title="..."` in `add_json` is saved as an annotation of the layer. OMMX Artifact is a data format for humans, so this is basically information for humans to read. The `ArtifactBuilder.add_*` functions all accept optional keyword arguments and automatically convert them to the `org.ommx.user.` namespace.

Finally, call `build` to save it to a file.


```python
# 3. Create the OMMX Artifact file
artifact = builder.build()
```

This `artifact` is the same as the one that will be explained in the next section, which is the one you just saved. Let's check if the file has been created:


```python
! ls $filename
```

Now you can share this `my_instance.ommx` with others using the usual file sharing methods.

## Read OMMX Artifact file

Next, let's read the OMMX Artifact we saved. When loading an OMMX Artifact in archive format, use [`Artifact.load_archive`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.Artifact.load_archive).


```python
from ommx.artifact import Artifact

# Load the OMMX Artifact file locally
artifact = Artifact.load_archive(filename)
```

OMMX Artifacts store data in layers, with a manifest (catalog) that details their contents. You can check the `Descriptor` of each layer, including its Media Type and annotations, without reading the entire archive.


```python
import pandas as pd

# Convert to pandas.DataFrame for better readability
pd.DataFrame({
    "Media Type": desc.media_type,
    "Size (Bytes)": desc.size
  } | desc.annotations
  for desc in artifact.layers
)
```

For instance, to retrieve the JSON in layer 3, use [`Artifact.get_json`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.Artifact.get_json). This function confirms that the Media Type is `application/json` and reinstates the bytes into a Python object.


```python
artifact.get_json(artifact.layers[3])
```


```python
# Remove the created OMMX Artifact file to clean up
! rm $filename
```



-------------

### Download Miplib Instance


The OMMX repository provides mixed-integer programming benchmark instances from MIPLIB 2017 in OMMX Artifact format.

```{note}
More details: The MIPLIB 2017 instances in OMMX Artifact format are hosted in the GitHub Container Registry for the OMMX repository ([link](https://github.com/Jij-Inc/ommx/pkgs/container/ommx%2Fmiplib2017)).

Please see [this page](https://docs.github.com/ja/packages/working-with-a-github-packages-registry/working-with-the-container-registry) for information on GitHub Container Registry.
```

You can easily download these instances with the OMMX SDK, then directly use them as inputs to OMMX Adapters.
For example, to solve the air05 instance from MIPLIB 2017 ([reference](https://miplib.zib.de/instance_details_air05.html)) with PySCIPOpt, you can:

1. Download the air05 instance with `dataset.miplib2017` from the OMMX Python SDK.
2. Solve with PySCIPOpt via the OMMX PySCIPOpt Adapter.

Here is a sample Python code:


```python
# OMMX Python SDK
from ommx import dataset
# OMMX PySCIPOpt Adapter
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

# Step 1: Download the air05 instance from MIPLIB 2017
instance = dataset.miplib2017("air05")

# Step 2: Solve with PySCIPOpt via the OMMX PySCIPOpt Adapter
solution = OMMXPySCIPOptAdapter.solve(instance)
```

This functionality makes it easy to run benchmark tests on multiple OMMX-compatible solvers using the same MIPLIB instances.

## Note about Annotations with the Instance

The downloaded instance includes various annotations accessible via the `annotations` property:


```python
import pandas as pd
# Display annotations in tabular form using pandas
pd.DataFrame.from_dict(instance.annotations, orient="index", columns=["Value"]).sort_index()
```

These instances have both dataset-level annotations and dataset-specific annotations.

There are seven dataset-wide annotations with dedicated properties:

| Annotation                                    | Property          | Description                                               |
|----------------------------------------------|-------------------|-----------------------------------------------------------|
| `org.ommx.v1.instance.authors`               | `authors`         | The authors of the instance                              |
| `org.ommx.v1.instance.constraints`           | `num_constraints` | The number of constraint conditions in the instance      |
| `org.ommx.v1.instance.created`               | `created`         | The date of the instance was saved as an OMMX Artifact   |
| `org.ommx.v1.instance.dataset`               | `dataset`         | The name of the dataset to which this instance belongs   |
| `org.ommx.v1.instance.license`               | `license`         | The license of this dataset                              |
| `org.ommx.v1.instance.title`                 | `title`           | The name of the instance                                 |
| `org.ommx.v1.instance.variables`             | `num_variables`   | The total number of decision variables in the instance   |

MIPLIB-specific annotations are prefixed with `org.ommx.miplib.*`.

For example, the optimal objective of the air05 instance is `26374`, which you can check with the key `org.ommx.miplib.objective`:



```python
# Note that the values of annotations are all strings (str)!
assert instance.annotations["org.ommx.miplib.objective"] == "26374"
```

Thus, we can verify that the optimization result from the OMMX PySCIPOpt Adapter matches the expected optimal value.


```python
import numpy as np

best = float(instance.annotations["org.ommx.miplib.objective"])
assert np.isclose(solution.objective, best)
```



-------------

### Implement Adapter


As mentioned in [Solve with multiple adapters and compare the results](../tutorial/switching_adapters), OMMX Adapters have a common API. This common API is realized by inheriting the abstract base classes provided by the OMMX Python SDK. OMMX provides two abstract base classes depending on the type of adapter:

- `ommx.adapter.SolverAdapter`: An abstract base class for optimization solvers that return one solution
- `ommx.adapter.SamplerAdapter`: An abstract base class for sampling-based optimization solvers

Solvers that produce multiple solutions can be automatically treated as solvers returning a single solution by selecting the best sample. Therefore, `SamplerAdapter` inherits `SolverAdapter`. If you are unsure which one to implement, consider the number of solutions: if the solver returns one solution, use `SolverAdapter`; if it returns multiple solutions, use `SamplerAdapter`. For example, exact solvers like [PySCIPOpt](https://github.com/scipopt/PySCIPOpt) should use `SolverAdapter`, while samplers like [OpenJij](https://github.com/OpenJij/OpenJij) should use `SamplerAdapter`.

In OMMX, a class inheriting `ommx.adapter.SolverAdapter` is called a **Solver Adapter** and one inheriting `ommx.adapter.SamplerAdapter` is called a **Sampler Adapter**.
For clear explaination in this chapter, the software that the adapter wraps (such as PySCIPOpt or OpenJij) is referred as "backend solver".

## Adapter Workflow

The adapter process can be roughly divided into these 3 steps:

1. Convert `ommx.v1.Instance` into a format the backend solver can understand
2. Run the backend solver to obtain a solution
3. Convert the backend solver’s output into `ommx.v1.Solution` or `ommx.v1.SampleSet`

Because the step 2 is nothing but the usage of the backend solver, we assume you to known it well. This tutorial explains steps 1 and 3.

Many backend solvers are designed to receive only the minimum necessary information to represent an optimization problem in a form suitable for their algorithms, whereas `ommx.v1.Instance` contains more information, assuming optimization as part of data analysis. Therefore, step 1 involves discarding much of this information. Additionally, OMMX manages decision variables and constraints with IDs that are not necessarily sequential, while some backend solvers manage them by names or sequential numbers. This correspondence is needed in step 3, so the adapter must manage it.

Conversely, in step 3, `ommx.v1.Solution` or `ommx.v1.SampleSet`, because these stores information same as `ommx.v1.Instance`, cannot be constructed solely from the backend solver's output. Instead, the adapter will construct `ommx.v1.State` or `ommx.v1.Samples` from the backend solver's output and the information from step 1, then convert it to `ommx.v1.Solution` or `ommx.v1.SampleSet` using `ommx.v1.Instance`.

## Implementing a Solver Adapter

Here, we will implement a Solver Adapter using PySCIPOpt as an example. For a complete example, refer to [ommx-pyscipopt-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-pyscipopt-adapter).

For this tutorial, we will proceed in the following order to make it easier to execute step by step:

- Implement functions to construct a PySCIPOpt model from `ommx.v1.Instance` one by one.
- Finally, combine these functions into the `OMMXPySCIPOptAdapter` class.

### Custom Exception

First, it is good to define custom exceptions. This makes it easier for users to understand which part is causing the problem when an exception occurs.


```python
class OMMXPySCIPOptAdapterError(Exception):
    pass
```

OMMX can store a wide range of optimization problems, so there may be cases where the backend solver does not support the problem. In such cases, throw an error.

### Setting Decision Variables

PySCIPOpt manages decision variables by name, so register the OMMX decision variable IDs as strings. This allows you to reconstruct `ommx.v1.State` from PySCIPOpt decision variables in the `decode_to_state` function mentioned later. Note that the appropriate method depends on the backend solver's implementation. The important thing is to retain the information needed to convert to `ommx.v1.State` after obtaining the solution.


```python
import pyscipopt
from ommx.v1 import Instance, Solution, DecisionVariable, Constraint, State, Optimality, Function

def set_decision_variables(model: pyscipopt.Model, instance: Instance) -> dict[str, pyscipopt.Variable]:
    """Add decision variables to the model and create a mapping from variable names to variables"""
    # Create PySCIPOpt variables from OMMX decision variable information
    for var in instance.raw.decision_variables:
        if var.kind == DecisionVariable.BINARY:
            model.addVar(name=str(var.id), vtype="B")
        elif var.kind == DecisionVariable.INTEGER:
            model.addVar(
                name=str(var.id), vtype="I", lb=var.bound.lower, ub=var.bound.upper
            )
        elif var.kind == DecisionVariable.CONTINUOUS:
            model.addVar(
                name=str(var.id), vtype="C", lb=var.bound.lower, ub=var.bound.upper
            )
        else:
            # Throw an error if an unsupported decision variable type is encountered
            raise OMMXPySCIPOptAdapterError(
                f"Unsupported decision variable kind: id: {var.id}, kind: {var.kind}"
            )

    # If the objective is quadratic, add an auxiliary variable for linearization
    if instance.raw.objective.HasField("quadratic"):
        model.addVar(
            name="auxiliary_for_linearized_objective", vtype="C", lb=None, ub=None
        )

    # Create a dictionary to access the variables added to the model
    return {var.name: var for var in model.getVars()}
```

### Converting `ommx.v1.Function` to `pyscipopt.Expr`

Implement a function to convert `ommx.v1.Function` to `pyscipopt.Expr`. Since `ommx.v1.Function` only has the OMMX decision variable IDs, you need to obtain the PySCIPOpt variables from the IDs using the variable name and variable mapping created in `set_decision_variables`.


```python
def make_linear_expr(function: Function, varname_map: dict) -> pyscipopt.Expr:
    """Helper function to generate a linear expression"""
    linear = function.linear
    return (
        pyscipopt.quicksum(
            term.coefficient * varname_map[str(term.id)]
            for term in linear.terms
        )
        + linear.constant
    )

def make_quadratic_expr(function: Function, varname_map: dict) -> pyscipopt.Expr:
    """Helper function to generate a quadratic expression"""
    quad = function.quadratic
    quad_terms = pyscipopt.quicksum(
        varname_map[str(row)] * varname_map[str(column)] * value
        for row, column, value in zip(quad.rows, quad.columns, quad.values)
    )

    linear_terms = pyscipopt.quicksum(
        term.coefficient * varname_map[str(term.id)]
        for term in quad.linear.terms
    )

    constant = quad.linear.constant

    return quad_terms + linear_terms + constant
```

### Setting Objective Function and Constraints

Add the objective function and constraints to the `pyscipopt.Model`. This part requires knowledge of what and how the backend solver supports. For example, in the following code, since PySCIPOpt cannot directly handle quadratic objective functions, an auxiliary variable is introduced according to the [PySCIPOpt documentation](https://pyscipopt.readthedocs.io/en/latest/tutorials/expressions.html#non-linear-objectives).


```python
import math

def set_objective(model: pyscipopt.Model, instance: Instance, varname_map: dict):
    """Set the objective function for the model"""
    objective = instance.raw.objective

    if instance.sense == Instance.MAXIMIZE:
        sense = "maximize"
    elif instance.sense == Instance.MINIMIZE:
        sense = "minimize"
    else:
        raise OMMXPySCIPOptAdapterError(
            f"Sense not supported: {instance.sense}"
        )

    if objective.HasField("constant"):
        model.setObjective(objective.constant, sense=sense)
    elif objective.HasField("linear"):
        expr = make_linear_expr(objective, varname_map)
        model.setObjective(expr, sense=sense)
    elif objective.HasField("quadratic"):
        # Since PySCIPOpt doesn't support quadratic objectives directly, linearize using an auxiliary variable
        auxilary_var = varname_map["auxiliary_for_linearized_objective"]

        # Set the auxiliary variable as the objective
        model.setObjective(auxilary_var, sense=sense)

        # Add a constraint for the auxiliary variable
        expr = make_quadratic_expr(objective, varname_map)
        if sense == "minimize":
            constr_expr = auxilary_var >= expr
        else:  # sense == "maximize"
            constr_expr = auxilary_var <= expr

        model.addCons(constr_expr, name="constraint_for_linearized_objective")
    else:
        raise OMMXPySCIPOptAdapterError(
            "The objective function must be `constant`, `linear`, or `quadratic`."
        )

def set_constraints(model: pyscipopt.Model, instance: Instance, varname_map: dict):
    """Set the constraints for the model"""
    # Process regular constraints
    for constraint in instance.raw.constraints:
        # Generate an expression based on the type of constraint function
        if constraint.function.HasField("linear"):
            expr = make_linear_expr(constraint.function, varname_map)
        elif constraint.function.HasField("quadratic"):
            expr = make_quadratic_expr(constraint.function, varname_map)
        elif constraint.function.HasField("constant"):
            # For constant constraints, check feasibility
            if constraint.equality == Constraint.EQUAL_TO_ZERO and math.isclose(
                constraint.function.constant, 0, abs_tol=1e-6
            ):
                continue
            elif (
                constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
                and constraint.function.constant <= 1e-6
            ):
                continue
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Infeasible constant constraint found: id {constraint.id}"
                )
        else:
            raise OMMXPySCIPOptAdapterError(
                f"Constraints must be either `constant`, `linear` or `quadratic`. id: {constraint.id}, type: {constraint.function.WhichOneof('function')}"
            )

        # Add constraints based on the type (equality/inequality)
        if constraint.equality == Constraint.EQUAL_TO_ZERO:
            constr_expr = expr == 0
        elif constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO:
            constr_expr = expr <= 0
        else:
            raise OMMXPySCIPOptAdapterError(
                f"Not supported constraint equality: id: {constraint.id}, equality: {constraint.equality}"
            )

        # Add the constraint to the model
        model.addCons(constr_expr, name=str(constraint.id))
```

Also, if the backend solver supports special constraints (e.g., [SOS constraints](https://en.wikipedia.org/wiki/Special_ordered_set)), you need to add functions to handle them.

Now, we can construct a `pycscipopt.Model` from `ommx.v1.Instance`.

### Converting Obtained Solutions to `ommx.v1.State`

Next, implement a function to convert the solution obtained by solving the PySCIPOpt model to `ommx.v1.State`. First, check if it is solved. SCIP has functions to guarantee optimality and detect unbounded solutions, so throw corresponding exceptions if detected. This also depends on the backend solver.

```{warning}
Note that `ommx.adapter.InfeasibleDetected` means that the optimization problem itself is infeasible, i.e., **it is guaranteed to have no solutions**. Do not use this when a heuristic solver fails to find any feasible solutions.
```


```python
from ommx.adapter import InfeasibleDetected, UnboundedDetected

def decode_to_state(model: pyscipopt.Model, instance: Instance) -> State:
    """Create an ommx.v1.State from an optimized PySCIPOpt Model"""
    if model.getStatus() == "unknown":
        raise OMMXPySCIPOptAdapterError(
            "The model may not be optimized. [status: unknown]"
        )

    if model.getStatus() == "infeasible":
        raise InfeasibleDetected("Model was infeasible")

    if model.getStatus() == "unbounded":
        raise UnboundedDetected("Model was unbounded")

    try:
        # Get the best solution
        sol = model.getBestSol()
        # Create a mapping from variable names to variables
        varname_map = {var.name: var for var in model.getVars()}
        # Create a State with a mapping from variable IDs to their values
        return State(
            entries={
                var.id: sol[varname_map[str(var.id)]]
                for var in instance.raw.decision_variables
            }
        )
    except Exception:
        raise OMMXPySCIPOptAdapterError(
            f"There is no feasible solution. [status: {model.getStatus()}]"
        )
```

### Creating a Class that Inherits `ommx.adapter.SolverAdapter`

Finally, create a class that inherits `ommx.adapter.SolverAdapter` to standardize the API for each adapter. This is an abstract base class with `@abstractmethod` as follows:

```python
class SolverAdapter(ABC):
    @abstractmethod
    def __init__(self, ommx_instance: Instance):
        pass

    @classmethod
    @abstractmethod
    def solve(cls, ommx_instance: Instance) -> Solution:
        pass

    @property
    @abstractmethod
    def solver_input(self) -> SolverInput:
        pass

    @abstractmethod
    def decode(self, data: SolverOutput) -> Solution:
        pass
```

This abstract base class assumes the following two use cases:

- If you do not adjust the backend solver's parameters, use the `solve` class method.
- If you adjust the backend solver's parameters, use `solver_input` to get the data structure for the backend solver (in this case, `pyscipopt.Model`), adjust it, then input it to the backend solver, and finally convert the backend solver's output using `decode`.

Using the functions prepared so far, you can implement it as follows:


```python
from ommx.adapter import SolverAdapter

class OMMXPySCIPOptAdapter(SolverAdapter):
    def __init__(self, ommx_instance: Instance):
        self.instance = ommx_instance
        self.model = pyscipopt.Model()
        self.model.hideOutput()
        
        # Build the model with helper functions
        self.varname_map = set_decision_variables(self.model, self.instance)
        set_objective(self.model, self.instance, self.varname_map)
        set_constraints(self.model, self.instance, self.varname_map)

    @classmethod
    def solve(cls, ommx_instance: Instance) -> Solution:
        """Solve an ommx.v1.Instance using PySCIPopt and return an ommx.v1.Solution"""
        adapter = cls(ommx_instance)
        model = adapter.solver_input
        model.optimize()
        return adapter.decode(model)

    @property
    def solver_input(self) -> pyscipopt.Model:
        """Return the generated PySCIPopt model"""
        return self.model

    def decode(self, data: pyscipopt.Model) -> Solution:
        """Generate an ommx.v1.Solution from an optimized pyscipopt.Model and the OMMX Instance"""
        if data.getStatus() == "infeasible":
            raise InfeasibleDetected("Model was infeasible")

        if data.getStatus() == "unbounded":
            raise UnboundedDetected("Model was unbounded")

        # Convert the solution to state
        state = decode_to_state(data, self.instance)
        # Evaluate the state using the instance
        solution = self.instance.evaluate(state)

        # Set the optimality status if the model is optimal
        if data.getStatus() == "optimal":
            solution.raw.optimality = Optimality.OPTIMALITY_OPTIMAL

        return solution
```

This completes the Solver Adapter 🎉

```{note}
You can add parameter arguments in the inherited class in Python, so you can define additional parameters as follows. However, while this allows you to use various features of the backend solver, it may compromise compatibility with other adapters, so carefully consider when creating an adapter.

```python
    @classmethod
    def solve(
        cls,
        ommx_instance: Instance,
        *,
        timeout: Optional[int] = None,
    ) -> Solution:
```

### Solving a Knapsack Problem Using the Solver Adapter

For verification, let's solve a knapsack problem using this.


```python
v = [10, 13, 18, 31, 7, 15]
w = [11, 25, 20, 35, 10, 33]
W = 47
N = len(v)

x = [
    DecisionVariable.binary(
        id=i,
        name="x",
        subscripts=[i],
    )
    for i in range(N)
]
instance = Instance.from_components(
    decision_variables=x,
    objective=sum(v[i] * x[i] for i in range(N)),
    constraints=[sum(w[i] * x[i] for i in range(N)) - W <= 0],
    sense=Instance.MAXIMIZE,
)

solution = OMMXPySCIPOptAdapter.solve(instance)
```

## Implementing a Sampler Adapter

Next, let's create a Sampler Adapter using OpenJij. OpenJij includes [`openjij.SASampler`](https://openjij.github.io/OpenJij/reference/openjij/index.html#openjij.SASampler) for Simulated Annealing (SA) and [`openjij.SQASampler`](https://openjij.github.io/OpenJij/reference/openjij/index.html#openjij.SQASampler) for Simulated Quantum Annealing (SQA). In this tutorial, we will use `SASampler` as an example.

For simplicity, this tutorial omits the parameters passed to OpenJij. For more details, refer to the implementation of [`ommx-openjij-adapter`](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-openjij-adapter). For how to use the OpenJij Adapter, refer to [Sampling from QUBO with OMMX Adapter](../tutorial/tsp_sampling_with_openjij_adapter).

### Converting `openjij.Response` to `ommx.v1.Samples`

OpenJij manages decision variables with IDs that are not necessarily sequential, similar to OMMX, so there is no need to create an ID correspondence table as in the case of PySCIPOpt.

The sample results from OpenJij are obtained as `openjij.Response`, so implement a function to convert this to `ommx.v1.Samples`. OpenJij returns the number of occurrences of the same sample as `num_occurrence`. On the other hand, `ommx.v1.Samples` has unique sample IDs for each sample, and the same value samples are compressed as `SamplesEntry`. Note that a conversion is needed to bridge this difference.


```python
import openjij as oj
from ommx.v1 import Instance, SampleSet, Solution, Samples, State

def decode_to_samples(response: oj.Response) -> Samples:
    # Generate sample IDs
    sample_id = 0
    entries = []

    num_reads = len(response.record.num_occurrences)
    for i in range(num_reads):
        sample = response.record.sample[i]
        state = State(entries=zip(response.variables, sample))
        # Encode `num_occurrences` into a list of sample IDs
        ids = []
        for _ in range(response.record.num_occurrences[i]):
            ids.append(sample_id)
            sample_id += 1
        entries.append(Samples.SamplesEntry(state=state, ids=ids))
    return Samples(entries=entries)
```

Note that at this stage, `ommx.v1.Instance` or its extracted correspondence table is not needed because there is no need to consider ID correspondence.

### Implementing a Class that Inherits `ommx.adapter.SamplerAdapter`

In the case of PySCIPOpt, we inherited `SolverAdapter`, but this time we will inherit `SamplerAdapter`. This has three `@abstractmethod` as follows:

```python
class SamplerAdapter(SolverAdapter):
    @classmethod
    @abstractmethod
    def sample(cls, ommx_instance: Instance) -> SampleSet:
        pass

    @property
    @abstractmethod
    def sampler_input(self) -> SamplerInput:
        pass

    @abstractmethod
    def decode_to_sampleset(self, data: SamplerOutput) -> SampleSet:
        pass
```

`SamplerAdapter` inherits from `SolverAdapter`, so you might think you need to implement `solve` and other `@abstractmethod`. However, since `SamplerAdapter` has a function to return the best sample using `sample`, it is sufficient to implement only `sample`. If you want to implement a more efficient implementation yourself, override `solve`.


```python
from ommx.adapter import SamplerAdapter

class OMMXOpenJijSAAdapter(SamplerAdapter):
    """
    Sampling QUBO with Simulated Annealing (SA) by `openjij.SASampler`
    """

    # Retain the Instance because it is required to convert to SampleSet
    ommx_instance: Instance
    
    def __init__(self, ommx_instance: Instance):
        self.ommx_instance = ommx_instance

    # Perform sampling
    def _sample(self) -> oj.Response:
        sampler = oj.SASampler()
        # Convert to QUBO dictionary format
        # If the Instance is not in QUBO format, an error will be raised here
        qubo, _offset = self.ommx_instance.as_qubo_format()
        return sampler.sample_qubo(qubo)

    # Common method for performing sampling
    @classmethod
    def sample(cls, ommx_instance: Instance) -> SampleSet:
        adapter = cls(ommx_instance)
        response = adapter._sample()
        return adapter.decode_to_sampleset(response)
    
    # In this adapter, `SamplerInput` uses a QUBO dictionary
    @property
    def sampler_input(self) -> dict[tuple[int, int], float]:
        qubo, _offset = self.ommx_instance.as_qubo_format()
        return qubo
   
    # Convert OpenJij Response to a SampleSet
    def decode_to_sampleset(self, data: oj.Response) -> SampleSet:
        samples = decode_to_samples(data)
        # The information stored in `ommx.v1.Instance` is required here
        return self.ommx_instance.evaluate_samples(samples)
```

## Summary

In this tutorial, we learned how to implement an OMMX Adapter by connecting to PySCIPOpt as a Solver Adapter and OpenJij as a Sampler Adapter. Here are the key points when implementing an OMMX Adapter:

1. Implement an OMMX Adapter by inheriting the abstract base class `SolverAdapter` or `SamplerAdapter`.
2. The main steps of the implementation are as follows:
   - Convert `ommx.v1.Instance` into a format that the backend solver can understand.
   - Run the backend solver to obtain a solution.
   - Convert the backend solver's output into `ommx.v1.Solution` or `ommx.v1.SampleSet`.
3. Understand the characteristics and limitations of each backend solver and handle them appropriately.
4. Pay attention to managing IDs and mapping variables to bridge the backend solver and OMMX.

If you want to connect your own backend solver to OMMX, refer to this tutorial for implementation. By implementing an OMMX Adapter following this tutorial, you can use optimization with various backend solvers through a common API.

For more detailed implementation examples, refer to the repositories such as [ommx-pyscipopt-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-pyscipopt-adapter) and [ommx-openjij-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-openjij-adapter).



-------------

## User Guide

### Function


In mathematical optimization, functions are used to express objective functions and constraints. Specifically, OMMX handles polynomials and provides the following data structures in OMMX Message to represent polynomials.

| Data Structure | Description |
| --- | --- |
| [ommx.v1.Linear](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Linear) | Linear function. Holds pairs of variable IDs and their coefficients |
| [ommx.v1.Quadratic](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Quadratic) | Quadratic function. Holds pairs of variable ID pairs and their coefficients |
| [ommx.v1.Polynomial](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Polynomial) | Polynomial. Holds pairs of variable ID combinations and their coefficients |
| [ommx.v1.Function](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Function) | One of the above or a constant |


## Creating ommx.v1.Function
In the Python SDK, there are two main approachs to create these data structures. The first approach is to directly call the constructors of each data structure. For example, you can create `ommx.v1.Linear` as follows.


```python
from ommx.v1 import Linear

linear = Linear(terms={1: 1.0, 2: 2.0}, constant=3.0)
print(linear)
```

In this way, decision variables are identified by IDs and coefficients are represented by real numbers. To access coefficients and constant values, use the `terms` and `constant` properties.


```python
print(f"{linear.terms=}, {linear.constant=}")
```

Another approach is to create from `ommx.v1.DecisionVariable`. `ommx.v1.DecisionVariable` is a data structure that only holds the ID of the decision variable. When creating polynomials such as `ommx.v1.Linear`, you can first create decision variables using `ommx.v1.DecisionVariable` and then use them to create polynomials.


```python
from ommx.v1 import DecisionVariable

x = DecisionVariable.binary(1, name="x")
y = DecisionVariable.binary(2, name="y")

linear = x + 2.0 * y + 3.0
print(linear)
```

Note that the polynomial data type retains only the ID of the decision variable and does not store additional information. In the above example, information passed to `DecisionVariable.binary` such as `x` and `y` is not carried over to `Linear`. This second method can create polynomials of any degree.


```python
q = x * x + x * y + y * y
print(q)
```


```python
p = x * x * x + y * y
print(p)
```

`Linear`, `Quadratic`, and `Polynomial` each have their own unique data storage methods, so they are separate Messages. However, since any of them can be used as objective functions or constraints, a Message called `Function` is provided, which can be any of the above or a constant.


```python
from ommx.v1 import Function

# Constant
print(Function(1.0))
# Linear
print(Function(linear))
# Quadratic
print(Function(q))
# Polynomial
print(Function(p))
```

## Substitution and Partial Evaluation of Decision Variables

`Function` and other polynomials have an `evaluate` method that substitutes values for decision variables. For example, substituting $x_1 = 1$ and $x_2 = 0$ into the linear function $x_1 + 2x_2 + 3$ created above results in $1 + 2 \times 0 + 3 = 4$.


```python
value, used_id = linear.evaluate({1: 1, 2: 0})
print(f"{value=}, {used_id=}")
```

The argument supports the format `dict[int, float]` and `ommx.v1.State`. `evaluate` returns the evaluated value and the IDs of the decision variables used. This is useful when you want to know which parts were used when evaluating against `ommx.v1.State`, which is the solution obtained by solving the optimization problem. `evaluate` returns an error if the necessary decision variable IDs are missing.


```python
try:
    linear.evaluate({1: 1})
except RuntimeError as e:
    print(f"Error: {e}")
```

If you want to substitute values for only some of the decision variables, use the `partial_evaluate` method. This takes the same arguments as `evaluate` but returns the decision variables without assigned values unevaluated.


```python
linear2, used_id = linear.partial_evaluate({1: 1})
print(f"{linear2=}, {used_id=}")
```

The result of partial evaluation is a polynomial, so it is returned in the same type as the original polynomial.

## Comparison of Coefficients

`Function` and other polynomial types have an `almost_equal` function. This function determines whether the coefficients of the polynomial match within a specified error. For example, to confirm that $ (x + 1)^2 = x^2 + 2x + 1 $, write as follows


```python
xx = (x + 1) * (x + 1)
xx.almost_equal(x * x + 2 * x + 1)
```



-------------

### Instance


[`ommx.v1.Instance`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance) is a data structure for describing the optimization problem itself (mathematical model). It consists of the following components:

- Decision variables ([`decision_variables`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.decision_variables))
- Objective function ([`objective`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.objective))
- Constraints ([`constraints`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.constraints))
- Maximization/Minimization ([`sense`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.sense))

For example, let's consider a simple optimization problem:

$$
\begin{align}
\max \quad & x + y \\
\text{subject to} \quad & x y  = 0 \\
& x, y \in \{0, 1\}
\end{align}
$$

The corresponding `ommx.v1.Instance` is as follows.


```python
from ommx.v1 import Instance, DecisionVariable

x = DecisionVariable.binary(1, name='x')
y = DecisionVariable.binary(2, name='y')

instance = Instance.from_components(
    decision_variables=[x, y],
    objective=x + y,
    constraints=[x * y == 0],
    sense=Instance.MAXIMIZE
)
```

Each of these components has a corresponding property. The objective function is converted into the form of [`ommx.v1.Function`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Function), as explained in the previous section.


```python
instance.objective
```

`sense` is set to `Instance.MAXIMIZE` for maximization problems or `Instance.MINIMIZE` for minimization problems.


```python
instance.sense == Instance.MAXIMIZE
```

## Decision Variables

Decision variables and constraints can be obtained in the form of [`pandas.DataFrame`](https://pandas.pydata.org/pandas-docs/stable/reference/frame.html).


```python
instance.decision_variables
```

First, `kind`, `lower`, and `upper` are essential information for the mathematical model.

- `kind` specifies the type of decision variable, which can be Binary, Integer, Continuous, SemiInteger, or SemiContinuous.
- `lower` and `upper` are the lower and upper bounds of the decision variable. For Binary variables, this range is $[0, 1]$.

Additionally, OMMX is designed to handle metadata that may be needed when integrating mathematical optimization into practical data analysis. While this metadata does not affect the mathematical model itself, it is useful for data analysis and visualization.

- `name` is a human-readable name for the decision variable. In OMMX, decision variables are always identified by ID, so this `name` may be duplicated. It is intended to be used in combination with `subscripts`, which is described later.
- `description` is a more detailed explanation of the decision variable.
- When dealing with many mathematical optimization problems, decision variables are often handled as multidimensional arrays. For example, it is common to consider constraints with subscripts like $x_i + y_i \leq 1, \forall i \in [1, N]$. In this case, `x` and `y` are the names of the decision variables, so they are stored in `name`, and the part corresponding to $i$ is stored in `subscripts`. `subscripts` is a list of integers, but if the subscript cannot be represented as an integer, there is a `parameters` property that allows storage in the form of `dict[str, str]`.

If you need a list of [`ommx.v1.DecisionVariable`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.DecisionVariable) directly, you can use the [`get_decision_variables`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.get_constraints) method.


```python
for v in instance.get_decision_variables():
    print(f"{v.id=}, {v.name=}")
```

To obtain `ommx.v1.DecisionVariable` from the ID of the decision variable, you can use the [`get_decision_variable`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.get_decision_variable) method.


```python
x1 = instance.get_decision_variable(1)
print(f"{x1.id=}, {x1.name=}")
```

## Constraints
Next, let's look at the constraints.


```python
instance.constraints
```

In OMMX, constraints are also managed by ID. This ID is independent of the decision variable ID. When you create a constraint like `x * y == 0`, a sequential number is automatically assigned. To manually set the ID, you can use the [`set_id`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Constraint.set_id) method.


```python
c = (x * y == 0).set_id(100)
print(f"{c.id=}")
```

The essential information for constraints is `id` and `equality`. `equality` indicates whether the constraint is an equality constraint ([`Constraint.EQUAL_TO_ZERO`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Constraint.EQUAL_TO_ZERO)) or an inequality constraint ([`Constraint.LESS_THAN_OR_EQUAL_TO_ZERO`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Constraint.LESS_THAN_OR_EQUAL_TO_ZERO)). Note that constraints of the type $f(x) \geq 0$ are treated as $-f(x) \leq 0$.

Constraints can also store metadata similar to decision variables. You can use `name`, `description`, `subscripts`, and `parameters`. These can be set using the [`add_name`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Constraint.add_name), [`add_description`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Constraint.add_description), [`add_subscripts`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Constraint.add_subscripts), and [`add_parameters`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Constraint.add_parameters) methods.


```python
c = (x * y == 0).set_id(100).add_name("prod-zero")
print(f"{c.id=}, {c.name=}")
```

You can also use the [`get_constraints`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.get_constraints) method to directly obtain a list of [`ommx.v1.Constraint`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Constraint). To obtain `ommx.v1.Constraint` by its the constraint ID, use the [`get_constraint`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.get_constraint) method.


```python
for c in instance.get_constraints():
    print(c)
```



-------------

### Parametric Instance


[`ommx.v1.ParametricInstance`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.ParametricInstance) is a class that represents mathematical models similar to [`ommx.v1.Instance`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance). It also supports parameters (via [`ommx.v1.Parameter`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Parameter)) in addition to decision variables. By assigning values to these parameters, you can create an `ommx.v1.Instance`. Because the resulting `ommx.v1.Instance` keeps the IDs of decision variables and constraints from `ommx.v1.ParametricInstance`, it is helpful when you need to handle a series of models where only some coefficients of the objective function or constraints change.

Consider the following knapsack problem.

$$
\begin{align*}
\text{maximize} \quad & \sum_{i=1}^{N} p_i x_i \\
\text{subject to} \quad & \sum_{i=1}^{N} w_i x_i \leq W \\
& x_i \in \{0, 1\} \quad (i=1, 2, \ldots, N)
\end{align*}
$$

Here, $N$ is the number of items, $p_i$ is the value of item i, $w_i$ is the weight of item i, and $W$ is the knapsack's capacity. The variable $x_i$ is binary and indicates whether item i is included in the knapsack. In `ommx.v1.Instance`, fixed values were used for $p_i$ and $w_i$, but here they are treated as parameters.


```python
from ommx.v1 import ParametricInstance, DecisionVariable, Parameter, Instance

N = 6
x = [DecisionVariable.binary(id=i, name="x", subscripts=[i]) for i in range(N)]

p = [Parameter.new(id=i+  N, name="Profit", subscripts=[i]) for i in range(N)]
w = [Parameter.new(id=i+2*N, name="Weight", subscripts=[i]) for i in range(N)]
W =  Parameter.new(id=  3*N, name="Capacity")
```

`ommx.v1.Parameter` also has an ID and uses the same numbering as `ommx.v1.DecisionVariable`, so please ensure there are no duplicates. Like decision variables, parameters can have names and subscripts. They can also be used with operators such as `+` and `<=` to create `ommx.v1.Function` or `ommx.v1.Constraint` objects.


```python
objective = sum(p[i] * x[i] for i in range(N))
constraint = sum(w[i] * x[i] for i in range(N)) <= W
```

Now let’s combine these elements into an `ommx.v1.ParametricInstance` that represents the knapsack problem.


```python
parametric_instance = ParametricInstance.from_components(
    decision_variables=x,
    parameters=p + w + [W],
    objective=objective,
    constraints=[constraint],
    sense=Instance.MAXIMIZE,
)
```

Like `ommx.v1.Instance`, you can view the decision variables and constraints as DataFrames through the `decision_variables` and `constraints` properties. In addition, `ommx.v1.ParametricInstance` has a `parameters` property for viewing parameter information in a DataFrame.


```python
parametric_instance.parameters
```

Next, let’s assign specific values to the parameters. Use `ParametricInstance.with_parameters`, which takes a dictionary mapping each `ommx.v1.Parameter` ID to its corresponding value.


```python
p_values = { x.id: value for x, value in zip(p, [10, 13, 18, 31, 7, 15]) }
w_values = { x.id: value for x, value in zip(w, [11, 15, 20, 35, 10, 33]) }
W_value = { W.id: 47 }

instance = parametric_instance.with_parameters({**p_values, **w_values, **W_value})
```

````{note}
`ommx.v1.ParametricInstance` cannot handle parameters that change the number of decision variables or parameters (for example, a variable $N$). If you need this functionality, please use a more advanced modeler such as [JijModeling](https://jij-inc.github.io/JijModeling-Tutorials/ja/introduction.html).
````



-------------

### Solution


OMMX has several structures that represent the solution of mathematical models.

| Data Structure | Description |
| --- | --- |
| [`ommx.v1.State`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/solution_pb2/index.html#ommx.v1.solution_pb2.State) | Holds the solution value for the decision variable ID. The simplest representation of a solution. |
| [`ommx.v1.Solution`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Solution) | A representation of the solution intended to be human-readable. In addition to the values of the decision variables and the evaluation values of the constraints, it also holds metadata for the decision variables and constraints added to the [`ommx.v1.Instance`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance). |

Most solvers are software designed to solve mathematical models, so they return minimal information equivalent to `ommx.v1.State`, but OMMX mainly handles `ommx.v1.Solution`, which allows users to easily check the optimization results.

`ommx.v1.Solution` is generated by passing `ommx.v1.State` or equivalent `dict[int, float]` to the `ommx.v1.Instance.evaluate` method. Let's consider the simple optimization problem we saw in the previous section again:

$$
\begin{align}
\max \quad & x + y \\
\text{subject to} \quad & x y  = 0 \\
& x, y \in \{0, 1\}
\end{align}
$$

It is clear that this has a feasible solution $x = 1, y = 0$.


```python
from ommx.v1 import Instance, DecisionVariable

# Create a simple instance
x = DecisionVariable.binary(1, name='x')
y = DecisionVariable.binary(2, name='y')

instance = Instance.from_components(
    decision_variables=[x, y],
    objective=x + y,
    constraints=[x * y == 0],
    sense=Instance.MAXIMIZE
)

# Create a solution
solution = instance.evaluate({1: 1, 2: 0})  # x=1, y=0
```

The generated `ommx.v1.Solution` inherits most of the information from the `ommx.v1.Instance`. Let's first look at the decision variables.


```python
solution.decision_variables
```

In addition to the required attributes—ID, `kind`, `lower`, and `upper`-it also inherits metadata such as `name`. Additionally, the `value` stores which was assigned in `evaluate`.  Similarly, the evaluation value is added to the constraints as `value`.


```python
solution.constraints
```

The `objective` property contains the value of the objective function, and the `feasible` property contains whether the constraints are satisfied.


```python
print(f"{solution.objective=}, {solution.feasible=}")
```

Since $xy = 0$ when $x = 1, y = 0$, all constraints are satisfied, so `feasible` is `True`. The value of the objective function is $x + y = 1$.

What happens in the case of an infeasible solution, $x = 1, y = 1$?


```python
solution11 = instance.evaluate({1: 1, 2: 1})  # x=1, y=1
print(f"{solution11.objective=}, {solution11.feasible=}")
```

`feasible = False` indicates that it is an infeasible solution.



-------------

### Sample Set

ommx.v1.SampleSet
=================

[`ommx.v1.Solution`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Solution) represents a single solution returned by a solver. However, some solvers, often called samplers, can return multiple solutions. To accommodate this, OMMX provides two data structures for representing multiple solutions:

| Data Structure  | Description |
|:---------------|:------------|
| [`ommx.v1.Samples`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/sample_set_pb2/index.html#ommx.v1.sample_set_pb2.Samples) | A list of multiple solutions for decision variable IDs |
| [`ommx.v1.SampleSet`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.SampleSet) | Evaluations of objective and constraints with decision variables |

`Samples` corresponds to `State` and `SampleSet` corresponds to `Solution`. This notebook explains how to use `SampleSet`.

Creating a SampleSet
--------------------

Let's consider a simple optimization problem：

$$
\begin{align*}
    \max &\quad x_1 + 2 x_2 + 3 x_3 \\
    \text{s.t.} &\quad x_1 + x_2 + x_3 = 1 \\
    &\quad x_1, x_2, x_3 \in \{0, 1\}
\end{align*}
$$


```python
from ommx.v1 import DecisionVariable, Instance

x = [DecisionVariable.binary(i) for i in range(3)]

instance = Instance.from_components(
    decision_variables=x,
    objective=x[0] + 2*x[1] + 3*x[2],
    constraints=[sum(x) == 1],
    sense=Instance.MAXIMIZE,
)
```

Normally, solutions are provided by a solver, commonly referred to as a sampler, but for simplicity, we prepare them manually here. `ommx.v1.Samples` can hold multiple samples, each expressed as a set of values associated with decision variable IDs, similar to `ommx.v1.State`.

Each sample is assigned an ID. Some samplers issue their own IDs for logging, so OMMX allows specifying sample IDs. If omitted, IDs are assigned sequentially starting from `0`.

A helper function `ommx.v1.to_samples` can convert to `ommx.v1.Samples`.


```python
from ommx.v1 import to_samples
from ommx.v1.sample_set_pb2 import Samples

# When specifying Sample ID
samples = to_samples({
    0: {0: 1, 1: 0, 2: 0},  # x1 = 1, x2 = x3 = 0
    1: {0: 0, 1: 0, 2: 1},  # x3 = 1, x1 = x2 = 0
    2: {0: 1, 1: 1, 2: 0},  # x1 = x2 = 1, x3 = 0 (infeasible)
})# ^ sample ID
assert isinstance(samples, Samples)

# When automatically assigning Sample ID
samples = to_samples([
    {0: 1, 1: 0, 2: 0},  # x1 = 1, x2 = x3 = 0
    {0: 0, 1: 0, 2: 1},  # x3 = 1, x1 = x2 = 0
    {0: 1, 1: 1, 2: 0},  # x1 = x2 = 1, x3 = 0 (infeasible)
])
assert isinstance(samples, Samples)
```

While `ommx.v1.Solution` is obtained via `Instance.evaluate`, `ommx.v1.SampleSet` can be obtained via `Instance.evaluate_samples`.


```python
sample_set = instance.evaluate_samples(samples)
sample_set.summary
```

The `summary` attribute displays each sample's objective value and feasibility in a DataFrame format. For example, the sample with `sample_id=2` is infeasible and shows `feasible=False`. The table is sorted with feasible samples appearing first, and within them, those with better bjective values (depending on whether `Instance.sense` is maximization or minimization) appear at the top.

```{note}
For clarity, we explicitly pass `ommx.v1.Samples` created by `to_samples` to `evaluate_samples`, but you can omit it because `to_samples` would be called automatically.
```

Extracting individual samples
----------------------------
You can use `SampleSet.get` to retrieve each sample as an `ommx.v1.Solution` by specifying the sample ID:


```python
from ommx.v1 import Solution

solution = sample_set.get(sample_id=0)
assert isinstance(solution, Solution)

print(f"{solution.objective=}")
solution.decision_variables
```

Retrieving the best solution
---------------------------
`SampleSet.best_feasible` returns the best feasible sample, meaning the one with the highest objective value among all feasible samples:


```python
solution = sample_set.best_feasible()

print(f"{solution.objective=}")
solution.decision_variables
```

Of course, if the problem is a minimization, the sample with the smallest objective value will be returned. If no feasible samples exist, an error will be raised.


```python
sample_set_infeasible = instance.evaluate_samples([
    {0: 1, 1: 1, 2: 0},  # Infeasible since x0 + x1 + x2 = 2
    {0: 1, 1: 0, 2: 1},  # Infeasible since x0 + x1 + x2 = 2
])

# Every samples are infeasible
display(sample_set_infeasible.summary)

try:
    sample_set_infeasible.best_feasible()
    assert False # best_feasible() should raise RuntimeError
except RuntimeError as e:
    print(e)
```

```{note}
OMMX does not provide a method to determine which infeasible solution is the best, as many different criteria can be considered. Implement it yourself if needed.
```



-------------

## Release Note

### Ommx-1.9.0


This release significantly enhances the conversion functionality from `ommx.v1.Instance` to QUBO, with added support for **inequality constraints** and **integer variables**. Additionally, a new Driver API `to_qubo` has been introduced to simplify the QUBO conversion process.

## ✨ New Features

### Integer variable log-encoding ([#363](https://github.com/Jij-Inc/ommx/pull/363), [#260](https://github.com/Jij-Inc/ommx/pull/260))

Integer variables $x$ are encoded using binary variables $b_i$ as follows:

$$
x = \sum_{i=0}^{m-2} 2^l b_i + (u - l - 2^{m-1} + 1) b_{m-1} + l
$$

This allows optimization problems with integer variables to be handled by QUBO solvers that can only deal with binary variables.

While QUBO solvers return only binary variables, `Instance.evaluate` or `evaluate_samples` automatically restore these integer variables and return them as `ommx.v1.Solution` or `ommx.v1.SampleSet`.


```python
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

#### Conversion to equality constraints using integer slack variables ([#366](https://github.com/Jij-Inc/ommx/pull/366))

In this method, the coefficients of the inequality constraint are first represented as rational numbers, and then multiplied by an appropriate rational number $a > 0$ to convert all coefficients of $a f(x)$ to integers. Next, an integer slack variable $s$ is introduced to transform the inequality constraint into an equality constraint $ f(x) + s/a = 0$. The converted equality constraint is then added to the QUBO objective function as a penalty term using existing techniques.

This method can always be applied, but if there are non-divisible coefficients in the polynomial, `a` may become very large, and consequently, the range of `s` may also expand, potentially making it impractical. Therefore, the API allows users to input the upper limit for the range of `s`. The `to_qubo` function described later uses this method by default.


```python
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


```python
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


```python
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



-------------

### Ommx-1.8.0


[](https://github.com/Jij-Inc/ommx/releases/tag/python-1.8.0)

Please refer to the GitHub Release for individual changes.

⚠️ Includes breaking changes due to the addition of `SolverAdapter`.

Summary
--------
- Added a new `SolverAdapter` abstract base class to serve as a common interface for adapters to different solvers.
- `ommx-python-mip-adapter` and `ommx-pyscipopt-adapter` have been changed to use `SolverAdapter` according to the [adapter implementation guide](https://jij-inc.github.io/ommx/en/ommx_ecosystem/solver_adapter_guide.html)
  - ⚠️ This is a breaking change. Code using these adapters will need to be updated.
  - Other adapters will be updated in future versions. 

# Solver Adapter 

The introduction of the `SolverAdapter` base class aims to make the API for different adapters more consistent. `ommx-python-mip-adapter` and `ommx-pyscipopt-adapter` now use the `SolverAdapter` base class.

Here is an example of the new Adapter interface to simply solve an OMMX instance.


```python
from ommx.v1 import Instance, DecisionVariable
from ommx_python_mip_adapter import OMMXPythonMIPAdapter

p = [10, 13, 18, 32, 7, 15]
w = [11, 15, 20, 35, 10, 33]
x = [DecisionVariable.binary(i) for i in range(6)]
instance = Instance.from_components(
    decision_variables=x,
    objective=sum(p[i] * x[i] for i in range(6)),
    constraints=[sum(w[i] * x[i] for i in range(6)) <= 47],
    sense=Instance.MAXIMIZE,
)

solution = OMMXPythonMIPAdapter.solve(instance)
solution.objective
```

With the new update, the process looks the same as the above when using the `OMMXPySCIPOptAdapter` class instead.

To replace the usage of `instance_to_model()` functions, you can instantiating an adapter and using `solver_input`. You can then apply any solver-specific parameters before optimizing manually, then calling `decode()` to obtain the OMMX solution.


```python
adapter = OMMXPythonMIPAdapter(instance)
model = adapter.solver_input # in OMMXPythonMIPAdapter's case, this is a `mip.Model` object
# modify model parameters here
model.optimize() 
solution = adapter.decode(model)
solution.objective
```



-------------

### Ommx-1.7.0


[](https://github.com/Jij-Inc/ommx/releases/tag/python-1.7.0)

Please refer to the GitHub Release for individual changes.

Summary
--------
- [English Jupyter Book](https://jij-inc.github.io/ommx/en/introduction.html)
- QPLIB format parser
- Several APIs have been added to `ommx.v1.SampleSet` and `ommx.v1.ParametricInstance`, and integration with OMMX Artifact has been added.
  - For `ommx.v1.SampleSet`, please refer to the [new explanation page](https://jij-inc.github.io/ommx/en/ommx_message/sample_set.html)
  - For support of OMMX Artifact, please refer to the API reference [ommx.artifact.Artifact](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.Artifact) and [ommx.artifact.ArtifactBuilder](https://jij-inc.github.io/ommx/python/autoapi/ommx/artifact/index.html#ommx.artifact.ArtifactBuilder).
- Change in behavior of `{Solution, SampleSet}.feasible`

QPLIB format parser
---------------------------

Following the MPS format, support for the QPLIB format parser has been added.


```python
import tempfile

# Example problem from QPLIB
#
# Furini, Fabio, et al. "QPLIB: a library of quadratic programming instances." Mathematical Programming Computation 11 (2019): 237-265 pages 42 & 43
# https://link.springer.com/article/10.1007/s12532-018-0147-4
contents = """
! ---------------
! example problem
! ---------------
MIPBAND # problem name
QML # problem is a mixed-integer quadratic program
Minimize # minimize the objective function
3 # variables
2 # general linear constraints
5 # nonzeros in lower triangle of Q^0
1 1 2.0 5 lines row & column index & value of nonzero in lower triangle Q^0
2 1 -1.0 |
2 2 2.0 |
3 2 -1.0 |
3 3 2.0 |
-0.2 default value for entries in b_0
1 # non default entries in b_0
2 -0.4 1 line of index & value of non-default values in b_0
0.0 value of q^0
4 # nonzeros in vectors b^i (i=1,...,m)
1 1 1.0 4 lines constraint, index & value of nonzero in b^i (i=1,...,m)
1 2 1.0 |
2 1 1.0 |
2 3 1.0 |
1.0E+20 infinity
1.0 default value for entries in c_l
0 # non default entries in c_l
1.0E+20 default value for entries in c_u
0 # non default entries in c_u
0.0 default value for entries in l
0 # non default entries in l
1.0 default value for entries in u
1 # non default entries in u
2 2.0 1 line of non-default indices and values in u
0 default variable type is continuous
1 # non default variable types
3 2 variable 3 is binary
1.0 default value for initial values for x
0 # non default entries in x
0.0 default value for initial values for y
0 # non default entries in y
0.0 default value for initial values for z
0 # non default entries in z
0 # non default names for variables
0 # non default names for constraints"#;
"""

# Create a named temporary file
with tempfile.NamedTemporaryFile(delete=False, suffix='.qplib') as temp_file:
    temp_file.write(contents.encode())
    qplib_sample_path = temp_file.name


print(f"QPLIB sample file created at: {qplib_sample_path}")
```


```python
from ommx import qplib

# Load a QPLIB file
instance = qplib.load_file(qplib_sample_path)

# Display decision variables and constraints
display(instance.decision_variables)
display(instance.constraints)
```

Change in behavior of `{Solution, SampleSet}.feasible`
---------------------

- The behavior of `feasible` in `ommx.v1.Solution` and `ommx.v1.SampleSet` has been changed.
  - The handling of `removed_constraints` introduced in Python SDK 1.6.0 has been changed. In 1.6.0, `feasible` ignored `removed_constraints`, but in 1.7.0, `feasible` now considers `removed_constraints`.
  - Additionally, `feasible_relaxed` which explicitly ignores `removed_constraints` and `feasible_unrelaxed` which considers `removed_constraints` have been introduced. `feasible` is an alias for `feasible_unrelaxed`.


To understand the behavior, let's consider the following simple optimization problem:

$$
\begin{align*}
    \max &\quad x_0 + x_1 + x_2 \\
    \text{s.t.} &\quad x_0 + x_1 \leq 1 \\
                &\quad x_1 + x_2 \leq 1 \\
    &\quad x_1, x_2, x_3 \in \{0, 1\}
\end{align*}
$$


```python
from ommx.v1 import DecisionVariable, Instance

x = [DecisionVariable.binary(i) for i in range(3)]

instance = Instance.from_components(
    decision_variables=x,
    objective=sum(x),
    constraints=[
        (x[0] + x[1] <= 1).set_id(0),
        (x[1] + x[2] <= 1).set_id(1),
    ],
    sense=Instance.MAXIMIZE,
)
instance.constraints
```

Next, we relax one of the constraints $x_0 + x_1 \leq 1$.


```python
instance.relax_constraint(constraint_id=0, reason="Manual relaxation")
display(instance.constraints)
display(instance.removed_constraints)
```

Now, $x_0 = 1, x_1 = 1, x_2 = 0$ is not a solution to the original problem, but it is a solution to the relaxed problem. Therefore, `feasible_relaxed` will be `True`, but `feasible_unrelaxed` will be `False`. Since `feasible` is an alias for `feasible_unrelaxed`, it will be `False`.


```python
solution = instance.evaluate({0: 1, 1: 1, 2: 0})
print(f"{solution.feasible=}")
print(f"{solution.feasible_relaxed=}")
print(f"{solution.feasible_unrelaxed=}")
```



-------------

### Ommx-1.6.0


[](https://github.com/Jij-Inc/ommx/releases/tag/python-1.6.0)

Summary
--------

- OMMX starts to support QUBO.
  - New adapter package [ommx-openjij-adapter](https://pypi.org/project/ommx-openjij-adapter/) has been added.
  - Please see new [tutorial page](https://jij-inc.github.io/ommx/en/tutorial/tsp_sampling_with_openjij_adapter.html)
  - Several APIs are added for converting `ommx.v1.Instance` into QUBO format. Please see the above tutorial.
- Python 3.8 support has been dropped due to its EOL



-------------

### Ommx-1.5.0


[](https://github.com/Jij-Inc/ommx/releases/tag/python-1.5.0)

This notebook describes the new features. Please refer the GitHub release note for the detailed information.

## Evaluation and Partial Evaluation

From the first release of OMMX, `ommx.v1.Instance` supports `evaluate` method to produce `Solution` message


```python
from ommx.v1 import Instance, DecisionVariable

# Create an instance of the OMMX API
x = DecisionVariable.binary(1)
y = DecisionVariable.binary(2)

instance = Instance.from_components(
    decision_variables=[x, y],
    objective=x + y,
    constraints=[x + y <= 1],
    sense=Instance.MINIMIZE
)
solution = instance.evaluate({1: 1, 2: 0})
```


```python
solution.decision_variables
```

From Python SDK 1.5.0, `Function` and its base classes, `Linear`, `Quadratic`, and `Polynomial` also support `evaluate` method:


```python
f = 2*x + 3*y
value, used_ids = f.evaluate({1: 1, 2: 0})
print(f"{value=}, {used_ids=}")
```

This returns evaluated value of the function and used decision variable IDs. If some decision variables are lacking, the `evaluate` method raises an exception:


```python
try:
    f.evaluate({3: 1})
except RuntimeError as e:
    print(e)
```

In addition, there is `partial_evaluate` method


```python
f2, used_ids = f.partial_evaluate({1: 1})
print(f"{f2=}, {used_ids=}")
```

This creates a new function by substituting `x = 1`. `partial_evaluate` is also added to `ommx.v1.Instance` class:


```python
new_instance = instance.partial_evaluate({1: 1})
new_instance.objective
```

This method will be useful for creating a problem with fixing specific decision variables.


