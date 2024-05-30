from __future__ import annotations

from ._ommx_rust import ArtifactArchive, ArtifactDir, Descriptor
from pathlib import Path


class Artifact:
    """
    Reader class for OMMX Artifacts.
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
            _base = ArtifactArchive.from_oci_archive(str(path))
        elif path.is_dir():
            _base = ArtifactDir.from_oci_dir(str(path))
        else:
            raise ValueError("Path must be a file or a directory")

        return Artifact(_base)

    @staticmethod
    def load(image_name: str) -> Artifact:
        """
        Load an artifact stored as container image in local or remote registry

        If the image is not found in local registry, it will try to pull from remote registry.
        """
        raise NotImplementedError

    @property
    def layers(self) -> list[Descriptor]:
        return self._base.layers

    def get_blob(self, digest: str | Descriptor) -> bytes:
        if isinstance(digest, Descriptor):
            digest = digest.digest
        return self._base.get_blob(digest)
