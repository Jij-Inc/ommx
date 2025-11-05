from __future__ import annotations

import io
import json
import pandas
import numpy
from dataclasses import dataclass
from pathlib import Path
from abc import ABC, abstractmethod

from ._ommx_rust import (
    ArtifactArchive as _ArtifactArchive,
    ArtifactDir as _ArtifactDir,
    Descriptor,
    ArtifactArchiveBuilder as _ArtifactArchiveBuilder,
    ArtifactDirBuilder as _ArtifactDirBuilder,
    get_local_registry_root as _get_local_registry_root,
    set_local_registry_root as _set_local_registry_root,
    get_image_dir as _get_image_dir,
    get_images as _get_images,
)
from .v1 import Instance, Solution, ParametricInstance, SampleSet


__all__ = [
    "Artifact",
    "ArtifactBuilder",
    "ArtifactBase",
    "ArtifactDir",
    "ArtifactDirBuilder",
    "ArtifactArchive",
    "ArtifactArchiveBuilder",
    "Descriptor",
    "get_local_registry_root",
    "set_local_registry_root",
    "get_image_dir",
    "get_images",
]


def get_local_registry_root() -> Path:
    """
    Get the path to the local OMMX registry root directory.

    The local registry is used to store OMMX artifacts locally. By default, it uses
    the XDG Base Directory specification, but can be overridden using
    :py:func:`set_local_registry_root` or the ``OMMX_LOCAL_REGISTRY_ROOT`` environment variable.

    Returns:
        Path: Path to the local registry root directory

    See Also:
        :py:func:`set_local_registry_root`: Set a custom registry root path
        :py:func:`get_image_dir`: Get the directory for a specific image
    """
    return Path(_get_local_registry_root())


def set_local_registry_root(path: str | Path) -> None:
    """
    Set the path to the local OMMX registry root directory.

    This function allows you to override the default registry location. The change
    affects all subsequent artifact operations in the current process.

    Args:
        path: Path to the local registry root directory

    See Also:
        :py:func:`get_local_registry_root`: Get the current registry root path
    """
    _set_local_registry_root(path)


def get_image_dir(image_name: str) -> Path:
    """
    Get the directory path for a specific image in the local registry.

    This function returns the filesystem path where the artifact with the given
    image name is stored in the local registry.

    Args:
        image_name: The OMMX image name in the format ``registry/repository:tag``
            (e.g., ``ghcr.io/jij-inc/ommx/example:v1``)

    Returns:
        Path: Directory path for the image in the local registry

    See Also:
        :py:func:`get_local_registry_root`: Get the registry root directory
        :py:class:`Artifact`: Load artifacts from the registry
    """
    return Path(_get_image_dir(image_name))


def get_images() -> list[str]:
    """
    Get all image names stored in the local registry.

    This function scans the local registry and returns a list of all OMMX artifact
    image names that are currently stored locally.

    Returns:
        list[str]: List of image names in the format ``registry/repository:tag``

    See Also:
        :py:func:`get_local_registry_root`: Get the registry root directory
        :py:func:`get_image_dir`: Get the directory for a specific image
        :py:class:`Artifact`: Load artifacts from the registry
    """
    return _get_images()


class ArtifactBase(ABC):
    @property
    @abstractmethod
    def image_name(self) -> str | None: ...

    @property
    @abstractmethod
    def annotations(self) -> dict[str, str]: ...

    @property
    @abstractmethod
    def layers(self) -> list[Descriptor]: ...

    @abstractmethod
    def get_blob(self, digest: str) -> bytes: ...

    @abstractmethod
    def push(self): ...


# FIXME: This wrapper class should be defined in Rust binding directly,
#        but PyO3 does not support inheriting Python class https://github.com/PyO3/pyo3/issues/991
@dataclass
class ArtifactArchive(ArtifactBase):
    _base: _ArtifactArchive

    @staticmethod
    def from_oci_archive(path: str) -> ArtifactArchive:
        return ArtifactArchive(_ArtifactArchive.from_oci_archive(path))

    @property
    def image_name(self) -> str | None:
        return self._base.image_name

    @property
    def annotations(self) -> dict[str, str]:
        return self._base.annotations

    @property
    def layers(self) -> list[Descriptor]:
        return self._base.layers

    def get_blob(self, digest: str) -> bytes:
        return self._base.get_blob(digest)

    def push(self):
        self._base.push()


