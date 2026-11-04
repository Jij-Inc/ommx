Attach an existing filesystem file in the experiment space.

File bytes are streamed into the Local Registry immediately. If `media_type`
is omitted, OMMX infers it from the contents and falls back to
`application/octet-stream`. Only a basename for export is stored, not the source
path. `filename` overrides that basename. `compression="zstd"` compresses the
stored layer transparently.

This example records common input data and a file at Experiment scope, and a
candidate evaluation at Run scope. It then retrieves the typed attachments and
exports the original file bytes. Changing the source file after logging does
not alter the stored attachment.

```pycon
>>> from pathlib import Path
>>> from tempfile import TemporaryDirectory
>>> from ommx import Instance
>>> from ommx.experiment import Experiment
>>> problem = Instance.minimize()
>>> x = problem.new_binary("x")
>>> problem.objective = x
>>> with TemporaryDirectory() as directory:
...     source = Path(directory) / "notes.txt"
...     _ = source.write_bytes(b"candidate evaluated locally\n")
...     with Experiment.with_temp_local_registry() as experiment:
...         experiment.log_json("source-data", {"description": "one binary variable"})
...         experiment.log_instance("problem", problem)
...         experiment.log_file(
...             "notes", source, media_type="text/plain", compression="zstd"
...         )
...         with experiment.run() as run:
...             run.log_solution("evaluation", problem.evaluate({x.id: 0}))
...     _ = source.write_bytes(b"changed later\n")
...     assert experiment.get_blob("notes") == b"candidate evaluated locally\n"
...     output = Path(directory) / "restored-notes.txt"
...     _ = experiment.write_attachment("notes", output)
...     assert output.read_bytes() == b"candidate evaluated locally\n"
>>> sorted(experiment.attachment_names)
['notes', 'problem', 'source-data']
>>> experiment.get_json("source-data")
{'description': 'one binary variable'}
>>> saved = experiment.runs[0].get_solution("evaluation")
>>> experiment.get_instance("problem").evaluate(saved.state).objective == saved.objective
True

```

`write_attachment` rejects an existing file unless `overwrite=True` is explicit.
Use {meth}`get_blob` for a binary-file consumer, for example
`io.BytesIO(experiment.get_blob("notes"))`. Typed getters reject mismatched media
types; {meth}`get_attachment` uses the media type to choose a standard decoder.
The {class}`Experiment` example shows saving and reopening all these records
without the original Python objects.
