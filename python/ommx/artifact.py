from __future__ import annotations

from ._ommx_rust import ArtifactArchive, ArtifactDir, Descriptor
from pathlib import Path


class Artifact:
    """
    Reader class for OMMX Artifacts.

    Examples
    --------

    Load an artifact stored as a single file:

    ```python
    >>> from ommx.artifact import Artifact
    >>> artifact = Artifact.load_archive("data/random_lp_instance.ommx")
    >>> for layer in artifact.layers:
    ...     print(layer.digest)
    sha256:93fdc9fcb8e21b34e3517809a348938d9455e9b9e579548bbf018a514c082df2

    ```

    Load from image name

    ```python
    >>> artifact = Artifact.load("ghcr.io/jij-inc/ommx/random_lp_instance:4303c7f")
    >>> for layer in artifact.layers:
    ...     print(layer.digest)
    sha256:93fdc9fcb8e21b34e3517809a348938d9455e9b9e579548bbf018a514c082df2

    """

    _base: ArtifactArchive | ArtifactDir

    def __init__(self, base: ArtifactArchive | ArtifactDir):
        self._base = base

    @staticmethod
    def load_archive(path: str | Path) -> Artifact:
        """
        Load an artifact stored as a single file
        """
        if isinstance(path, str):
            path = Path(path)

        if path.is_file():
            base = ArtifactArchive.from_oci_archive(str(path))
        elif path.is_dir():
            base = ArtifactDir.from_oci_dir(str(path))
        else:
            raise ValueError("Path must be a file or a directory")

        return Artifact(base)

    @staticmethod
    def load(image_name: str) -> Artifact:
        """
        Load an artifact stored as container image in local or remote registry

        If the image is not found in local registry, it will try to pull from remote registry.
        """
        base = ArtifactDir.from_image_name(image_name)
        return Artifact(base)

    @property
    def layers(self) -> list[Descriptor]:
        return self._base.layers

    def get_blob(self, digest: str | Descriptor) -> bytes:
        if isinstance(digest, Descriptor):
            digest = digest.digest
        return self._base.get_blob(digest)
