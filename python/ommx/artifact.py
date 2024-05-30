from __future__ import annotations

from ._ommx_rust import ArtifactArchive, ArtifactDir, Descriptor
from pathlib import Path
from .v1 import Instance, Solution


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

            >>> artifact = Artifact.load_archive("data/random_lp_instance.ommx")
            >>> for layer in artifact.layers:
            ...     print(layer.digest)
            sha256:93fdc9fcb8e21b34e3517809a348938d9455e9b9e579548bbf018a514c082df2
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

            >>> artifact = Artifact.load("ghcr.io/jij-inc/ommx/random_lp_instance:4303c7f")
            >>> for layer in artifact.layers:
            ...    print(layer.digest)
            sha256:93fdc9fcb8e21b34e3517809a348938d9455e9b9e579548bbf018a514c082df2
        """
        base = ArtifactDir.from_image_name(image_name)
        return Artifact(base)

    @property
    def layers(self) -> list[Descriptor]:
        return self._base.layers

    def get_layer_descriptor(self, digest: str) -> Descriptor:
        """
        Look up a layer descriptor by digest

            >>> artifact = Artifact.load("ghcr.io/jij-inc/ommx/random_lp_instance:4303c7f")
            >>> layer = artifact.get_layer_descriptor("sha256:93fdc9fcb8e21b34e3517809a348938d9455e9b9e579548bbf018a514c082df2")
            >>> print(layer.media_type)
            application/org.ommx.v1.instance
        """
        for layer in self.layers:
            if layer.digest == digest:
                return layer
        raise ValueError(f"Layer {digest} not found")

    def get_blob(self, digest: str | Descriptor) -> bytes:
        if isinstance(digest, Descriptor):
            digest = digest.digest
        return self._base.get_blob(digest)

    def get_layer(self, descriptor: Descriptor) -> Instance | Solution:
        """
        Get the layer object corresponding to the descriptor

        This is dynamically dispatched based on the `descriptor.media_type`.
        """
        if descriptor.media_type == "application/org.ommx.v1.instance":
            return self.get_instance(descriptor)
        if descriptor.media_type == "application/org.ommx.v1.solution":
            return self.get_solution(descriptor)
        raise ValueError(f"Unsupported media type {descriptor.media_type}")

    def get_instance(self, descriptor: Descriptor) -> Instance:
        """
        Get an instance from the artifact

            >>> artifact = Artifact.load("ghcr.io/jij-inc/ommx/random_lp_instance:4303c7f")
            >>> for layer in artifact.layers:
            ...     if layer.media_type == "application/org.ommx.v1.instance":
            ...         instance = artifact.get_instance(layer)
            ...         print(len(instance.constraints))
            7
        """
        blob = self.get_blob(descriptor)
        instance = Instance()
        instance.ParseFromString(blob)
        return instance

    def get_solution(self, descriptor: Descriptor) -> Solution:
        blob = self.get_blob(descriptor)
        solution = Solution()
        solution.ParseFromString(blob)
        return solution
