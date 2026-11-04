# Replace an archive builder and reopen its output

```{index} ArtifactBuilder.new_archive (v2), ArtifactBuilder.new_archive_unnamed (v2), Artifact.load_archive (v2)
```

A v2 archive builder wrote a file when `build()` was called. In v3, `commit()` writes to the local registry and `save()` exports a file. This example creates an anonymous registry entry and uses a fresh directory for the archive.

```python
from pathlib import Path
from tempfile import TemporaryDirectory

from ommx import Instance
from ommx.artifact import Artifact, ArtifactDraft

model = Instance.minimize()
model.title = "Archive migration"
draft = ArtifactDraft.new_anonymous()
draft.add_instance(model)
artifact = draft.commit()
with TemporaryDirectory() as directory:
    path = Path(directory) / "model.ommx"
    artifact.save(path)
    manifest = Artifact.inspect_archive(path)
    assert manifest.image_name == artifact.image_name
    restored = Artifact.import_archive(path).get_instance()
    assert restored.title == model.title
```

For an existing unnamed v2 archive without an image-name annotation, `import_archive` assigns an anonymous local name. Read the returned Artifact's `image_name` if a later step needs to load it by name. Choose a fresh output path when exporting: `save` refuses to overwrite an existing file.

## Read files from another SDK

Use {meth}`~ommx.artifact.Artifact.inspect_archive` to inventory an archive without writing to the local registry. The manifest describes layers, but reading a model from a layer requires {meth}`~ommx.artifact.Artifact.import_archive`. For old archives with annotations on layer descriptors, protobuf metadata takes precedence if both sources specify the same OMMX key. Check titles and application annotations when migrating stored experiments.
