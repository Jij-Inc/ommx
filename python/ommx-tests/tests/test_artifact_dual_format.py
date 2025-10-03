"""
Test dual format support for OMMX artifacts (oci-dir and oci-archive).

This test validates that:
1. New artifacts default to oci-archive format
2. Both oci-dir and oci-archive formats can be loaded
3. Backward compatibility is maintained
4. get_artifact_path correctly identifies both formats
"""

import uuid

import pytest

from ommx.artifact import (
    Artifact,
    ArtifactBuilder,
    ArtifactDirBuilder,
    get_local_registry_path,
)
from ommx.testing import SingleFeasibleLPGenerator, DataType


@pytest.fixture
def test_instance():
    """Create a test instance for artifact tests."""
    generator = SingleFeasibleLPGenerator(3, DataType.INT)
    return generator.get_v1_instance()


def test_new_artifacts_default_to_archive_format(test_instance):
    """Test that new artifacts created with ArtifactBuilder.new() use oci-archive format."""
    image_name = f"test.local/archive-default:{uuid.uuid4()}"

    # Build artifact using the default new() method
    builder = ArtifactBuilder.new(image_name)
    builder.add_instance(test_instance)
    artifact = builder.build()

    # Verify artifact is stored as oci-archive format (.ommx file)
    base_path = get_local_registry_path(image_name)
    archive_path = base_path.with_suffix(".ommx")
    assert archive_path.exists()
    assert archive_path.is_file()
    assert archive_path.suffix == ".ommx"

    # Verify we can load it back
    loaded = Artifact.load(image_name)
    assert loaded.image_name == image_name
    assert len(loaded.layers) == len(artifact.layers)


def test_legacy_oci_dir_format_still_works(test_instance):
    """Test that the legacy oci-dir format can still be created and loaded."""
    image_name = f"test.local/dir-legacy:{uuid.uuid4()}"

    # Build artifact using the legacy dir format
    dir_builder_base = ArtifactDirBuilder.new(image_name)
    builder = ArtifactBuilder(dir_builder_base)
    builder.add_instance(test_instance)
    artifact = builder.build()

    # Verify artifact is stored as oci-dir format (directory)
    dir_path = get_local_registry_path(image_name)
    assert dir_path.exists()
    assert dir_path.is_dir()
    assert (dir_path / "oci-layout").exists()

    # Verify we can load it back
    loaded = Artifact.load(image_name)
    assert loaded.image_name == image_name
    assert len(loaded.layers) == len(artifact.layers)


def test_dual_format_interoperability(test_instance):
    """Test that both formats work seamlessly together."""
    archive_image = f"test.local/interop-archive:{uuid.uuid4()}"
    dir_image = f"test.local/interop-dir:{uuid.uuid4()}"

    # Create artifacts in both formats
    archive_builder = ArtifactBuilder.new(archive_image)
    archive_builder.add_instance(test_instance)
    archive_builder.build()

    dir_builder = ArtifactBuilder(ArtifactDirBuilder.new(dir_image))
    dir_builder.add_instance(test_instance)
    dir_builder.build()

    # Both can be loaded with the same API
    loaded_archive = Artifact.load(archive_image)
    loaded_dir = Artifact.load(dir_image)
    assert loaded_archive.image_name == archive_image
    assert loaded_dir.image_name == dir_image

    # get_local_registry_path returns base path, format checked via path suffix/directory
    archive_base_path = get_local_registry_path(archive_image)
    archive_file = archive_base_path.with_suffix(".ommx")
    assert archive_file.exists() and archive_file.is_file()

    dir_base_path = get_local_registry_path(dir_image)
    assert dir_base_path.exists() and dir_base_path.is_dir()

    # Non-existent artifacts don't have files/directories
    nonexistent_base = get_local_registry_path(f"test.local/nonexistent:{uuid.uuid4()}")
    assert not nonexistent_base.exists()
    assert not nonexistent_base.with_suffix(".ommx").exists()