# FIXME: This wrapper class should be defined in Rust binding directly,
#        but PyO3 does not support inheriting Python class https://github.com/PyO3/pyo3/issues/991
@dataclass
class ArtifactDir(ArtifactBase):
    _base: _ArtifactDir

    @staticmethod
    def from_oci_dir(path: str) -> ArtifactDir:
        return ArtifactDir(_ArtifactDir.from_oci_dir(path))

    @staticmethod
    def from_image_name(image_name: str) -> ArtifactDir:
        return ArtifactDir(_ArtifactDir.from_image_name(image_name))

    @property
    def image_name(self) -> str | None:
        return self._base.image_name

    @property
    def annotations(self) -> dict[str, str]:
        return self._base.annotations

    @property
    def layers(self) -> list[Descriptor]:
        return self._base.layers

    def get_blob(self, digest: str) -> bytes:
        return self._base.get_blob(digest)

    def push(self):
        self._base.push()


@dataclass
class Artifact:
    """
    Reader for OMMX Artifacts.
    """

    _base: ArtifactBase

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

    def get_layer(self, descriptor: Descriptor) -> Instance | Solution | numpy.ndarray:
        """
        Get the layer object corresponding to the descriptor

        This is dynamically dispatched based on the :py:attr:`Descriptor.media_type`.
        """
        if descriptor.media_type == "application/org.ommx.v1.instance":
            return self.get_instance(descriptor)
        if descriptor.media_type == "application/org.ommx.v1.solution":
            return self.get_solution(descriptor)
        if descriptor.media_type == "application/vnd.numpy":
            return self.get_ndarray(descriptor)
        raise ValueError(f"Unsupported media type {descriptor.media_type}")

    @property
    def instance(self) -> Instance:
        """
        Take the first instance layer in the artifact

        - If the artifact does not contain any instance layer, it raises an :py:exc:`ValueError`.
        - For multiple instance layers, use :py:meth:`Artifact.get_instance` instead.
        """
        for desc in self.layers:
            if desc.media_type == "application/org.ommx.v1.instance":
                return self.get_instance(desc)
        else:
            raise ValueError("Instance layer not found")

    def get_instance(self, descriptor: Descriptor) -> Instance:
        """
        Get an instance from the artifact
        """
        assert descriptor.media_type == "application/org.ommx.v1.instance"
        blob = self.get_blob(descriptor)
        instance = Instance.from_bytes(blob)
        instance.annotations = descriptor.annotations
        return instance

    @property
    def solution(self) -> Solution:
        """
        Take the first solution layer in the artifact

        - If the artifact does not have a solution layer, it raises an :py:exc:`ValueError`.
        - For multiple solution layers, use :py:meth:`Artifact.get_solution` instead.
        """
        for desc in self.layers:
            if desc.media_type == "application/org.ommx.v1.solution":
                return self.get_solution(desc)
        else:
            raise ValueError("Solution layer not found")

    def get_solution(self, descriptor: Descriptor) -> Solution:
        assert descriptor.media_type == "application/org.ommx.v1.solution"
        blob = self.get_blob(descriptor)
        solution = Solution.from_bytes(blob)
        solution.annotations = descriptor.annotations
        return solution

    @property
    def parametric_instance(self) -> ParametricInstance:
        """
        Take the first parametric instance layer in the artifact

        - If the artifact does not have a parametric instance layer, it raises an :py:exc:`ValueError`.
        - For multiple parametric instance layers, use :py:meth:`Artifact.get_parametric_instance` instead.
        """
        for desc in self.layers:
            if desc.media_type == "application/org.ommx.v1.parametric-instance":
                return self.get_parametric_instance(desc)
        else:
            raise ValueError("Parametric instance layer not found")

    def get_parametric_instance(self, descriptor: Descriptor) -> ParametricInstance:
        """
        Get an parametric instance from the artifact
        """
        assert descriptor.media_type == "application/org.ommx.v1.parametric-instance"
        blob = self.get_blob(descriptor)
        instance = ParametricInstance.from_bytes(blob)
        instance.annotations = descriptor.annotations
        return instance

    @property
    def sample_set(self) -> SampleSet:
        """
        Take the first sample set layer in the artifact

        - If the artifact does not have a sample set layer, it raises an :py:exc:`ValueError`.
        - For multiple sample set layers, use :py:meth:`Artifact.get_sample_set` instead.
        """
        for desc in self.layers:
            if desc.media_type == "application/org.ommx.v1.sample-set":
                return self.get_sample_set(desc)
        else:
            raise ValueError("Sample set layer not found")

    def get_sample_set(self, descriptor: Descriptor) -> SampleSet:
        """
        Get a sample set from the artifact
        """
        assert descriptor.media_type == "application/org.ommx.v1.sample-set"
        blob = self.get_blob(descriptor)
        sample_set = SampleSet.from_bytes(blob)
        sample_set.annotations = descriptor.annotations
        return sample_set

    def get_ndarray(self, descriptor: Descriptor) -> numpy.ndarray:
        """
        Get a numpy array from an artifact layer stored by :py:meth:`ArtifactBuilder.add_ndarray`
        """
        assert descriptor.media_type == "application/vnd.numpy"
        blob = self.get_blob(descriptor)
        f = io.BytesIO(blob)
        return numpy.load(f)

    def get_dataframe(self, descriptor: Descriptor) -> pandas.DataFrame:
        """
        Get a pandas DataFrame from an artifact layer stored by :py:meth:`ArtifactBuilder.add_dataframe`
        """
        assert descriptor.media_type == "application/vnd.apache.parquet"
        blob = self.get_blob(descriptor)
        return pandas.read_parquet(io.BytesIO(blob))

    def get_json(self, descriptor: Descriptor):
        """
        Get a JSON object from an artifact layer stored by :py:meth:`ArtifactBuilder.add_json`
        """
        assert descriptor.media_type == "application/json"
        blob = self.get_blob(descriptor)
        return json.loads(blob)


