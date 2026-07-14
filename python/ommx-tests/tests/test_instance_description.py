"""Tests for Instance description property."""

import pytest

from ommx import (
    DecisionVariable,
    Instance,
    OneHotConstraint,
    Parameter,
    ParametricInstance,
)


def _set_mapping_item(mapping, key, value):
    mapping[key] = value


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

    restored = Instance.from_v1_bytes(instance.to_v1_bytes())

    assert restored.title == "Proto Title"
    assert restored.license == "MIT"
    assert restored.dataset == "unit-test"
    assert restored.get_user_annotation("source") == "bytes"
    assert restored.description is not None
    assert restored.description.name == "Proto Title"
    assert restored.description.license == "MIT"
    assert restored.description.dataset == "unit-test"


def test_instance_annotations_is_read_only_mapping():
    """The annotations projection is readable but not mutable in place."""
    instance = Instance.empty()
    instance.add_user_annotation("source", "old")

    annotations = instance.annotations

    assert annotations["org.ommx.user.source"] == "old"
    assert dict(annotations)["org.ommx.user.source"] == "old"
    with pytest.raises(TypeError):
        _set_mapping_item(annotations, "org.ommx.user.source", "new")
    with pytest.raises(AttributeError):
        setattr(instance, "annotations", {})
    assert instance.get_user_annotation("source") == "old"


def test_instance_replace_annotations_replaces_existing_metadata():
    """replace_annotations replaces protobuf-backed metadata and user annotations."""
    instance = Instance.empty()
    instance.title = "Old Title"
    instance.license = "MIT"
    instance.add_user_annotation("source", "old")

    instance.replace_annotations({"org.ommx.user.keep": "new"})

    assert instance.title is None
    assert instance.license is None
    assert instance.get_user_annotation("keep") == "new"
    assert "org.ommx.user.source" not in instance.annotations
    assert "org.ommx.v1.instance.title" not in instance.annotations
    assert "org.ommx.v1.instance.variables" in instance.annotations


def test_instance_annotations_do_not_serialize_structural_constraints():
    """Annotation operations must not round-trip through v1 serialization."""
    x = [DecisionVariable.binary(0), DecisionVariable.binary(1)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0],
        constraints={},
        one_hot_constraints={10: OneHotConstraint(variables=x)},
        sense=Instance.MINIMIZE,
    )

    instance.title = "Structural Instance"
    annotations = instance.annotations

    assert instance.title == "Structural Instance"
    assert annotations["org.ommx.v1.instance.title"] == "Structural Instance"
    assert annotations["org.ommx.v1.instance.constraints"] == "1"
    assert instance.num_constraints == 1
    assert len(instance.constraints_df(kind="one_hot")) == 1


def test_instance_add_user_annotation_rejects_reserved_namespace():
    """User annotation helpers must not write OMMX-reserved proto annotation keys."""
    instance = Instance.empty()

    with pytest.raises(ValueError, match="reserved for OMMX metadata"):
        instance.add_user_annotation(
            "title",
            "invalid",
            annotation_namespace="org.ommx.v1.instance",
        )

    with pytest.raises(ValueError, match="reserved for OMMX metadata"):
        instance.add_user_annotations(
            {"title": "invalid"},
            annotation_namespace="org.ommx.v1.instance",
        )

    with pytest.raises(ValueError, match="reserved for OMMX metadata"):
        instance.get_user_annotation(
            "title",
            annotation_namespace="org.ommx.v1.instance",
        )

    with pytest.raises(ValueError, match="reserved for OMMX metadata"):
        instance.get_user_annotations(annotation_namespace="org.ommx.v1.instance")

    assert instance.title is None
    assert "org.ommx.v1.instance.title" not in instance.annotations


def test_solution_annotations_round_trip_through_bytes():
    """Solution provenance and user annotations are persisted in protobuf."""
    solution = Instance.empty().evaluate({})
    solution.solver = {"name": "unit-solver"}
    solution.parameters = {"time_limit": 1}
    solution.add_user_annotation("source", "bytes")

    restored = type(solution).from_v1_bytes(solution.to_v1_bytes())

    assert restored.solver == {"name": "unit-solver"}
    assert restored.parameters == {"time_limit": 1}
    assert restored.get_user_annotation("source") == "bytes"


def test_solution_annotations_is_read_only_mapping():
    """Solution annotations projection is also not mutable in place."""
    solution = Instance.empty().evaluate({})
    solution.add_user_annotation("source", "old")

    annotations = solution.annotations

    assert annotations["org.ommx.user.source"] == "old"
    with pytest.raises(TypeError):
        _set_mapping_item(annotations, "org.ommx.user.source", "new")
    with pytest.raises(AttributeError):
        setattr(solution, "annotations", {})
    assert solution.get_user_annotation("source") == "old"


