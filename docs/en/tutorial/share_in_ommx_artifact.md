# Exchange a problem and evaluation results in a file

**Add the data to an Artifact, commit it, and save a `.ommx` file.** The receiver can retrieve the problem, evaluation result and analysis note from that file alone.

This example transfers a problem that selects at most one of two items, together with an existing candidate's evaluation. No solver searches for candidates. An `Instance` describes the problem, a `State` assigns variable values, and a `Solution` records evaluation of that candidate against the problem.

## Setup

Install Python SDK `3.0.0b7` using steps 1–2 of [the installation instructions](../getting_started/install.md). No Adapter, Experiment or registry login is needed.

In an empty working directory, save the following two Python files and **run them separately**. Separate sender and receiver storage confirms that the receiver does not depend on sender memory or cached data.

## Sender: save three pieces of data in one file

Save this as `send_artifact.py`.

```python
import os
from pathlib import Path

# この例専用の保存場所。初回に存在しないフォルダを指定する。
registry = Path("artifact-sender-registry").resolve()
registry.mkdir()
os.environ["OMMX_LOCAL_REGISTRY_ROOT"] = str(registry)

from ommx import Instance, State
from ommx.artifact import ArtifactDraft

instance = Instance.maximize()
x = instance.new_binary("x")
y = instance.new_binary("y")
instance.objective = 3 * x + 2 * y
instance.add_constraint(x + y <= 1)

solution = instance.evaluate(State({x.id: 1, y.id: 0}))
assert solution.feasible
assert solution.objective == 3

draft = ArtifactDraft.new_anonymous()
instance_layer = draft.add_instance(instance)
solution_layer = draft.add_solution(solution)
_ = draft.add_json(
    {
        "instance_digest": instance_layer.digest,
        "solution_digest": solution_layer.digest,
        "selected": ["x"],
        "note": "手元の候補を評価した結果。最適性の証明ではない。",
    },
    title="analysis",
)
artifact = draft.commit()
artifact.save("candidate.ommx")
print("candidate.ommxに保存しました")
```

Run it in the environment where you installed the SDK.

```bash
python send_artifact.py
```

Share `candidate.ommx` alone. The `artifact-sender-registry` directory is the sender's **Local Registry**, the SDK's local storage, and need not be transferred.

Methods such as `add_instance()` add data as **layers**, the components of an Artifact. `commit()` finalizes their inventory in the Local Registry, while `save()` writes the exchange file. These are separate operations.

## Receiver: read only the file in a new process

Save the following as `receive_artifact.py` beside `candidate.ommx`. It uses no sender variables or returned objects.

```python
import os
from pathlib import Path

# 空のLocal Registryから始める。送信側とは別の場所を指定する。
registry = Path("artifact-receiver-registry").resolve()
registry.mkdir()
os.environ["OMMX_LOCAL_REGISTRY_ROOT"] = str(registry)

from ommx.artifact import Artifact

received = Artifact.import_archive("candidate.ommx")
instance = received.instance
solution = received.solution

analysis_layer = next(
    layer
    for layer in received.layers
    if layer.annotations.get("org.ommx.user.title") == "analysis"
)
analysis = received.get_json(analysis_layer)

checked = instance.evaluate(solution.state)
assert checked.feasible == solution.feasible
assert checked.objective == solution.objective
print(solution.feasible, solution.objective)
print(analysis["selected"])
```

Run this in a new Python process.

```bash
python receive_artifact.py
```

The output is `True 3.0` and `['x']`. You retrieved the problem, candidate evaluation and JSON analysis note from the file. `import_archive()` persistently imports the contents into the receiver's Local Registry; it does not merely read the archive in place.

This example stores one object of each type, so `.instance` and `.solution` retrieve the first problem and evaluation result. The JSON layer is selected by its saved `title="analysis"`. The note explicitly records the content identifiers of the referenced layers as `instance_digest` and `solution_digest`. With several problems or results, use {meth}`~ommx.artifact.Artifact.get_layer_descriptor` and typed getters to select the required objects.

When repeating the scripts, choose a new working directory or storage paths instead of deleting existing data. A failing `mkdir()` indicates that a previous storage directory already exists.

## Stop here, or continue when needed

You transferred an evaluation of the supplied candidate. Saving, loading and candidate feasibility do not establish optimality. The file also contains none of the source code or environment information that you did not explicitly add.

- Understand storage and names: [Artifact storage and identity](../user_guide/artifact.md).
- Distribute through a registry instead of a file: [Share an Artifact through a registry](../how_to/share_artifact_registry.md).
- Organize conditions and results from several trials: [Experiment Tutorial](experiment_management). Experiment is not needed for this file exchange.

For another language or tool, use [the Artifact specification matching public b7](https://github.com/Jij-Inc/ommx/blob/python-3.0.0b7/rust/ommx/doc/artifact_design.md), not this Python procedure as the format definition.
