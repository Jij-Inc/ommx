# Attach data to an Experiment

Attach input JSON, models, evaluation results, logs or images to an Experiment or an individual Run. Start with typed methods or file attachments; use a custom Codec only when necessary.

Use `ommx==3.0.0b7` from steps 1–2 of [Install the SDK](../getting_started/install.md). This example needs no Adapter or solver.

(experiment-management-attachable-data-formats)=
## Save standard data and a file

The example creates a small input file, saves a shared problem, JSON and file, and attaches an evaluation result to a Run. It creates or updates `attachment-example/notes.txt`.

```python
from pathlib import Path

from ommx import Instance
from ommx.experiment import Experiment

data_dir = Path("attachment-example")
data_dir.mkdir(exist_ok=True)
input_file = data_dir / "notes.txt"
input_file.write_text("candidate evaluated locally\n", encoding="utf-8")

instance = Instance.minimize()
x = instance.new_binary("x")
instance.objective = x

with Experiment() as experiment:
    experiment.log_json("source-data", {"description": "one binary variable"})
    experiment.log_instance("problem", instance)
    experiment.log_file("notes", input_file, media_type="text/plain")
    with experiment.run() as run:
        run.log_solution("evaluation", instance.evaluate({x.id: 0}))

experiment.save(data_dir / "attachments.ommx")
```

`log_file` copies the file's current contents, rather than retaining a link to it. Later edits to the original file do not change the saved attachment. `save` does not overwrite an existing archive; when rerunning, choose another output filename and update the subsequent import path too.

## Retrieve attachments by name

Use a typed getter when you know the expected type. A mismatched type raises an error instead of silently converting to another type.

```python
from ommx.experiment import Experiment

loaded = Experiment.import_archive("attachment-example/attachments.ommx")
print(sorted(loaded.attachment_names))
source_data = loaded.get_json("source-data")
problem = loaded.get_instance("problem")
evaluation = loaded.runs[0].get_solution("evaluation")
assert source_data["description"] == "one binary variable"
assert problem.evaluate(evaluation.state).objective == evaluation.objective
assert loaded.get_blob("notes") == b"candidate evaluated locally\n"
```

`get_attachment(name)` selects a standard type using the Media Type, which identifies the kind of saved data. Use `get_blob(name)` for bytes. To restore an attachment as a file, supply a destination path.

```python
from pathlib import Path

destination = Path("attachment-example/restored-notes.txt")
loaded.write_attachment("notes", destination, overwrite=True)
assert destination.read_text(encoding="utf-8") == "candidate evaluated locally\n"
```

Here `overwrite=True` updates that destination file so the example can be repeated. Omit it and choose a new path if you do not want to replace an existing file.

## Choose the scope and record needed information explicitly

Put inputs shared by all Runs at Experiment scope, and trial-specific data at Run scope. An attachment name identifies data within that scope: the same name in different Runs refers to different attachments.

Use `run.log_parameter` for scalar values that should become comparison-table columns. Numbers inside attachment JSON, and options passed to an Adapter, are not automatically expanded into Run parameters.

Large JSON or file attachments can use `compression="zstd"`. Getters return the decompressed content, so readback is unchanged. See {py:class}`~ommx.experiment.Experiment` and {py:class}`~ommx.experiment.Run` for supported methods and exact limits.

## Use a Codec only for custom types

For a Python object not covered by standard types or files, pass a Codec providing `media_type`, `encode(value) -> bytes` and `decode(bytes) -> value` to `log_with_codec` / `get_with_codec`. Concrete Codecs should normally come from the package that owns the payload type. Readers also need the appropriate package and Codec.

OMMX does not automatically define an external type's meaning or version compatibility. Check the owner's public Codec or serialization specification. See {py:meth}`~ommx.experiment.Run.log_with_codec` and {py:meth}`~ommx.experiment.SealedRun.get_with_codec` for the calling protocol and Media Type validation.

[Experiment records and lifecycle](../user_guide/experiment.md) explains finalizing and continuing records. [Artifact storage and identity](../user_guide/artifact.md) covers data exchange without a trial hierarchy.
