"""Tests for Instance description property."""

from ommx.v1 import Instance, DecisionVariable


def test_instance_description_none():
    """Test that instance description is None when not set."""
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x], objective=x, constraints=[], sense=Instance.MINIMIZE
    )

    assert instance.description is None


def test_instance_description_with_from_components():
    """Test creating instance with description using from_components."""
    # Create InstanceDescription
    desc = Instance.Description(
        name="Test Problem",
        description="A simple test optimization problem",
        authors=["Test Author"],
        created_by="OMMX Test",
    )

    # Create instance with description
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints=[],
        sense=Instance.MINIMIZE,
        description=desc,
    )

    # Test description is accessible
    assert instance.description is not None

    # Test description fields
    inst_desc = instance.description
    assert inst_desc.name == "Test Problem"
    assert inst_desc.description == "A simple test optimization problem"
    assert inst_desc.authors == ["Test Author"]
    assert inst_desc.created_by == "OMMX Test"


def test_instance_description_constructor():
    """Test InstanceDescription constructor with various combinations."""
    # Test full constructor
    desc1 = Instance.Description(
        name="Full Test",
        description="Full description",
        authors=["Author 1", "Author 2"],
        created_by="Creator",
    )

    assert desc1.name == "Full Test"
    assert desc1.description == "Full description"
    assert desc1.authors == ["Author 1", "Author 2"]
    assert desc1.created_by == "Creator"

    # Test partial constructor
    desc2 = Instance.Description(name="Partial Test")

    assert desc2.name == "Partial Test"
    assert desc2.description is None
    assert desc2.authors == []
    assert desc2.created_by is None

    # Test empty constructor
    desc3 = Instance.Description()

    assert desc3.name is None
    assert desc3.description is None
    assert desc3.authors == []
    assert desc3.created_by is None


def test_instance_description_repr():
    """Test InstanceDescription __repr__ method."""
    desc = Instance.Description(
        name="Repr Test",
        description="Testing repr",
        authors=["Repr Author"],
        created_by="Repr Creator",
    )

    # Test __repr__ contains expected information
    repr_str = repr(desc)
    assert "InstanceDescription" in repr_str
    assert "Repr Test" in repr_str
    assert "Testing repr" in repr_str
    assert "Repr Author" in repr_str
    assert "Repr Creator" in repr_str


def test_instance_description_deepcopy():
    """Test InstanceDescription deepcopy functionality."""
    import copy

    desc = Instance.Description(
        name="Copy Test",
        description="Testing copy",
        authors=["Copy Author"],
        created_by="Copy Creator",
    )

    # Test deepcopy
    desc_copy = copy.deepcopy(desc)

    # Verify copy has same data
    assert desc_copy.name == desc.name
    assert desc_copy.description == desc.description
    assert desc_copy.authors == desc.authors
    assert desc_copy.created_by == desc.created_by

    # Verify they are different objects
    assert desc_copy is not desc


def test_instance_description_empty_lists():
    """Test InstanceDescription handles empty authors list correctly."""
    desc = Instance.Description(name="Empty List Test", authors=[])

    assert desc.name == "Empty List Test"
    assert desc.authors == []


def test_instance_with_description_serialization():
    """Test that instance with description can be serialized and deserialized."""
    # Create instance with description
    desc = Instance.Description(
        name="Serialization Test",
        description="Testing serialization",
        authors=["Author"],
        created_by="Creator",
    )

    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints=[],
        sense=Instance.MINIMIZE,
        description=desc,
    )

    # Serialize and deserialize
    bytes_data = instance.to_bytes()
    restored_instance = Instance.from_bytes(bytes_data)

    # Test description is preserved
    assert restored_instance.description is not None
    restored_desc = restored_instance.description
    assert restored_desc.name == "Serialization Test"
    assert restored_desc.description == "Testing serialization"
    assert restored_desc.authors == ["Author"]
    assert restored_desc.created_by == "Creator"


def test_instance_description_protobuf_compatibility():
    """Test that Instance.from_components accepts both InstanceDescription and Protocol Buffer Description."""
    from ommx.v1.instance_pb2 import Instance as _Instance

    # Test with Protocol Buffer Description
    pb_desc = _Instance.Description()
    pb_desc.name = "PB Test"
    pb_desc.description = "Protocol Buffer Description"
    pb_desc.authors.append("PB Author")
    pb_desc.created_by = "PB Creator"

    x = DecisionVariable.binary(0)
    instance1 = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints=[],
        sense=Instance.MINIMIZE,
        description=pb_desc,
    )

    # Test with Rust InstanceDescription
    rust_desc = Instance.Description(
        name="Rust Test",
        description="Rust InstanceDescription",
        authors=["Rust Author"],
        created_by="Rust Creator",
    )

    instance2 = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints=[],
        sense=Instance.MINIMIZE,
        description=rust_desc,
    )

    # Both should work and produce similar results
    assert instance1.description is not None
    assert instance2.description is not None

    desc1 = instance1.description
    desc2 = instance2.description

    # Test Protocol Buffer conversion worked
    assert desc1.name == "PB Test"
    assert desc1.description == "Protocol Buffer Description"
    assert desc1.authors == ["PB Author"]
    assert desc1.created_by == "PB Creator"

    # Test Rust description worked
    assert desc2.name == "Rust Test"
    assert desc2.description == "Rust InstanceDescription"
    assert desc2.authors == ["Rust Author"]
    assert desc2.created_by == "Rust Creator"


def test_instance_description_alias():
    """Test that Instance.Description is properly exposed and works as expected."""
    # Test that Instance.Description is accessible
    assert hasattr(Instance, "Description")

    # Test that we can create Instance.Description directly
    desc = Instance.Description(
        name="Alias Test",
        description="Testing Instance.Description alias",
        authors=["Alias Author"],
        created_by="Alias Creator",
    )

    # Test properties
    assert desc.name == "Alias Test"
    assert desc.description == "Testing Instance.Description alias"
    assert desc.authors == ["Alias Author"]
    assert desc.created_by == "Alias Creator"

    # Test that it works with from_components
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints=[],
        sense=Instance.MINIMIZE,
        description=desc,
    )

    # Test that description is properly set
    assert instance.description is not None
    inst_desc = instance.description
    assert inst_desc.name == "Alias Test"
    assert inst_desc.description == "Testing Instance.Description alias"
    assert inst_desc.authors == ["Alias Author"]
    assert inst_desc.created_by == "Alias Creator"
