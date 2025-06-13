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

    # Test description fields are set correctly
    inst_desc = instance.description
    assert inst_desc.name == "Test Problem"
    assert inst_desc.description == "A simple test optimization problem"
    assert inst_desc.authors == ["Test Author"]
    assert inst_desc.created_by == "OMMX Test"