def test_solution_replace_annotations_replaces_existing_metadata():
    """replace_annotations clears Solution ProcessMetadata and user annotations."""
    solution = Instance.empty().evaluate({})
    solution.instance = "sha256:old"
    solution.solver = {"name": "old-solver"}
    solution.parameters = {"time_limit": 1}
    solution.add_user_annotation("source", "old")

    solution.replace_annotations({"org.ommx.user.keep": "new"})

    assert solution.instance is None
    assert solution.solver is None
    assert solution.parameters is None
    assert solution.get_user_annotation("keep") == "new"
    assert "org.ommx.user.source" not in solution.annotations
    assert "org.ommx.v1.solution.solver" not in solution.annotations


def test_solution_annotations_preserve_structural_constraint_evaluations():
    """Annotation setters must not drop non-regular evaluated constraints."""
    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        one_hot_constraints={10: OneHotConstraint(variables=x)},
        sense=Instance.MAXIMIZE,
    )
    solution = instance.evaluate({1: 0.0, 2: 1.0, 3: 0.0})

    assert len(solution.constraints_df(kind="one_hot")) == 1

    solution.solver = {"name": "unit-solver"}

    assert solution.solver == {"name": "unit-solver"}
    assert len(solution.constraints_df(kind="one_hot")) == 1


def test_solution_add_user_annotation_rejects_reserved_namespace():
    """Solution user annotation helpers must reject OMMX metadata namespaces."""
    solution = Instance.empty().evaluate({})

    with pytest.raises(ValueError, match="reserved for OMMX metadata"):
        solution.add_user_annotation(
            "solver",
            "invalid",
            annotation_namespace="org.ommx.v1.solution",
        )

    assert solution.solver is None
    assert "org.ommx.v1.solution.solver" not in solution.annotations


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

    restored = ParametricInstance.from_v1_bytes(instance.to_v1_bytes())

    assert restored.title == "Parametric Proto Title"
    assert restored.get_user_annotation("source") == "bytes"
    assert restored.description is not None
    assert restored.description.name == "Parametric Proto Title"


def test_parametric_instance_replace_annotations_replaces_existing_metadata():
    """replace_annotations clears ParametricInstance description and user annotations."""
    x = DecisionVariable.binary(0)
    p = Parameter(100, name="p")
    instance = ParametricInstance.from_components(
        decision_variables=[x],
        parameters=[p],
        objective=x + p,
        constraints={},
        sense=Instance.MINIMIZE,
    )
    instance.title = "Old Parametric Title"
    instance.add_user_annotation("source", "old")

    instance.replace_annotations({})

    assert instance.title is None
    assert "org.ommx.user.source" not in instance.annotations
    assert "org.ommx.v1.parametric-instance.title" not in instance.annotations
    assert "org.ommx.v1.parametric-instance.variables" in instance.annotations


def test_sample_set_annotations_round_trip_through_bytes():
    """SampleSet provenance and user annotations are persisted in protobuf."""
    sample_set = Instance.empty().evaluate_samples([{}])
    sample_set.solver = {"name": "unit-sampler"}
    sample_set.parameters = {"num_reads": 10}
    sample_set.add_user_annotation("source", "bytes")

    restored = type(sample_set).from_v1_bytes(sample_set.to_v1_bytes())

    assert restored.solver == {"name": "unit-sampler"}
    assert restored.parameters == {"num_reads": 10}
    assert restored.get_user_annotation("source") == "bytes"


def test_sample_set_replace_annotations_replaces_existing_metadata():
    """replace_annotations clears SampleSet ProcessMetadata and user annotations."""
    sample_set = Instance.empty().evaluate_samples([{}])
    sample_set.instance = "sha256:old"
    sample_set.solver = {"name": "old-sampler"}
    sample_set.parameters = {"num_reads": 10}
    sample_set.add_user_annotation("source", "old")

    sample_set.replace_annotations({})

    assert sample_set.instance is None
    assert sample_set.solver is None
    assert sample_set.parameters is None
    assert "org.ommx.user.source" not in sample_set.annotations
    assert "org.ommx.v1.sample-set.solver" not in sample_set.annotations


def test_sample_set_annotations_preserve_structural_constraint_samples():
    """Annotation setters must not reparse and invalidate sampled structural constraints."""
    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        one_hot_constraints={10: OneHotConstraint(variables=x)},
        sense=Instance.MAXIMIZE,
    )
    sample_set = instance.evaluate_samples(
        {
            0: {1: 1.0, 2: 0.0, 3: 0.0},
            1: {1: 1.0, 2: 1.0, 3: 0.0},
        }
    )

    assert len(sample_set.constraints_df(kind="one_hot")) == 1

    sample_set.solver = {"name": "unit-sampler"}

    assert sample_set.solver == {"name": "unit-sampler"}
    assert len(sample_set.constraints_df(kind="one_hot")) == 1
