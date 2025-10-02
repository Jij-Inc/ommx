"""
Test dual format support for OMMX artifacts (oci-dir and oci-archive).

This test validates that:
1. New artifacts default to oci-archive format
2. Both oci-dir and oci-archive formats can be loaded
3. Backward compatibility is maintained
4. get_artifact_path correctly identifies both formats
"""
import tempfile
import shutil
import uuid
from pathlib import Path

import pytest

from ommx.artifact import (
    Artifact, 
    ArtifactBuilder, 
    ArtifactDirBuilder,
    ArtifactArchive,
    ArtifactDir,
    get_artifact_path,
    set_local_registry_root,
)
from ommx.testing import SingleFeasibleLPGenerator, DataType


@pytest.fixture
def temp_registry():
    """Create a temporary directory for local registry testing."""
    temp_dir = tempfile.mkdtemp()
    try:
        set_local_registry_root(temp_dir)
        yield Path(temp_dir)
    finally:
        shutil.rmtree(temp_dir, ignore_errors=True)


def test_new_artifacts_default_to_archive_format(temp_registry):
    """Test that new artifacts created with ArtifactBuilder.new() use oci-archive format."""
    # Create a test instance
    generator = SingleFeasibleLPGenerator(3, DataType.INT)
    instance = generator.get_v1_instance()
    
    # Create a unique image name
    image_name = f"test.local/dual-format-test:{uuid.uuid4()}"
    
    # Build artifact using the default new() method
    builder = ArtifactBuilder.new(image_name)
    desc = builder.add_instance(instance)
    artifact = builder.build()
    
    # Verify artifact was created
    assert artifact.image_name == image_name
    
    # Check that the artifact is stored as a file (.ommx extension)
    artifact_path = get_artifact_path(image_name)
    assert artifact_path is not None
    assert artifact_path.is_file()
    assert artifact_path.suffix == ".ommx" or artifact_path.name.endswith(".ommx")
    
    # Verify we can load it back
    loaded_artifact = Artifact.load(image_name)
    assert loaded_artifact.image_name == image_name
    assert len(loaded_artifact.layers) == len(artifact.layers)


def test_legacy_oci_dir_format_still_works(temp_registry):
    """Test that the legacy oci-dir format can still be created and loaded."""
    # Create a test instance
    generator = SingleFeasibleLPGenerator(3, DataType.INT)
    instance = generator.get_v1_instance()
    
    # Create a unique image name
    image_name = f"test.local/legacy-format-test:{uuid.uuid4()}"
    
    # Build artifact using the legacy dir format
    builder = ArtifactDirBuilder.new(image_name)
    desc = builder.add_instance(instance)
    artifact = builder.build()
    
    # Verify artifact was created
    assert artifact.image_name == image_name
    
    # Check that the artifact is stored as a directory
    artifact_path = get_artifact_path(image_name)
    assert artifact_path is not None
    assert artifact_path.is_dir()
    assert (artifact_path / "oci-layout").exists()
    
    # Verify we can load it back
    loaded_artifact = Artifact.load(image_name)
    assert loaded_artifact.image_name == image_name
    assert len(loaded_artifact.layers) == len(artifact.layers)


def test_artifact_load_handles_both_formats(temp_registry):
    """Test that Artifact.load() can handle both oci-dir and oci-archive formats."""
    # Create test instances
    generator = SingleFeasibleLPGenerator(3, DataType.INT)
    instance = generator.get_v1_instance()
    
    # Create artifacts in both formats
    archive_image = f"test.local/archive-test:{uuid.uuid4()}"
    dir_image = f"test.local/dir-test:{uuid.uuid4()}"
    
    # Create archive format artifact
    archive_builder = ArtifactBuilder.new(archive_image)
    archive_builder.add_instance(instance)
    archive_artifact = archive_builder.build()
    
    # Create dir format artifact  
    dir_builder = ArtifactDirBuilder.new(dir_image)
    dir_builder.add_instance(instance)
    dir_artifact = dir_builder.build()
    
    # Verify both can be loaded with the same API
    loaded_archive = Artifact.load(archive_image)
    loaded_dir = Artifact.load(dir_image)
    
    assert loaded_archive.image_name == archive_image
    assert loaded_dir.image_name == dir_image
    
    # Both should have the same structure
    assert len(loaded_archive.layers) == len(loaded_dir.layers)
    assert len(loaded_archive.layers) > 0


def test_get_artifact_path_finds_both_formats(temp_registry):
    """Test that get_artifact_path can find artifacts in both formats."""
    # Create test instances
    generator = SingleFeasibleLPGenerator(3, DataType.INT)
    instance = generator.get_v1_instance()
    
    # Create artifacts in both formats
    archive_image = f"test.local/path-test-archive:{uuid.uuid4()}"
    dir_image = f"test.local/path-test-dir:{uuid.uuid4()}"
    
    # Create and build artifacts
    archive_builder = ArtifactBuilder.new(archive_image)
    archive_builder.add_instance(instance)
    archive_builder.build()
    
    dir_builder = ArtifactDirBuilder.new(dir_image)  
    dir_builder.add_instance(instance)
    dir_builder.build()
    
    # Test get_artifact_path finds both
    archive_path = get_artifact_path(archive_image)
    dir_path = get_artifact_path(dir_image)
    
    assert archive_path is not None
    assert dir_path is not None
    
    assert archive_path.is_file()
    assert dir_path.is_dir()
    
    # Test with non-existent artifact
    nonexistent_path = get_artifact_path(f"test.local/nonexistent:{uuid.uuid4()}")
    assert nonexistent_path is None


def test_backward_compatibility_with_existing_api(temp_registry):
    """Test that existing code continues to work without changes.""" 
    # Create a test instance
    generator = SingleFeasibleLPGenerator(3, DataType.INT)
    instance = generator.get_v1_instance()
    
    # Create artifacts using both approaches
    image_name = f"test.local/compat-test:{uuid.uuid4()}"
    
    # Original API - should now default to archive format
    builder = ArtifactBuilder.new(image_name)
    builder.add_instance(instance)
    artifact = builder.build()
    
    # Loading should work transparently
    loaded = Artifact.load(image_name)
    
    # All the existing properties should work
    assert loaded.image_name == image_name
    assert loaded.annotations is not None
    assert len(loaded.layers) > 0
    
    # Should be able to get the instance back
    instance_layer = None
    for layer in loaded.layers:
        if layer.media_type == "application/org.ommx.v1.instance": 
            instance_layer = layer
            break
    
    assert instance_layer is not None
    retrieved_instance = loaded.get_instance(instance_layer)
    assert retrieved_instance is not None


if __name__ == "__main__":
    pytest.main([__file__])