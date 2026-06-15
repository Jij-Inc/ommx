"""Tests for Instance description property."""

from ommx.v1 import Instance, DecisionVariable, Parameter, ParametricInstance


def test_instance_description_none():
    """Test that instance description is None when not set."""
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x], objective=x, constraints={}, sense=Instance.MINIMIZE
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
        constraints={},
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
    assert instance.annotations["org.ommx.v1.instance.title"] == "Test Problem"
    assert instance.annotations["org.ommx.v1.instance.authors"] == "Test Author"


def test_instance_annotations_round_trip_through_bytes():
    """Instance annotations are persisted in the protobuf payload."""
    instance = Instance.empty()
    instance.title = "Proto Title"
    instance.license = "MIT"
    instance.dataset = "unit-test"
    instance.add_user_annotation("source", "bytes")

    restored = Instance.from_bytes(instance.to_bytes())

    assert restored.title == "Proto Title"
    assert restored.license == "MIT"
    assert restored.dataset == "unit-test"
    assert restored.get_user_annotation("source") == "bytes"
    assert restored.description is not None
    assert restored.description.name == "Proto Title"
    assert restored.description.license == "MIT"
    assert restored.description.dataset == "unit-test"


def test_solution_annotations_round_trip_through_bytes():
    """Solution provenance and user annotations are persisted in protobuf."""
    solution = Instance.empty().evaluate({})
    solution.solver = {"name": "unit-solver"}
    solution.parameters = {"time_limit": 1}
    solution.add_user_annotation("source", "bytes")

    restored = type(solution).from_bytes(solution.to_bytes())

    assert restored.solver == {"name": "unit-solver"}
    assert restored.parameters == {"time_limit": 1}
    assert restored.get_user_annotation("source") == "bytes"


def test_parametric_instance_annotations_round_trip_through_bytes():
    """ParametricInstance annotations are persisted in protobuf."""
    x = DecisionVariable.binary(0)
    p = Parameter(100, name="p")
    instance = ParametricInstance.from_components(
        decision_variables=[x],
        parameters=[p],
        objective=x + p,
        constraints={},
        sense=Instance.MINIMIZE,
    )
    instance.title = "Parametric Proto Title"
    instance.add_user_annotation("source", "bytes")

    restored = ParametricInstance.from_bytes(instance.to_bytes())

    assert restored.title == "Parametric Proto Title"
    assert restored.get_user_annotation("source") == "bytes"
    assert restored.description is not None
    assert restored.description.name == "Parametric Proto Title"


def test_sample_set_annotations_round_trip_through_bytes():
    """SampleSet provenance and user annotations are persisted in protobuf."""
    sample_set = Instance.empty().evaluate_samples([{}])
    sample_set.solver = {"name": "unit-sampler"}
    sample_set.parameters = {"num_reads": 10}
    sample_set.add_user_annotation("source", "bytes")

    restored = type(sample_set).from_bytes(sample_set.to_bytes())

    assert restored.solver == {"name": "unit-sampler"}
    assert restored.parameters == {"num_reads": 10}
    assert restored.get_user_annotation("source") == "bytes"