class ArtifactBuilderBase(ABC):
    @abstractmethod
    def add_layer(
        self, media_type: str, blob: bytes, annotations: dict[str, str]
    ) -> Descriptor: ...

    @abstractmethod
    def add_annotation(self, key: str, value: str): ...

    @abstractmethod
    def build(self) -> ArtifactBase: ...


# FIXME: This wrapper class should be defined in Rust binding directly,
#        but PyO3 does not support inheriting Python class https://github.com/PyO3/pyo3/issues/991
@dataclass
class ArtifactArchiveBuilder(ArtifactBuilderBase):
    _base: _ArtifactArchiveBuilder

    @staticmethod
    def new(path: str, image_name: str) -> ArtifactArchiveBuilder:
        return ArtifactArchiveBuilder(_ArtifactArchiveBuilder.new(path, image_name))

    @staticmethod
    def new_unnamed(path: str) -> ArtifactArchiveBuilder:
        return ArtifactArchiveBuilder(_ArtifactArchiveBuilder.new_unnamed(path))

    @staticmethod
    def temp() -> ArtifactArchiveBuilder:
        return ArtifactArchiveBuilder(_ArtifactArchiveBuilder.temp())

    def add_layer(
        self, media_type: str, blob: bytes, annotations: dict[str, str] = {}
    ) -> Descriptor:
        return self._base.add_layer(media_type, blob, annotations)

    def add_annotation(self, key: str, value: str):
        self._base.add_annotation(key, value)

    def build(self) -> ArtifactArchive:
        return ArtifactArchive(self._base.build())


# FIXME: This wrapper class should be defined in Rust binding directly,
#        but PyO3 does not support inheriting Python class https://github.com/PyO3/pyo3/issues/991
@dataclass
class ArtifactDirBuilder(ArtifactBuilderBase):
    _base: _ArtifactDirBuilder

    @staticmethod
    def new(image_name: str) -> ArtifactDirBuilder:
        return ArtifactDirBuilder(_ArtifactDirBuilder.new(image_name))

    @staticmethod
    def for_github(org: str, repo: str, name: str, tag: str) -> ArtifactDirBuilder:
        return ArtifactDirBuilder(_ArtifactDirBuilder.for_github(org, repo, name, tag))

    def add_layer(
        self, media_type: str, blob: bytes, annotations: dict[str, str] = {}
    ) -> Descriptor:
        return self._base.add_layer(media_type, blob, annotations)

    def add_annotation(self, key: str, value: str):
        self._base.add_annotation(key, value)

    def build(self) -> ArtifactDir:
        return ArtifactDir(self._base.build())


