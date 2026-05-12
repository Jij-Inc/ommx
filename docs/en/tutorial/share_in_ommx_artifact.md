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

# Sharing Various Types of Data in an OMMX Artifact

In mathematical optimization workflows, it is important to generate and manage a variety of data. Properly handling these data ensures reproducible computational results and allows teams to share information efficiently.

OMMX provides a straightforward and efficient way to manage different data types. Specifically, it defines a data format called an OMMX Artifact, which lets you store, organize, and share various optimization data through the OMMX SDK.

+++

## Preparation: Data to Share

First, let's prepare the data we want to share. We will create an `ommx.v1.Instance` representing the 0-1 knapsack problem and solve it using SCIP. We will also share the results of our optimization analysis. Details are omitted for brevity.

```{code-cell} ipython3
:tags: [hide-input]

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
    # Register all constraints (keys are constraint IDs)
    constraints={0: constraint},
    # Specify that it is a maximization problem
    sense=Instance.MAXIMIZE,
)

# Solve with SCIP
solution = OMMXPySCIPOptAdapter.solve(instance)

# Analyze the optimal solution
df_vars = solution.decision_variables_df()
df = pd.DataFrame.from_dict(
    {
        "Item Number": df_vars.index,
        "Put in Knapsack?": df_vars["value"].apply(lambda x: "Yes" if x == 1.0 else "No"),
    }
)
```

```{list-table}
:header-rows: 1

* - Variable Name
  - Description
* - `instance`
  - `ommx.v1.Instance` object representing the 0-1 knapsack problem
* - `solution`
  - `ommx.v1.Solution` object containing the results of solving the 0-1 knapsack problem with SCIP
* - `data`
  - Input data for the 0-1 knapsack problem
* - `df`
  - `pandas.DataFrame` object representing the optimal solution of the 0-1 knapsack problem
```

+++

## Creating an OMMX Artifact as a File

OMMX Artifacts can be managed as files or by assigning them container-like names. Here, we'll show how to save the data as a file. Using the OMMX SDK, we'll store the data in a new file called `my_instance.ommx`. First, we need an `ArtifactBuilder`.

```{code-cell} ipython3
:tags: [remove-output]

import os
from ommx.artifact import ArtifactBuilder

# Specify the name of the OMMX Artifact file
filename = "my_instance.ommx"

# If the file already exists, remove it
if os.path.exists(filename):
    os.remove(filename)

# 1. Create a builder; v3 publishes every artifact into the SQLite
#    Local Registry, so the builder takes (or synthesizes) an image
#    name. Use `new_anonymous()` if you don't want to invent one.
builder = ArtifactBuilder.new_anonymous()
```

[`ArtifactBuilder`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.ArtifactBuilder) has two primary constructors. v3 always publishes into the SQLite Local Registry, so a build produces a registry entry; if you also want a `.ommx` file for sharing, call `Artifact.save(path)` afterward.

| Constructor | Description |
| --- | --- |
| [`ArtifactBuilder.new`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.ArtifactBuilder.new) | Caller-supplied image name |
| [`ArtifactBuilder.new_anonymous`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.ArtifactBuilder.new_anonymous) | Synthesized name `<registry-id8>.ommx.local/anonymous:<local-timestamp>-<nonce>` for share-and-discard archives |
| [`ArtifactBuilder.for_github`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.ArtifactBuilder.for_github) | Convenience for GitHub Container Registry naming |

`new_anonymous` uses the `.local` mDNS link-local TLD so an accidental push won't leak to a real remote registry. The registry-id prefix is generated once per `LocalRegistry` (a random UUID stored in the registry's SQLite metadata) — anonymous artifacts from the same registry share a prefix, so when an archive is shared you can tell artifacts apart by their source registry. Clean accumulated anonymous entries with `ommx artifact prune-anonymous` (which removes entries from every registry-id prefix, not just the current host's).

**Caveat on the timestamp**: the synthesized tag is the **builder's local time** without a timezone marker. If an anonymous archive is shared with someone in a different timezone, the recipient will read the same digits as their own local time, so the time component is not absolute across machines. Pick an explicit name via `ArtifactBuilder.new(...)` if you need a stable, timezone-unambiguous tag.

Regardless of the initialization method, you can save `ommx.v1.Instance` and other data in the same way. Let's add the data prepared above.

```{code-cell} ipython3
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

```{code-cell} ipython3
desc_json.to_dict()
```

The part added as `title="..."` in `add_json` is saved as an annotation of the layer. OMMX Artifact is a data format for humans, so this is basically information for humans to read. The `ArtifactBuilder.add_*` functions all accept optional keyword arguments and automatically convert them to the `org.ommx.user.` namespace.

Finally, call `build` to publish the artifact into the SQLite Local Registry, then `save` to export it as a `.ommx` file.

```{code-cell} ipython3
# 3. Publish into the local registry
artifact = builder.build()

# 4. Export to a .ommx archive for sharing
artifact.save(filename)
```

Let's check if the file has been created:

```{code-cell} ipython3
import os
print(os.path.exists(filename))
```

Now you can share this `my_instance.ommx` with others using the usual file sharing methods.

+++

## Read OMMX Artifact file

Next, let's read the OMMX Artifact we saved. When loading an OMMX Artifact in archive format, use [`Artifact.load_archive`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.Artifact.load_archive).

```{code-cell} ipython3
from ommx.artifact import Artifact

# Load the OMMX Artifact file locally
artifact = Artifact.load_archive(filename)
```

OMMX Artifacts store data in layers, with a manifest (catalog) that details their contents. You can check the `Descriptor` of each layer, including its Media Type and annotations, without reading the entire archive.

```{code-cell} ipython3
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

```{code-cell} ipython3
artifact.get_json(artifact.layers[3])
```

```{code-cell} ipython3
:tags: [remove-cell]

# Remove the created OMMX Artifact file to clean up
os.remove(filename)
```
