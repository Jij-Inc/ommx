from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from ._ommx_rust import (
    ArtifactArchive,
    ArtifactDir,
    Descriptor,
    ArtifactArchiveBuilder,
    ArtifactDirBuilder,
)
from .v1 import Instance, Solution


@dataclass
class Artifact:
    """
    Reader for OMMX Artifacts.
    """

    _base: ArtifactArchive | ArtifactDir

    @staticmethod
    def load_archive(path: str | Path) -> Artifact:
        """
        Load an artifact stored as a single file

        >>> artifact = Artifact.load_archive("data/random_lp_instance.ommx")
        >>> print(artifact.image_name)
        ghcr.io/jij-inc/ommx/random_lp_instance:...
        >>> for layer in artifact.layers:
        ...     print(layer.digest)
        sha256:...

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
        >>> print(artifact.image_name)
        ghcr.io/jij-inc/ommx/random_lp_instance:4303c7f
        >>> for layer in artifact.layers:
        ...    print(layer.digest)
        sha256:93fdc9fcb8e21b34e3517809a348938d9455e9b9e579548bbf018a514c082df2

        """
        base = ArtifactDir.from_image_name(image_name)
        return Artifact(base)

    def push(self):
        """
        Push the artifact to remote registry
        """
        self._base.push()

    @property
    def image_name(self) -> str | None:
        return self._base.image_name

    @property
    def annotations(self) -> dict[str, str]:
        """
        Annotations in the artifact manifest

        >>> artifact = Artifact.load("ghcr.io/jij-inc/ommx/random_lp_instance:4303c7f")
        >>> print(artifact.annotations['org.opencontainers.image.source'])
        https://github.com/Jij-Inc/ommx
        >>> print(artifact.annotations['org.opencontainers.image.description'])
        Test artifact created by examples/artifact_archive.rs

        """
        return self._base.annotations

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

        This is dynamically dispatched based on the :py:attr:`Descriptor.media_type`.
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
        return Instance.from_bytes(blob)

    def get_solution(self, descriptor: Descriptor) -> Solution:
        blob = self.get_blob(descriptor)
        return Solution.from_bytes(blob)


@dataclass(frozen=True)
class ArtifactBuilder:
    """
    Builder for OMMX Artifacts.
    """

    _base: ArtifactArchiveBuilder | ArtifactDirBuilder

    @staticmethod
    def new_archive_unnamed(path: str | Path) -> ArtifactBuilder:
        """
        Create a new artifact archive with an unnamed image name. This cannot be loaded into local registry nor pushed to remote registry.

        Example
        ========

        Ready instance to be added to the artifact

        >>> from ommx.testing import SingleFeasibleLPGenerator, DataType
        >>> generator = SingleFeasibleLPGenerator(3, DataType.INT)
        >>> instance = generator.get_v1_instance()

        File name for the artifact

        >>> import uuid  # To generate a unique name for testing
        >>> filename = f"data/single_feasible_lp.ommx.{uuid.uuid4()}"

        Build the artifact

        >>> builder = ArtifactBuilder.new_archive_unnamed(filename)
        >>> _desc = builder.add_instance(instance)
        >>> artifact = builder.build()

        In this case, the :py:attr:`Artifact.image_name` is `None`.

        >>> print(artifact.image_name)
        None

        """
        if isinstance(path, str):
            path = Path(path)
        return ArtifactBuilder(ArtifactArchiveBuilder.new_unnamed(str(path)))

    @staticmethod
    def new_archive(path: str | Path, image_name: str) -> ArtifactBuilder:
        """
        Create a new artifact archive with a named image name

        Example
        ========

        Ready instance to be added to the artifact

        >>> from ommx.testing import SingleFeasibleLPGenerator, DataType
        >>> generator = SingleFeasibleLPGenerator(3, DataType.INT)
        >>> instance = generator.get_v1_instance()

        File name and image name for the artifact.

        >>> import uuid  # To generate a unique name for testing
        >>> tag = uuid.uuid4()
        >>> filename = f"data/single_feasible_lp.ommx.{tag}"
        >>> image_name = f"ghcr.io/jij-inc/ommx/single_feasible_lp:{tag}"

        Build the artifact

        >>> builder = ArtifactBuilder.new_archive(filename, image_name)
        >>> _desc = builder.add_instance(instance)
        >>> artifact = builder.build()

        >>> print(artifact.image_name)
        ghcr.io/jij-inc/ommx/single_feasible_lp:...

        """
        if isinstance(path, str):
            path = Path(path)
        return ArtifactBuilder(ArtifactArchiveBuilder.new(str(path), image_name))

    @staticmethod
    def new(image_name: str) -> ArtifactBuilder:
        """
        Create a new artifact in local registry with a named image name

        Example
        ========

        Ready instance to be added to the artifact

        >>> from ommx.testing import SingleFeasibleLPGenerator, DataType
        >>> generator = SingleFeasibleLPGenerator(3, DataType.INT)
        >>> instance = generator.get_v1_instance()

        Image name for the artifact

        >>> import uuid  # To generate a unique name for testing
        >>> image_name = f"ghcr.io/jij-inc/ommx/single_feasible_lp:{uuid.uuid4()}"

        Build the artifact

        >>> builder = ArtifactBuilder.new(image_name)
        >>> _desc = builder.add_instance(instance)
        >>> artifact = builder.build()

        >>> print(artifact.image_name)
        ghcr.io/jij-inc/ommx/single_feasible_lp:...

        """
        return ArtifactBuilder(ArtifactDirBuilder.new(image_name))

    @staticmethod
    def for_github(org: str, repo: str, name: str, tag: str) -> ArtifactBuilder:
        """
        An alias for :py:meth:`new` to create a new artifact in local registry with GitHub Container Registry image name

        This also set the `org.opencontainers.image.source` annotation to the GitHub repository URL.

        Example
        ========

        Ready instance to be added to the artifact

        >>> from ommx.testing import SingleFeasibleLPGenerator, DataType
        >>> generator = SingleFeasibleLPGenerator(3, DataType.INT)
        >>> instance = generator.get_v1_instance()

        Build the artifact

        >>> import uuid  # To generate a unique name for testing
        >>> builder = ArtifactBuilder.for_github(
        ...    "Jij-Inc", "ommx", "single_feasible_lp", str(uuid.uuid4())
        ... )
        >>> _desc = builder.add_instance(instance)
        >>> artifact = builder.build()

        >>> print(artifact.image_name)
        ghcr.io/jij-inc/ommx/single_feasible_lp:...

        """
        return ArtifactBuilder(ArtifactDirBuilder.for_github(org, repo, name, tag))

    def add_instance( self, instance: Instance) -> Descriptor:
        """
        Add an instance to the artifact with annotations
        """
        blob = instance.to_bytes()
        annotations = instance.annotations.copy()
        if instance.created:
            annotations["org.ommx.v1.instance.created"] = instance.created.isoformat()
        if instance.title:
            annotations["org.ommx.v1.instance.title"] = instance.title
        return self.add_layer("application/org.ommx.v1.instance", blob, annotations)

    def add_layer(
        self, media_type: str, blob: bytes, annotations: dict[str, str] = {}
    ) -> Descriptor:
        """
        Low-level API to add any type of layer to the artifact with annotations. Use :meth:`add_instance` or other high-level methods if possible.
        """
        return self._base.add_layer(media_type, blob, annotations)

    def add_annotation(self, key: str, value: str):
        """
        Add annotation to the artifact itself.
        """
        self._base.add_annotation(key, value)

    def build(self) -> Artifact:
        """
        Build the artifact.
        """
        return Artifact(self._base.build())
