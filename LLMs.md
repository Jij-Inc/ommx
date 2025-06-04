# OMMX Documentation for AI Assistants

# Table of Contents
- [Introduction](#introduction)
- [Switch Language](#switch-language)
  - [日本語](https://jij-inc.github.io/ommx/ja/)
- [AI Assistant](#ai-assistant)
  - [DeepWiki](https://deepwiki.com/Jij-Inc/ommx)
- [Tutorial](#tutorial)
  - [Solve With Ommx Adapter](#solve-with-ommx-adapter)
  - [Tsp Sampling With Openjij Adapter](#tsp-sampling-with-openjij-adapter)
  - [Switching Adapters](#switching-adapters)
  - [Share In Ommx Artifact](#share-in-ommx-artifact)
  - [Download Miplib Instance](#download-miplib-instance)
  - [Implement Adapter](#implement-adapter)
- [User Guide](#user-guide)
  - [Supported Ommx Adapters](#supported-ommx-adapters)
  - [Adapter Initial State](#adapter-initial-state)
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

Here, we will use OSS adapters developed as a part of OMMX Python SDK.
For non-OSS solvers, adapters are also available and can be used with the same interface.
A complete list of supported adapters for each solver can be found in [Supported Adapters](../user_guide/supported_ommx_adapters.ipynb).

Here, let's solve the knapsack problem with OSS solvers, Highs, SCIP.


```python
from ommx_highs_adapter import OMMXHighsAdapter
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter


# List of adapters to use
adapters = {
    "highs": OMMXHighsAdapter,
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

### Supported Ommx Adapters

To solve mathematical optimization problems described in OMMX using solvers, it is necessary to convert them into data structures that conform to the solver's specifications. OMMX Adapters play this conversion role. Since specifications differ for each solver, there exists an adapter for each solver.

## Adapters for OSS solvers/samplers
Several adapters for OSS solvers/samplers are supported in OMMX repository.

| Package name | PyPI | API Reference | Description |
|:--- |:--- |:--- |:--- |
| [ommx-highs-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-highs-adapter) | [](https://pypi.org/project/ommx-highs-adapter/) | [](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx_highs_adapter/index.html) | Adapter for [HiGHS](https://github.com/ERGO-Code/HiGHS)
| [ommx-openjij-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-openjij-adapter) | [](https://pypi.org/project/ommx-openjij-adapter/) | [](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx_openjij_adapter/index.html) | Adapter for [OpenJij](https://github.com/OpenJij/OpenJij)
| [ommx-python-mip-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-python-mip-adapter) | [](https://pypi.org/project/ommx-python-mip-adapter/) | [](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx_python_mip_adapter/index.html)| Adapter for [Python-MIP](https://www.python-mip.com/) |
| [ommx-pyscipopt-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-pyscipopt-adapter) | [](https://pypi.org/project/ommx-pyscipopt-adapter/) | [](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx_pyscipopt_adapter/index.html) | Adapter for [PySCIPOpt](https://github.com/scipopt/PySCIPOpt)

## Adapters for Non-OSS solvers/samplers
Non-OSS solvers/samplers are also supported in other repositories.

| Package name | PyPI | Description |
|:--- |:--- |:--- |
| [ommx-da4-adapter](https://github.com/Jij-Inc/ommx-da4-adapter) | [](https://pypi.org/project/ommx-da4-adapter/) | Adapter for [Fujitsu Digital Annealer(DA4)](https://www.fujitsu.com/jp/digitalannealer/) |
|  [ommx-dwave-adapter](https://github.com/Jij-Inc/ommx-dwave-adapter) | [](https://pypi.org/project/ommx-dwave-adapter) | Adapter for [D-Wave](https://docs.dwavequantum.com/en/latest/index.html) |
| [ommx-fixstars-amplify-adapter](https://github.com/Jij-Inc/ommx-fixstars-amplify-adapter) | [](https://pypi.org/project/ommx-fixstars-amplify-adapter/) | Adapter for [Fixstars Amplify](https://amplify.fixstars.com/ja/docs/amplify/v1/index.html#) |
| [ommx-gurobipy-adapter](https://github.com/Jij-Inc/ommx-gurobipy-adapter) | [](https://pypi.org/project/ommx-gurobipy-adapter/) | Adapter for [Gurobi](https://www.gurobi.com/) |


```python

```



-------------

### Adapter Initial State


Some OMMX Adapters support providing initial solutions when executing optimization calculations.
Here, we'll introduce this feature using OMMXPySCIPOptAdapter as an example. By providing an initial solution, the solver does not need to construct an initial feasible solution by itself, which can sometimes improve the performance of optimization calculations.

## How to Provide an Initial Solution

The initial solution (`initial_state`) that can be provided is of type `ToState`, which can accept both `ommx.v1.State` and `Mapping[int, float]`.

We'll demonstrate how to provide an initial solution using the following instance:


```python
from ommx.v1 import Instance, DecisionVariable
from ommx.v1.solution_pb2 import State

x = DecisionVariable.integer(1, lower=0, upper=5)
y = DecisionVariable.integer(2, lower=0, upper=5)

ommx_instance = Instance.from_components(
    decision_variables=[x, y],
    objective=x - y,
    constraints=[x + y <= 5],
    sense=Instance.MAXIMIZE,
)
```

Example of initial solution using `ommx.v1.State`


```python
initial_state = State(
    entries={
        1: 3.0,
        2: 2.0,
    }
)
```

Example of initial solution using `Mapping[int, float]`


```python
initial_state = {
    1: 3.0,
    2: 2.0,
}
```

As shown below, you can run the optimization with an initial solution by providing `initial_state` as an argument to the solve function:


```python
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

solution = OMMXPySCIPOptAdapter.solve(
    ommx_instance=ommx_instance,
    initial_state=initial_state,
)
```

If you need to tune the solver, you can directly use the OMMXPySCIPOptAdapter class to set solver parameters. In this case, you can also provide `initial_state` as an argument as shown below:


```python
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

adapter = OMMXPySCIPOptAdapter(
    ommx_instance=ommx_instance,
    initial_state=initial_state,
)
```

## Example Comparison With and Without Initial Solution

Using the `roll3000` instance from MIPLIB, let's compare the performance differences with and without an initial solution.
For the initial solution, we'll use a feasible solution (not an optimal one) that was prepared in advance.


```python
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter
from ommx import dataset

# Load the instance
ommx_instance = dataset.miplib2017("roll3000")
```

### Without an initial solution


```python
import time

start = time.perf_counter()
solution = OMMXPySCIPOptAdapter.solve(
    ommx_instance=ommx_instance,
)
duration = time.perf_counter() - start
print(f"Execution Time: {duration}")
```

### With an initial solution

For this tutorial, we'll use the following initial solution obtained in advance:


```python
initial_state = {257: -1.84297022087776e-14, 703: 0.0, 531: -4.6407322429331543e-14, 1052: -9.621932880084689e-15, 360: 0.0, 175: 0.0, 507: 11.000000000000002, 301: 0.0, 937: 3.0000000000000044, 859: 0.0, 564: -3.1086244689504383e-15, 557: -4.6407322429331543e-14, 205: 0.0, 617: 1.3322676295501878e-14, 162: 0.0, 811: 0.0, 759: 3.1086244689503373e-15, 682: 0.0, 987: 0.0, 1013: 0.0, 536: 0.9999999999998973, 189: 0.0, 540: -4.440892098500626e-15, 1135: 0.0, 1163: 0.0, 526: 0.0, 7: -5.362970457884017e-15, 83: 2.0, 606: 0.0, 677: 1.000000000000089, 935: 0.0, 224: 0.0, 824: -5.551115123115685e-17, 161: 0.0, 671: 0.9999999999999867, 1075: 0.0, 1147: 4.440892098500626e-15, 107: -2.6645352591003757e-15, 807: 0.0, 836: 0.0, 1110: 0.0, 1143: 0.0, 84: 0.0, 368: 4.218847493575595e-14, 639: 0.0, 663: 0.0, 171: 0.0, 423: 3.0000000000001226, 730: 0.0, 21: 0.0, 931: -8.881784197001252e-15, 611: 2.842170943040401e-14, 833: 0.0, 435: 4.000000000000141, 410: 0.0, 505: 3.000000000000361, 198: 0.0, 262: 0.0, 76: 0.0, 620: -8.881784197001252e-15, 616: 0.0, 95: -3.887299665209862e-15, 291: 0.0, 748: 2.842170943040401e-14, 822: 1.1435297153639112e-14, 450: 0.0, 61: 0.0, 1003: 3.999999999999979, 845: 2.000000000000089, 124: 0.9999999999999991, 1031: 0.0, 716: 0.0, 948: 5.999999999999996, 821: 0.0, 493: 0.0, 212: 0.0, 34: 0.0, 136: 2.999999999999928, 20: 1.0000000000000009, 701: -8.881784197001252e-15, 839: 0.0, 246: 0.0, 282: 0.0, 919: 0.0, 418: 0.0, 967: 0.0, 1099: 0.0, 458: 0.0, 342: 0.0, 452: 0.0, 637: 0.0, 233: 3.000000000000105, 138: 0.0, 857: 1.000000000000047, 487: -2.6645352591003757e-15, 275: 0.0, 964: 3.0000000000000124, 447: 1.0, 622: 0.0, 877: 0.0, 242: 0.0, 596: 0.0, 854: -1.865174681370263e-14, 153: 1.0000000000000286, 478: 0.0, 679: 2.9999999999999494, 1125: 0.0, 612: 0.0, 975: 0.0, 943: 0.0, 1009: 0.0, 829: 0.0, 400: 0.0, 321: 0.0, 1054: 1.865174681370263e-14, 1159: 1.9999999999999867, 471: -3.1086244689504383e-15, 797: -2.7533531010703882e-14, 1074: -4.3520742565306136e-14, 714: -3.019806626980426e-14, 1162: 0.0, 299: 0.0, 645: 0.0, 71: 0.0, 858: 5.000000000000172, 306: 1.0, 861: 1.84297022087776e-14, 172: 0.0, 1028: 8.881784197001252e-16, 808: 0.0, 1114: 3.9999999999999725, 134: 4.440892098500626e-14, 646: 0.0, 1080: 10.0, 393: -2.4868995751603507e-14, 55: 0.0, 110: 0.0, 949: 3.0531133177191805e-15, 373: 0.0, 456: 0.0, 856: -4.440892098500625e-16, 708: 0.0, 430: 0.0, 560: 0.0, 1017: 2.930988785010413e-14, 243: 1.887379141862766e-14, 971: 2.0000000000000666, 402: 0.9999999999999574, 37: 0.0, 324: 1.000000000000021, 1030: 0.0, 984: 0.0, 749: 8.881784197001252e-15, 250: 0.0, 472: 0.0, 177: 1.0000000000000315, 82: 7.105427357601002e-15, 333: 0.0, 1096: -1.0725940915768033e-14, 372: 4.884981308350689e-14, 140: 0.0, 652: 0.0, 60: 0.0, 316: 0.0, 785: 0.0, 432: 0.0, 401: 0.0, 513: 0.0, 309: 0.0, 633: 1.9999999999999853, 951: 0.0, 481: 3.7192471324942744e-14, 240: 1.000000000000007, 599: 0.0, 685: 0.0, 610: 0.0, 690: 0.9999999999999996, 500: 0.0, 252: 0.0, 603: 0.0, 853: 1.0000000000000089, 519: 0.0, 827: 2.0, 823: 7.000000000000012, 602: 0.0, 578: 0.0, 1064: -4.3520742565306136e-14, 215: 8.881784197001252e-16, 85: 0.0, 583: 0.0, 650: -8.926193117986259e-14, 517: 0.0, 412: 0.0, 609: 0.0, 1133: 1.9999999999999196, 1079: 0.0, 576: 0.0, 803: 0.0, 832: 0.0, 1066: 4.1744385725905886e-14, 694: 1.0, 408: 0.0, 315: 0.0, 783: 0.0, 1127: 0.0, 998: 0.0, 462: 1.0, 1034: 0.0, 678: 1.687538997430238e-14, 454: 0.0, 511: 0.0, 925: 0.0, 707: -3.647082635893639e-14, 411: 0.0, 618: 0.0, 46: 1.000000000000011, 891: 0.0, 740: 0.0, 121: 0.0, 2: 0.0, 459: 0.9999999999999853, 774: 0.0, 339: 1.0000000000000115, 334: 0.0, 669: -7.993605777301127e-15, 255: 0.0, 390: 0.0, 587: 1.000000000000007, 835: 0.0, 133: -1.7985612998927536e-14, 396: 0.0, 341: 0.0, 382: 2.999999999999912, 428: 0.0, 761: 2.220446049250313e-15, 99: 0.0, 156: 0.0, 589: 198.99999999999935, 621: 1.0000000000000009, 510: -9.386431026376354e-16, 687: 0.0, 1156: 0.0, 915: 2.842170943040401e-14, 1051: 0.0, 328: 0.0, 882: 0.0, 258: -1.84297022087776e-14, 1158: 0.0, 995: 1.9999999999999565, 636: -9.175151694187626e-17, 966: -3.1086244689504383e-15, 199: 0.0, 781: 0.0, 754: 0.0, 249: -2.6645352591003757e-15, 969: 0.0, 985: 0.0, 1055: 0.0, 961: 0.0, 141: 2217.9999999999973, 641: 0.0, 607: 0.0, 973: -3.887299665209862e-15, 630: 2.930988785010413e-14, 444: 1.0, 689: 0.0, 89: 0.0, 41: 0.0, 126: 0.0, 69: 0.0, 343: -5.362970457884017e-15, 1021: 0.0, 466: 1.0, 217: -8.881784197001252e-16, 902: 0.0, 604: 67.00000000000283, 539: 1.0047518372857667e-13, 876: 0.0, 534: 0.0, 414: 0.0, 720: 0.0, 911: 0.0, 1070: 0.0, 23: 0.0, 798: 0.0, 1142: -3.1086244689504383e-15, 791: 0.0, 119: 0.0, 615: 1.0, 795: 1.0, 417: 0.0, 59: -9.2148511043888e-15, 755: 1.0000000000000009, 286: 1.9999999999999485, 760: 0.0, 194: 2.0000000000001297, 139: 0.0, 533: 1.0, 608: 0.0, 463: 1.0000000000000364, 965: 5.000000000000172, 665: 0.0, 886: 0.0, 1145: 0.0, 326: 0.0, 453: 0.0, 1104: -1.5765166949677223e-14, 844: 0.0, 903: 0.0, 144: 0.0, 1023: 0.0, 775: 0.0, 884: 0.0, 739: 0.0, 776: 0.0, 308: -8.881784197001252e-15, 851: 4.218847493575595e-15, 294: 0.0, 247: 0.0, 1062: 1.0000000000000364, 125: 1.0000000000000102, 446: 0.0, 366: 0.0, 64: 0.0, 268: 0.0, 757: 0.0, 168: 0.0, 303: 0.0, 688: -2.6645352591003757e-15, 490: 0.0, 590: 0.0, 195: -3.1086244689504383e-15, 506: 1.3322676295501878e-14, 152: 0.0, 380: 1.0, 421: 1.0, 216: 0.0, 327: 0.0, 1015: 0.0, 312: 0.0, 1032: 0.0, 743: 1.0, 1040: 0.0, 176: 0.0, 376: 0.0, 314: 0.0, 165: 0.0, 159: 0.0, 1071: 1.0, 67: 0.0, 1121: -8.722144284963535e-15, 386: 0.9999999999999867, 696: 1.0000000000000004, 538: 0.0, 214: 0.0, 710: 0.0, 515: 0.0, 1082: -1.1102230246251565e-16, 979: 0.0, 94: -5.2004562015845005e-15, 527: 0.0, 102: 0.0, 982: 0.0, 100: 3.0000000000000124, 123: 0.0, 483: 0.0, 167: 1.0, 537: 0.0, 686: 0.0, 280: 3.9999999999999956, 293: 0.0, 672: 151.9999999999999, 191: 0.0, 896: 0.0, 997: 0.0, 92: 0.0, 873: 0.0, 237: 1.0000000000000102, 26: 0.0, 732: -6.217248937900877e-15, 469: 1.0, 959: 0.0, 485: 3.000000000000033, 974: 0.0, 799: 0.0, 750: 0.0, 43: 1.0, 711: 161.9999999999928, 548: 0.0, 362: 0.0, 787: 0.0, 244: 0.0, 24: 0.0, 96: 1.0000000000000284, 588: 0.0, 908: 0.0, 626: 0.0, 762: 0.9999999999999916, 234: 0.0, 1004: 0.0, 489: 0.0, 379: 2.9999999999998863, 651: 0.0, 623: 1.4210854715202004e-14, 1025: 1.0, 488: 0.0, 940: -2.6513460148478883e-14, 461: 0.0, 862: 0.0, 529: 3.197442310920451e-14, 8: 0.0, 950: 1.0000000000000158, 544: 0.0, 499: 0.0, 27: 29.99999999999986, 613: 0.9999999999998973, 521: 0.0, 993: 0.0, 784: -1.4223963249031613e-16, 10: 0.0, 477: -1.3322676295501877e-15, 747: 9.947598300641403e-14, 929: 0.0, 1018: 0.0, 986: 0.0, 562: 0.0, 185: 0.0, 594: 0.0, 726: 0.0, 129: -2.752545508256288e-16, 1024: 1.0000000000001048, 356: 0.9999999999999707, 201: 0.0, 455: 0.0, 436: 0.9999999999999707, 14: -8.548717289613705e-15, 354: 12889.999999999976, 498: 0.0, 860: 0.0, 643: 0.0, 12: -2.6645352591003757e-15, 468: 32.000000000000014, 605: 1.0000000000000278, 190: 0.0, 359: 0.0, 614: 0.0, 289: 0.0, 1022: 0.0, 434: 0.0, 479: -1.5765166949677223e-14, 572: 1.3322676295501878e-14, 1014: 0.0, 114: 0.0, 737: 0.0, 348: 2.1316282072803006e-14, 265: 0.0, 741: -5.400124791776761e-13, 169: 0.0, 825: 0.0, 1049: 4.440892098500626e-15, 561: 0.0, 1044: 0.0, 358: 0.9999999999999831, 1085: 2.999999999999983, 920: -8.881784197002262e-16, 281: 0.0, 1043: 0.0, 654: 0.0, 901: -1.6241245636507983e-14, 648: 2.9999999999999414, 397: -4.440892098500626e-15, 907: 197.00000000000014, 1124: 0.0, 1063: 1.0, 697: 0.0, 1086: 4.000000000000141, 728: -4.2549297418759124e-14, 403: 0.0, 6: 0.0, 913: 0.0, 228: 1.0047518372857667e-13, 442: 8.881784197001252e-16, 994: 0.0, 208: 0.0, 815: 0.0, 698: 0.0, 518: 0.0, 197: -1.021405182655144e-14, 934: 2.999999999999903, 591: -1.0725940915768033e-14, 439: 0.0, 443: 0.0, 1047: 0.0, 894: -3.7192471324942744e-15, 184: 0.0, 1128: 0.0, 70: 0.0, 492: 0.0, 782: 1.2156942119645464e-14, 516: 0.0, 1151: 0.0, 1097: 95.0, 101: 1.7763568394002505e-14, 976: 0.0, 352: 0.0, 151: 4.3115457266996746e-17, 270: 0.0, 1073: -3.4181831865424275e-16, 751: 0.0, 864: 0.0, 351: 0.0, 939: 0.0, 433: 0.0, 756: -4.440892098500627e-16, 30: 0.0, 166: 1.2156942119645464e-14, 955: 0.0, 241: 0.9999999999999999, 1132: 0.0, 658: 0.0, 1036: 0.0, 887: 279.9999999999998, 866: 4.063416270128073e-14, 311: 0.0, 655: 0.0, 753: 2.0, 35: 0.0, 962: 0.0, 385: -1.1435297153639112e-14, 335: 0.9999999999999987, 1057: 0.0, 674: 1.0000000000000884, 388: 1.0, 921: 1.9999999999999498, 264: 0.0, 972: 8.881784197001252e-15, 75: -8.881784197002262e-16, 15: 0.0, 344: 0.0, 528: 0.0, 780: 0.0, 1134: 0.0, 530: 0.0, 44: 0.0, 135: 2.968534766042603e-14, 771: 0.0, 649: 0.0, 1093: 0.0, 220: 0.0, 898: -3.647082635893639e-14, 305: 0.0, 1140: 0.0, 395: 0.0, 870: -3.019806626980426e-14, 905: 1.0000000000000435, 1008: 0.9999999999999813, 837: -5.2004562015845005e-15, 1011: 1.865174681370263e-14, 63: 0.0, 1006: 0.0, 673: 1.4210854715202004e-14, 960: 0.0, 627: 0.0, 378: 0.0, 399: 0.0, 132: 0.0, 475: 0.0, 319: 0.0, 735: 2.6513460148478883e-14, 4: 0.0, 419: 0.0, 1077: 3.9999999999999485, 582: 1.000000000000011, 550: 0.0, 881: 9.769962616701378e-15, 371: 1.0000000000000469, 676: 0.0, 553: 1.0, 229: 0.0, 155: 0.0, 1060: 0.0, 988: 0.0, 644: 7.993605777301127e-15, 357: 0.0, 638: 0.0, 56: -7.105427357601002e-15, 802: 0.0, 834: 0.0, 1045: 8.881784197001252e-16, 3: 0.0, 496: 0.0, 290: 0.0, 922: 0.0, 186: 0.0, 868: 0.0, 457: 0.0, 480: 0.0, 248: 0.9999999999999981, 1105: 0.0, 661: 2.220446049250313e-14, 106: 0.0, 1103: -8.881784197001252e-15, 1026: 0.0, 266: 1.0000000000000007, 424: 0.0, 956: 0.0, 778: 0.0, 668: 0.0, 325: 0.0, 692: 1.0047518372857667e-13, 952: 8.000000000000055, 323: 1.865174681370263e-14, 695: 0.9999999999999929, 942: 0.0, 374: 2.220446049250313e-15, 188: 0.0, 1107: 0.0, 712: 0.0, 767: 0.0, 150: -2.0261570199409107e-14, 938: 0.9999999999999991, 187: -1.3377022877126888e-14, 543: 0.0, 1102: 1.0000000000000018, 300: 0.0, 916: 1.0000000000000302, 330: -2.6645352591003757e-14, 36: 0.0, 1078: 0.0, 532: 1.0, 888: 30.000000000000682, 1067: 0.0, 347: 0.0, 429: 0.9999999999999999, 448: 0.0, 437: 0.0, 1095: 0.0, 284: 1.0, 431: 0.0, 9: 1.3322676295501878e-14, 725: 1.000000000000089, 1098: 0.0, 451: 1.9999999999999565, 81: 0.0, 667: 1.000000000000011, 570: -8.881784197001252e-16, 779: 9.202150330924782e-17, 54: 0.0, 22: 0.0, 404: 0.0, 235: 0.0, 542: 0.0, 118: 0.0, 805: 0.0, 398: 0.0, 878: 0.0, 226: -1.3377022877126888e-14, 78: 3.999999999999904, 909: -7.993605777301127e-15, 1012: 0.0, 814: 0.0, 524: 3.0000000000000115, 238: 0.0, 566: -2.6645352591003757e-15, 554: 0.0, 675: 0.0, 923: 0.0, 874: 0.0, 559: 0.0, 1154: 0.0, 1089: 1.7763568394002505e-15, 551: 0.0, 391: 0.0, 883: 0.0, 203: 0.0, 285: 1.999999999999993, 269: 0.0, 792: 1.0, 659: 0.0, 160: 2.000000000000005, 279: 0.0, 148: 2.0000000000000187, 702: 7.105427357601002e-15, 80: 0.0, 555: 0.0, 11: 0.0, 958: 0.0, 473: 0.0, 182: 0.0, 1138: 0.0, 427: 0.0, 231: 3.0000000000000213, 926: 0.0, 895: 0.0, 968: 0.0, 954: 0.0, 389: 0.0, 223: -5.684341886080802e-14, 329: 1.0, 800: 3.730349362740526e-14, 164: 0.0, 1058: 0.0, 1029: 0.0, 773: 0.0, 790: 0.0, 58: 0.0, 597: 0.0, 547: 0.0, 812: 0.0, 1002: -8.750737653510993e-14, 340: 4.999999999999949, 1094: 0.9999999999999973, 625: 2.9999999999999893, 804: 0.9999999999999996, 1088: 1.9999999999999885, 345: 1.000000000000089, 717: 1.000000000000031, 558: 0.0, 304: 0.9999999999999901, 867: 8.000000000000055, 253: 33.00000000000008, 765: 0.0, 1117: 0.0, 927: 2.9999999999999627, 202: 0.0, 1007: 0.0, 1108: 0.0, 355: 0.0, 1150: 0.0, 917: 4.999999999999981, 387: 8.43769498715119e-15, 632: 0.0, 192: 0.0, 1139: 0.0, 1160: 1.9999999999999898, 1141: 0.0, 843: 0.0, 48: 2.999999999999967, 830: 137.99999999999926, 1122: 0.0, 245: -1.0502709812953981e-13, 120: 0.0, 349: 0.0, 17: 9.2148511043888e-15, 642: 0.0, 200: 1.0, 912: 2.0000000000000187, 699: 238.00000000000003, 1129: 0.0, 724: 0.0, 992: 3.064215547965432e-14, 112: 0.0, 416: 187.00000000000009, 684: 0.0, 706: 0.0, 174: 4.000000000000132, 546: 0.0, 97: 0.0, 465: 2.3426113579956395e-16, 298: 0.0, 445: 0.9999999999999902, 733: 1.000000000000089, 1090: 0.0, 86: 0.0, 422: 0.0, 236: 0.0, 318: 8.43769498715119e-15, 232: 0.0, 31: 0.0, 273: 0.0, 810: 0.0, 764: 0.0, 1119: 0.9999999999999971, 1048: 0.0, 872: 15.000000000000012, 818: 0.0, 1101: 0.0, 653: 1.0, 384: 0.0, 297: 0.0, 777: -1.7985612998927536e-14, 145: -2.0261570199409107e-14, 127: -8.881784197001252e-16, 79: 0.0, 953: 0.0, 73: 0.0, 90: -8.881784197001252e-15, 363: 0.0, 502: 0.0, 227: -8.548717289613705e-15, 525: 2601.999999999994, 251: 0.0, 98: 2.842170943040401e-14, 211: 0.0, 13: 0.0, 470: 1.9999999999999947, 108: 0.0, 1136: 0.0, 310: 0.0, 426: 6.999999999999999, 892: 1.0000000000000941, 440: 0.0, 841: -3.7192471324942744e-14, 850: 0.0, 552: 0.0, 584: 1.0, 367: 0.0, 629: 0.0, 494: 0.0, 889: 0.0, 464: 9.325873406851315e-14, 259: 0.0, 375: 0.0, 353: 0.0, 598: 1.0000000000000213, 631: 3.000000000000022, 577: 0.0, 704: 0.0, 93: 0.0, 746: 0.0, 565: -2.6645352591003757e-15, 963: 0.0, 1050: 1.0, 1083: 0.0, 292: 1.000000000000023, 332: 0.0, 1100: 0.0, 693: 0.0, 990: 0.0, 512: 0.0, 601: 2.0000000000000355, 713: -8.903988657493755e-14, 369: 0.0, 0: 8.881784197001252e-16, 1081: 1.3322676295501878e-14, 541: 1.0, 196: 0.0, 681: 0.0, 213: 0.0, 467: 0.0, 474: -8.926193117986259e-14, 50: 0.9999999999999574, 1152: -1.674216321134736e-13, 1137: 1.000000000000105, 885: 2.999999999999928, 786: 1.0000000000000007, 817: 12889.999999999976, 130: 0.0, 137: 1.0, 53: 1.0000000000000315, 495: 0.0, 441: 0.0, 571: 1.865174681370263e-14, 1115: 8.881784197001252e-16, 809: 0.0, 819: -9.769962616701378e-15, 1165: 0.0, 364: 0.0, 261: 74.00000000000007, 535: 0.0, 848: 0.0, 731: 0.0, 945: 0.0, 624: 5.000000000000172, 1106: 0.0, 772: 1.000000000000007, 460: 0.0, 727: 0.0, 1046: 0.0, 394: 0.0, 267: -2.7411935155625904e-15, 842: 0.0, 49: 4.999999999999921, 930: 1.999999999999993, 670: 0.0, 928: 0.0, 924: 0.0, 691: 0.0, 146: 0.0, 25: 0.0, 491: 0.0, 142: 9.947598300641403e-14, 849: 0.0, 932: 0.9999999999999867, 33: 1.0, 1010: 0.9999999999999005, 409: 7.105427357601002e-15, 350: 0.0, 207: 2.0000000000000187, 1084: 0.0, 918: -3.7192471324942744e-15, 1112: 0.0, 1016: 0.0, 1053: 0.0, 508: 1.0047518372857667e-13, 482: 0.0, 944: 0.0, 789: 0.0, 1076: 2.9999999999999813, 52: 1.4210854715202004e-14, 680: -7.438494264988549e-15, 320: -8.548717289613705e-15, 449: -2.4868995751603507e-14, 1037: 0.0, 317: 1.000000000000028, 855: 0.9999999999999973, 277: 0.0, 758: 4.440892098500626e-15, 91: 0.0, 222: 0.0, 29: 0.0, 567: 0.0, 438: -6.078471059822732e-15, 549: 0.9999999999999707, 763: 0.0, 991: 4.440892098500626e-15, 628: 0.0, 1068: 0.0, 936: 0.0, 178: 1.0000000000000002, 256: 0.0, 831: 0.0, 183: -8.881784197001252e-16, 1164: 0.0, 946: 0.0, 580: 0.0, 838: 0.0, 19: 0.0, 18: 3.00000000000004, 88: 0.9999999999999999, 420: -3.552713678800501e-15, 377: 0.0, 425: 4.00000000000001, 662: 1.6105119292618043e-14, 806: 0.0, 370: 0.0, 87: -2.930988785010413e-14, 287: 0.0, 579: 0.0, 154: 0.0, 794: 0.0, 501: 7.000000000000055, 852: 1.0000000000000009, 744: 0.0, 1120: 0.0, 406: 0.9999999999999947, 1157: 0.0, 556: 0.0, 103: 1.000000000000018, 1161: 0.0, 666: 0.0, 705: 0.0, 592: 0.0, 47: 0.0, 128: 144.00000000000003, 977: -8.881784197001252e-15, 57: -1.0658141036401503e-14, 503: 0.0, 1144: 0.0, 900: 0.0, 647: 0.0, 68: 0.0, 595: 0.0, 575: 0.0, 415: 0.0, 288: 0.0, 39: 2.9999999999999956, 381: 0.0, 906: 0.0, 846: 1.0, 218: 1.0, 42: -7.993605777301127e-15, 514: 4.440892098500626e-14, 1091: 337.00000000000006, 664: 0.0, 523: 1.0000000000000226, 193: 4.440892098500626e-15, 74: 0.9999999999998995, 263: 0.0, 1027: 0.0, 593: 0.0, 170: 0.0, 1000: -1.2434497875801753e-14, 338: 1.0000000000000115, 1126: 2.6645352591003757e-15, 113: 0.0, 683: 0.0, 715: 0.0, 486: 0.0, 407: 0.0, 66: 0.0, 1148: 0.0, 914: 0.0, 718: -1.3322676295501877e-15, 115: 0.0, 1155: 0.0, 230: 0.0, 826: 0.0, 278: 1.0000000000000286, 999: 0.0, 947: 0.0, 1118: 0.0, 828: -2.930988785010413e-14, 634: 0.0, 1123: 0.0, 721: 0.0, 117: 0.0, 1116: 0.0, 719: -7.993605777301127e-15, 392: 0.0, 405: 2.0000000000000187, 880: 1.000000000000089, 520: 2.784308972829587e-15, 210: 0.0, 1061: 1.0, 793: 0.0, 504: 0.0, 1072: 0.0, 989: 0.0, 813: 0.0, 1042: 1.9999999999999885, 752: 8.881784197001252e-16, 700: 0.0, 1041: 2.0000000000000204, 413: 1.3322676295501878e-14, 574: 3.0000000000000018, 736: 0.0, 1113: 0.0, 904: 0.0, 173: 0.0, 1056: 5.000000000000033, 295: 1.0000000000000278, 619: 2.0, 5: 0.9999999999999951, 497: 0.0, 635: 1.0, 1065: 0.0, 788: 0.0, 742: 1.0, 1039: 0.9999999999999427, 522: 2.220446049250313e-16, 1038: 0.0, 180: 1.0000000000000469, 729: 0.9999999999999628, 869: 0.0, 820: 1.9999999999999982, 219: 0.0, 981: 8.881784197001252e-16, 1020: 1.000000000000089, 147: 0.0, 933: 0.0, 723: 10.000000000000068, 879: 212.00000000000003, 149: 0.0, 509: 1.0, 72: 0.0, 111: 0.0, 897: 3.7192471324942744e-14, 163: 0.0, 978: 1.000000000000047, 1109: 0.0, 545: 0.9999999999999853, 143: 0.9999999999999947, 337: 0.9999999999999005, 302: 1.999999999999993, 1149: 0.0, 840: 0.0, 283: 2.0000000000000204, 271: 0.0, 1019: 0.0, 769: 1.7763568394002505e-15, 16: 0.9999999999999707, 893: -4.6851411639181606e-14, 1092: 0.0, 331: 0.0, 865: 1.3322676295501878e-14, 1001: 2.0, 660: 0.0, 1111: 1.0, 204: 1.0000000000000187, 383: 0.0, 105: 10.000000000000002, 77: 0.0, 734: 1.0, 45: 0.0, 209: 1.0000000000000382, 657: 0.0, 131: 0.0, 1131: -8.881784197001252e-15, 709: 8.881784197001252e-15, 1: 0.0, 957: -1.84297022087776e-14, 656: 0.0, 476: 0.9999999999999694, 104: 0.0, 296: 0.0, 980: 0.0, 62: 0.0, 1130: 0.0, 847: 5.000000000000135, 1035: 0.0, 1146: 0.0, 361: 3.0000000000000124, 768: 0.0, 157: 0.0, 875: 0.0, 890: 0.0, 109: 3.0000000000000044, 585: 1.0, 28: 0.0, 796: 0.0, 365: 0.0, 983: 7.993605777301127e-15, 586: 0.0, 158: 0.0, 600: 2.0000000000000187, 910: 0.0, 179: 0.0, 65: 0.0, 581: 5.000000000000054, 941: 0.0, 722: -3.785860513971784e-14, 1069: 0.0, 1059: 0.0, 346: 0.0, 181: 1.0000000000000364, 307: -1.865174681370263e-14, 568: 0.0, 38: 1.0, 970: 0.0, 40: 4.440892098500626e-15, 738: 0.0, 276: 0.0, 274: 0.0, 871: 0.0, 272: 1.0, 260: 0.0, 801: 0.0, 239: 1.9999999999998908, 322: 0.0, 206: 0.0, 116: 0.0, 766: 0.0, 816: 0.0, 225: 0.9999999999998995, 770: -6.022959908591474e-15, 563: 0.0, 336: 0.0, 573: -9.386431026376354e-16, 254: 0.0, 122: 0.0, 899: -4.440892098500627e-16, 1087: 0.0, 745: 0.0, 640: 0.0, 313: -4.263256414560601e-14, 1153: 0.0, 996: 0.9999999999999498, 51: 0.0, 1033: 0.0, 863: 0.0, 32: 1.000000000000003, 484: 1.000000000000028, 1005: -1.021405182655144e-14, 569: 9.473903143468002e-15, 221: 0.0}
```

Execute a solve operation by providing the `initial_state`:


```python
start = time.perf_counter()
solution = OMMXPySCIPOptAdapter.solve(
    ommx_instance=ommx_instance,
    initial_state=initial_state,
)
duration = time.perf_counter() - start
print(f"Execution Time: {duration}")
```

In this example, providing an initial solution reduced the optimization calculation time.
Note that providing an initial solution does not always improve performance - it may have no effect or even make performance worse in some cases. Trial and error is required to determine if it helps, but in effective cases, significant performance improvement can be expected.



-------------

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
x = \sum_{i=0}^{m-2} 2^i b_i + (u - l - 2^{m-1} + 1) b_{m-1} + l
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
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

p = [10, 13, 18, 32, 7, 15]
w = [11, 15, 20, 35, 10, 33]
x = [DecisionVariable.binary(i) for i in range(6)]
instance = Instance.from_components(
    decision_variables=x,
    objective=sum(p[i] * x[i] for i in range(6)),
    constraints=[sum(w[i] * x[i] for i in range(6)) <= 47],
    sense=Instance.MAXIMIZE,
)

solution = OMMXPySCIPOptAdapter.solve(instance)
solution.objective
```

With the new update, the process looks the same as the above when using the `OMMXPythonMIPAdapter` class instead.

To replace the usage of `instance_to_model()` functions, you can instantiating an adapter and using `solver_input`. You can then apply any solver-specific parameters before optimizing manually, then calling `decode()` to obtain the OMMX solution.


```python
adapter = OMMXPySCIPOptAdapter(instance)
model = adapter.solver_input # in OMMXPySCIPOptAdapter's case, this is a `pyscipopt.Model` object
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