@dataclass(frozen=True)
class ArtifactBuilder:
    """
    Builder for OMMX Artifacts.
    """

    _base: ArtifactBuilderBase

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
    def temp() -> ArtifactBuilder:
        """
        Create a new artifact as a temporary file. Note that this is insecure and should only be used for testing.

        >>> builder = ArtifactBuilder.temp()
        >>> artifact = builder.build()

        Image name is set by random UUID, and can be pushed to https://ttl.sh/ registry. This will be removed after 1 hour.

        >>> print(artifact.image_name)
        ttl.sh/...-...-...-...-...:1h
        >>> artifact.push()

        """
        return ArtifactBuilder(ArtifactArchiveBuilder.temp())

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

    def add_instance(self, instance: Instance) -> Descriptor:
        """
        Add an instance to the artifact with annotations

        Example
        ========

        >>> from ommx.v1 import Instance
        >>> instance = Instance.empty()

        Set annotations into the instance itself
        >>> instance.title = "test instance"
        >>> instance.add_user_annotation("author", "Alice")

        Add the instance to the artifact
        >>> builder = ArtifactBuilder.temp()
        >>> desc = builder.add_instance(instance)
        >>> print(desc.annotations['org.ommx.v1.instance.title'])
        test instance
        >>> print(desc.annotations['org.ommx.user.author'])
        Alice
        >>> artifact = builder.build()

        Load the instance from the artifact by :py:meth:`Artifact.get_instance`
        >>> instance2 = artifact.get_instance(desc)
        >>> print(instance2.title)
        test instance
        >>> print(instance2.get_user_annotations())
        {'author': 'Alice'}

        """
        blob = instance.to_bytes()
        return self.add_layer(
            "application/org.ommx.v1.instance", blob, instance.annotations
        )

    def add_parametric_instance(self, instance: ParametricInstance) -> Descriptor:
        """
        Add a parametric instance to the artifact with annotations
        """
        blob = instance.to_bytes()
        return self.add_layer(
            "application/org.ommx.v1.parametric-instance", blob, instance.annotations
        )

    def add_solution(self, solution: Solution) -> Descriptor:
        """
        Add a solution to the artifact with annotations

        Example
        ========

        >>> from ommx.v1 import Instance, Solution
        >>> instance = Instance.empty()
        >>> solution = instance.evaluate({})

        Add the instance to the artifact first
        >>> builder = ArtifactBuilder.temp()
        >>> instance_desc = builder.add_instance(instance)

        Set annotations into the solution itself
        >>> solution.instance = instance_desc.digest
        >>> solution.solver = "manual"
        >>> solution.add_user_annotation("title", "test solution")
        >>> _desc = builder.add_solution(solution)
        >>> artifact = builder.build()

        Load the solution from the artifact by :py:meth:`Artifact.get_solution`
        >>> solution2 = artifact.get_solution(_desc)
        >>> print(solution2.instance)
        sha256:...
        >>> print(solution2.solver)
        manual
        >>> print(solution2.get_user_annotations())
        {'title': 'test solution'}

        """
        blob = solution.to_bytes()
        return self.add_layer(
            "application/org.ommx.v1.solution", blob, solution.annotations
        )

    def add_sample_set(self, sample_set: SampleSet) -> Descriptor:
        """
        Add a sample set to the artifact with annotations
        """
        blob = sample_set.to_bytes()
        return self.add_layer(
            "application/org.ommx.v1.sample-set", blob, sample_set.annotations
        )

    def add_ndarray(
        self,
        array: numpy.ndarray,
        /,
        *,
        annotation_namespace: str = "org.ommx.user.",
        **annotations: str,
    ) -> Descriptor:
        """
        Add a numpy ndarray to the artifact with npy format

        Example
        ========

        >>> import numpy as np
        >>> array = np.array([1, 2, 3])

        Store the array in the artifact with `application/vnd.numpy` media type. We can also add annotations to the layer.

        >>> builder = ArtifactBuilder.temp()
        >>> _desc = builder.add_ndarray(array, title="test_array")
        >>> artifact = builder.build()

        The `title` annotation is stored as `org.ommx.user.title` in the artifact, which can be accessed by :py:attr:`Descriptor.annotations` or :py:attr:`Descriptor.user_annotations`.

        >>> layer = artifact.layers[0]
        >>> print(layer.media_type)
        application/vnd.numpy
        >>> print(layer.annotations)
        {'org.ommx.user.title': 'test_array'}
        >>> print(layer.user_annotations)
        {'title': 'test_array'}

        Load the array from the artifact by :py:meth:`Artifact.get_ndarray`

        >>> ndarray = artifact.get_ndarray(layer)
        >>> print(ndarray)
        [1 2 3]

        """
        f = io.BytesIO()
        numpy.save(f, array)
        blob = f.getvalue()
        if not annotation_namespace.endswith("."):
            annotation_namespace += "."
        annotations = {annotation_namespace + k: v for k, v in annotations.items()}
        return self.add_layer("application/vnd.numpy", blob, annotations)

    def add_dataframe(
        self,
        df: pandas.DataFrame,
        /,
        *,
        annotation_namespace: str = "org.ommx.user.",
        **annotations: str,
    ) -> Descriptor:
        """
        Add a pandas DataFrame to the artifact with parquet format

        Example
        ========
        >>> import pandas as pd
        >>> df = pd.DataFrame({"a": [1, 2], "b": [3, 4]})

        Store the DataFrame in the artifact with `application/vnd.apache.parquet` media type.

        >>> builder = ArtifactBuilder.temp()
        >>> _desc = builder.add_dataframe(df, title="test_dataframe")
        >>> artifact = builder.build()

        The `title` annotation is stored as `org.ommx.user.title` in the artifact, which can be accessed by :py:attr:`Descriptor.annotations` or :py:attr:`Descriptor.user_annotations`.

        >>> layer = artifact.layers[0]
        >>> print(layer.media_type)
        application/vnd.apache.parquet
        >>> print(layer.annotations)
        {'org.ommx.user.title': 'test_dataframe'}
        >>> print(layer.user_annotations)
        {'title': 'test_dataframe'}

        >>> df2 = artifact.get_dataframe(layer)
        >>> assert df.equals(df2)

        You can use another namespace for annotations via `annotation_namespace` argument.

        >>> builder = ArtifactBuilder.temp()
        >>> desc = builder.add_dataframe(df, annotation_namespace="org.ommx.user2.", title="test_dataframe")
        >>> print(desc.annotations)
        {'org.ommx.user2.title': 'test_dataframe'}

        """
        blob = df.to_parquet()
        if not annotation_namespace.endswith("."):
            annotation_namespace += "."
        annotations = {annotation_namespace + k: v for k, v in annotations.items()}
        return self.add_layer("application/vnd.apache.parquet", blob, annotations)

    def add_json(
        self,
        obj,
        /,
        *,
        annotation_namespace: str = "org.ommx.user.",
        **annotations: str,
    ) -> Descriptor:
        """
        Add a JSON object to the artifact

        Example
        ========

        >>> obj = {"a": 1, "b": 2}

        Store the object in the artifact with `application/json` media type.

        >>> builder = ArtifactBuilder.temp()
        >>> _desc = builder.add_json(obj, title="test_json")
        >>> artifact = builder.build()

        The `title` annotation is stored as `org.ommx.user.title` in the artifact, which can be accessed by :py:attr:`Descriptor.annotations` or :py:attr:`Descriptor.user_annotations`.

        >>> layer = artifact.layers[0]
        >>> print(layer.media_type)
        application/json
        >>> print(layer.annotations)
        {'org.ommx.user.title': 'test_json'}
        >>> print(layer.user_annotations)
        {'title': 'test_json'}

        """
        blob = json.dumps(obj).encode("utf-8")
        if not annotation_namespace.endswith("."):
            annotation_namespace += "."
        annotations = {annotation_namespace + k: v for k, v in annotations.items()}
        return self.add_layer("application/json", blob, annotations)

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
