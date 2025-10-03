from __future__ import annotations

import io
import json
import pandas
import numpy
from pathlib import Path

from ._ommx_rust import (
    Descriptor,
    get_local_registry_root,
    set_local_registry_root,
    get_image_dir,
    get_local_registry_path as _get_local_registry_path,
    PyArtifact as _PyArtifact,  # Experimental Artifact API
    PyArtifactBuilder as _PyArtifactBuilder,  # Experimental Builder API
)
from .v1 import Instance, Solution, ParametricInstance, SampleSet


def get_local_registry_path(image_name: str) -> Path:
    """Get the base path for the given image name in the local registry.

    This returns the path where the artifact should be stored, without format-specific extensions.
    The caller should check:
    - If this path is a directory with oci-layout -> oci-dir format
    - If "{path}.ommx" exists as a file -> oci-archive format
    """
    return Path(_get_local_registry_path(image_name))


__all__ = [
    "Artifact",
    "ArtifactBuilder",
    "Descriptor",
    "get_local_registry_root",
    "set_local_registry_root",
    "get_image_dir",
    "get_local_registry_path",
]


# Legacy classes removed - now using experimental::artifact internally
# ArtifactBase, ArtifactArchive, ArtifactDir are no longer needed


class Artifact:
    """
    Reader for OMMX Artifacts.

    Now uses experimental::artifact internally for improved performance and format handling.
    """

    def __init__(self, rust_artifact: _PyArtifact):
        """Internal constructor - use static methods instead"""
        self._rust = rust_artifact

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
            rust_artifact = _PyArtifact.from_oci_archive(str(path))
        elif path.is_dir():
            rust_artifact = _PyArtifact.from_oci_dir(str(path))
        else:
            raise ValueError("Path must be a file or a directory")

        return Artifact(rust_artifact)

    @staticmethod
    def load(image_name: str) -> Artifact:
        """
        Load an artifact stored as container image in local or remote registry

        If the image is not found in local registry, it will try to pull from remote registry.
        Supports both oci-dir and oci-archive formats in local registry.

        >>> artifact = Artifact.load("ghcr.io/jij-inc/ommx/random_lp_instance:4303c7f")
        >>> print(artifact.image_name)
        ghcr.io/jij-inc/ommx/random_lp_instance:4303c7f
        >>> for layer in artifact.layers:
        ...    print(layer.digest)
        sha256:93fdc9fcb8e21b34e3517809a348938d9455e9b9e579548bbf018a514c082df2

        """
        # Use experimental Artifact.load which handles format detection and remote pull
        rust_artifact = _PyArtifact.load(image_name)
        return Artifact(rust_artifact)

    def push(self):
        """
        Push the artifact to remote registry
        """
        self._rust.push()

    @property
    def image_name(self) -> str | None:
        return self._rust.image_name

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
        return self._rust.annotations

    @property
    def layers(self) -> list[Descriptor]:
        return self._rust.layers

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
        return self._rust.get_blob(digest)

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


# Legacy builder classes removed - now using experimental::artifact::Builder internally
# ArtifactBuilderBase, ArtifactArchiveBuilder, ArtifactDirBuilder are no longer needed


class ArtifactBuilder:
    """
    Builder for OMMX Artifacts.

    Now uses experimental::artifact::Builder internally.
    """

    def __init__(self, rust_builder: _PyArtifactBuilder):
        """Internal constructor - use static methods instead"""
        self._rust = rust_builder

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
        rust_builder = _PyArtifactBuilder.new_archive_unnamed(path)
        return ArtifactBuilder(rust_builder)

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
        rust_builder = _PyArtifactBuilder.new_archive(path, image_name)
        return ArtifactBuilder(rust_builder)

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
        # Get base path and create oci-archive format path
        base_path = get_local_registry_path(image_name)
        archive_path = base_path.with_suffix(".ommx")
        # Ensure parent directory exists
        archive_path.parent.mkdir(parents=True, exist_ok=True)
        rust_builder = _PyArtifactBuilder.new_archive(archive_path, image_name)
        return ArtifactBuilder(rust_builder)

    @staticmethod
    def new_dir(path: str | Path, image_name: str) -> ArtifactBuilder:
        """
        Create a new artifact in oci-dir format at the specified path.

        This is mainly for backward compatibility with the old dir-based format.
        For new code, prefer using new() which creates oci-archive format.
        """
        if isinstance(path, str):
            path = Path(path)
        rust_builder = _PyArtifactBuilder.new_dir(path, image_name)
        return ArtifactBuilder(rust_builder)

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
        rust_builder = _PyArtifactBuilder.temp_archive()
        return ArtifactBuilder(rust_builder)

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
        image_name = f"ghcr.io/{org.lower()}/{repo.lower()}/{name}:{tag}"
        builder = ArtifactBuilder.new(image_name)
        builder.add_annotation(
            "org.opencontainers.image.source", f"https://github.com/{org}/{repo}"
        )
        return builder

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
        return self._rust.add_layer(media_type, blob, annotations)

    def add_annotation(self, key: str, value: str):
        """
        Add annotation to the artifact itself.
        """
        self._rust.add_annotation(key, value)

    def build(self) -> Artifact:
        """
        Build the artifact.
        """
        rust_artifact = self._rust.build()
        return Artifact(rust_artifact)
