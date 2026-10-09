from datetime import datetime, timezone
from typing import Literal

import pytest

from ommx import DecisionVariable, Instance, ParametricInstance, SampleSet, Solution
from ommx.artifact import ArtifactBuilder
from ommx import _ommx_rust
from ommx.v1.instance_pb2 import Instance as InstanceMessage
from ommx.v1.parametric_instance_pb2 import (
    ParametricInstance as ParametricInstanceMessage,
)
from ommx.v1.sample_set_pb2 import SampleSet as SampleSetMessage
from ommx.v1.solution_pb2 import Solution as SolutionMessage


def make_instance():
    x = DecisionVariable.binary(0)
    return Instance.from_components(
        objective=x, decision_variables=[x], constraints=[], sense=Instance.MINIMIZE
    )


ROOTS = [
    (make_instance, Instance, InstanceMessage, "instance"),
    (
        lambda: make_instance().as_parametric_instance(),
        ParametricInstance,
        ParametricInstanceMessage,
        "parametric-instance",
    ),
    (lambda: make_instance().evaluate({0: 1}), Solution, SolutionMessage, "solution"),
    (
        lambda: make_instance().evaluate_samples([{0: 0}, {0: 1}]),
        SampleSet,
        SampleSetMessage,
        "sample-set",
    ),
]


@pytest.mark.parametrize("factory,root_type,message_type,kind", ROOTS)
def test_annotation_bytes_round_trip(factory, root_type, message_type, kind):
    root = factory()
    root.add_user_annotation("source", "bytes")
    root.annotations["com.example.owner"] = "Alice"
    restored = root_type.from_bytes(root.to_bytes())
    assert restored.get_user_annotation("source") == "bytes"
    assert restored.annotations["com.example.owner"] == "Alice"

    # Rust decoders must retain metadata as well as the Python wrapper.
    raw = getattr(_ommx_rust, root_type.__name__).from_bytes(root.to_bytes())
    restored_from_raw = root_type.from_bytes(raw.to_bytes())
    assert restored_from_raw.annotations == restored.annotations

    # Replacing or deleting entries must not resurrect previously decoded values.
    restored.annotations = {"com.example.new": "replacement"}
    replaced = root_type.from_bytes(restored.to_bytes())
    assert replaced.annotations == {"com.example.new": "replacement"}
    replaced.annotations.clear()
    assert root_type.from_bytes(replaced.to_bytes()).annotations == {}


@pytest.mark.parametrize("factory,root_type,message_type,kind", ROOTS[:2])
def test_instance_metadata_uses_description(factory, root_type, message_type, kind):
    root = factory()
    root.title = "Annotated model"
    root.license = "MIT"
    root.dataset = "unit-test"
    root.authors = ["Alice", "Bob"]
    root.created = datetime(2026, 10, 9, tzinfo=timezone.utc)
    root.add_user_annotation("source", "python")

    message = message_type.FromString(root.to_bytes())
    assert message.description.name == root.title
    assert message.description.license == root.license
    assert message.description.dataset == root.dataset
    assert list(message.description.authors) == root.authors
    assert message.description.created == root.created.isoformat()
    assert dict(message.annotations) == {"org.ommx.user.source": "python"}
    assert message.format_version == 0

    restored = root_type.from_bytes(root.to_bytes())
    assert restored.title == root.title
    assert restored.license == root.license
    assert restored.dataset == root.dataset
    assert restored.authors == root.authors
    assert restored.created == root.created


@pytest.mark.parametrize("factory,root_type,message_type,kind", ROOTS[2:])
def test_process_metadata_uses_metadata(factory, root_type, message_type, kind):
    root = factory()
    root.instance = "sha256:instance"
    root.solver = {"name": "solver"}
    root.parameters = {"seed": 42}
    root.start = datetime(2026, 10, 9, 0, 0, tzinfo=timezone.utc)
    root.end = datetime(2026, 10, 9, 0, 1, tzinfo=timezone.utc)
    root.add_user_annotation("source", "python")

    message = message_type.FromString(root.to_bytes())
    assert message.metadata.instance == root.instance
    assert message.metadata.solver == '{"name": "solver"}'
    assert message.metadata.parameters == '{"seed": 42}'
    assert message.metadata.start == root.start.isoformat()
    assert message.metadata.end == root.end.isoformat()
    assert dict(message.annotations) == {"org.ommx.user.source": "python"}

    restored = root_type.from_bytes(root.to_bytes())
    assert restored.instance == root.instance
    assert restored.solver == root.solver
    assert restored.parameters == root.parameters
    assert restored.start == root.start
    assert restored.end == root.end


@pytest.mark.parametrize("factory,root_type,message_type,kind", ROOTS)
def test_artifact_annotations_merge_with_payload_precedence(
    factory, root_type, message_type, kind
):
    root = factory()
    root.add_user_annotation("source", "payload")
    field = "title" if kind in ("instance", "parametric-instance") else "instance"
    setattr(root, field, "payload metadata")
    descriptor_annotations = {
        f"org.ommx.v1.{kind}.{field}": "descriptor metadata",
        "org.ommx.user.source": "descriptor",
        "com.example.legacy": "retained",
    }
    builder = ArtifactBuilder.temp()
    descriptor = builder.add_layer(
        f"application/org.ommx.v1.{kind}", root.to_bytes(), descriptor_annotations
    )
    artifact = builder.build()
    getter = getattr(artifact, "get_" + kind.replace("-", "_"))
    restored = getter(descriptor)
    assert getattr(restored, field) == "payload metadata"
    assert restored.get_user_annotation("source") == "payload"
    assert restored.annotations["com.example.legacy"] == "retained"
    assert root_type.from_bytes(restored.to_bytes()).annotations == restored.annotations


@pytest.mark.parametrize("factory,root_type,message_type,kind", ROOTS[:2])
@pytest.mark.parametrize("field", ["description", "created_by"])
@pytest.mark.parametrize("payload_value", ["protobuf metadata", ""])
def test_description_metadata_has_payload_precedence(
    factory, root_type, message_type, kind, field, payload_value
):
    message = message_type.FromString(factory().to_bytes())
    setattr(message.description, field, payload_value)
    key = f"org.ommx.v1.{kind}.{field}"
    builder = ArtifactBuilder.temp()
    descriptor = builder.add_layer(
        f"application/org.ommx.v1.{kind}",
        message.SerializeToString(),
        {key: "descriptor metadata"},
    )
    artifact = builder.build()
    restored = getattr(artifact, "get_" + kind.replace("-", "_"))(descriptor)
    assert restored.annotations[key] == payload_value
    serialized = message_type.FromString(restored.to_bytes())
    assert serialized.description.HasField(field)
    assert getattr(serialized.description, field) == payload_value


@pytest.mark.parametrize("factory,root_type,message_type,kind", ROOTS[:2])
@pytest.mark.parametrize("field", ["description", "created_by"])
def test_missing_description_metadata_uses_descriptor_fallback(
    factory, root_type, message_type, kind, field
):
    message = message_type.FromString(factory().to_bytes())
    message.description.ClearField(field)
    key = f"org.ommx.v1.{kind}.{field}"
    builder = ArtifactBuilder.temp()
    descriptor = builder.add_layer(
        f"application/org.ommx.v1.{kind}",
        message.SerializeToString(),
        {key: "descriptor metadata"},
    )
    artifact = builder.build()
    restored = getattr(artifact, "get_" + kind.replace("-", "_"))(descriptor)
    assert restored.annotations[key] == "descriptor metadata"
    serialized = message_type.FromString(restored.to_bytes())
    assert serialized.description.HasField(field)
    assert getattr(serialized.description, field) == "descriptor metadata"


@pytest.mark.parametrize("factory,root_type,message_type,kind", ROOTS)
def test_legacy_descriptor_only_annotations(factory, root_type, message_type, kind):
    root = factory()
    message = message_type.FromString(root.to_bytes())
    message.ClearField("annotations")
    message.ClearField(
        "description" if kind in ("instance", "parametric-instance") else "metadata"
    )
    annotations = {"org.ommx.user.source": "legacy"}
    builder = ArtifactBuilder.temp()
    descriptor = builder.add_layer(
        f"application/org.ommx.v1.{kind}", message.SerializeToString(), annotations
    )
    artifact = builder.build()
    restored = getattr(artifact, "get_" + kind.replace("-", "_"))(descriptor)
    assert restored.annotations == annotations
    assert root_type.from_bytes(restored.to_bytes()).annotations == annotations


@pytest.mark.parametrize("factory,root_type,message_type,kind", ROOTS)
def test_annotations_reject_unknown_reserved_metadata(
    factory, root_type, message_type, kind
):
    root = factory()
    root.annotations["org.ommx.v1.custom"] = "invalid"
    with pytest.raises(ValueError, match="reserved"):
        root.to_bytes()


def test_description_is_retained_and_can_be_overridden_by_annotations():
    root = Instance.from_components(
        objective=0,
        decision_variables=[],
        constraints=[],
        sense=Instance.MINIMIZE,
        description=Instance.Description(
            name="Original",
            description="Model details",
            authors=["Alice"],
            created_by="test",
        ),
    )
    assert root.title == "Original"
    root.title = "Updated"
    restored = Instance.from_bytes(root.to_bytes())
    assert restored.title == "Updated"
    assert restored.description is not None
    assert restored.description.name == "Updated"
    assert restored.description.description == "Model details"
    assert restored.description.created_by == "test"


@pytest.mark.parametrize(
    "description_type", [Instance.Description, InstanceMessage.Description]
)
@pytest.mark.parametrize("empty", [False, True])
def test_description_constructor_preserves_new_fields(description_type, empty):
    values: dict[Literal["created", "license", "dataset"], str] = {
        "created": "" if empty else "2026-10-09T00:00:00Z",
        "license": "" if empty else "MIT",
        "dataset": "" if empty else "unit-test",
    }
    root = Instance.from_components(
        objective=0,
        decision_variables=[],
        constraints=[],
        sense=Instance.MINIMIZE,
        description=description_type(name="Model", **values),
    )
    assert root.description is not None
    for field, value in values.items():
        assert getattr(root.description, field) == value
    for data in (root.raw.to_bytes(), root.to_bytes()):
        description = InstanceMessage.FromString(data).description
        for field, value in values.items():
            assert description.HasField(field)
            assert getattr(description, field) == value


@pytest.mark.parametrize("factory,root_type,message_type,kind", ROOTS[:2])
@pytest.mark.parametrize(
    "authors", [["Doe, Jane", "Bob"], [""], ["", "Doe, Jane", ""], ["Alice", "Bob"]]
)
def test_protobuf_author_entries_survive_round_trip(
    factory, root_type, message_type, kind, authors
):
    message = message_type.FromString(factory().to_bytes())
    message.description.authors.extend(authors)
    root = root_type.from_bytes(message.SerializeToString())
    restored = message_type.FromString(root.to_bytes())
    assert list(restored.description.authors) == authors


@pytest.mark.parametrize("factory,root_type,message_type,kind", ROOTS[:2])
def test_legacy_author_annotation_can_be_changed_or_deleted(
    factory, root_type, message_type, kind
):
    message = message_type.FromString(factory().to_bytes())
    message.description.authors.append("Doe, Jane")
    root = root_type.from_bytes(message.SerializeToString())
    root.authors = ["Carol", "Dan"]
    assert list(message_type.FromString(root.to_bytes()).description.authors) == [
        "Carol",
        "Dan",
    ]
    del root.annotations[f"org.ommx.v1.{kind}.authors"]
    assert list(message_type.FromString(root.to_bytes()).description.authors) == []


@pytest.mark.parametrize("factory,root_type,message_type,kind", ROOTS[:2])
@pytest.mark.parametrize("authors", [[], [""], ["Doe, Jane", "Bob"]])
def test_explicit_author_list_overrides_ambiguous_protobuf_entries(
    factory, root_type, message_type, kind, authors
):
    message = message_type.FromString(factory().to_bytes())
    message.description.authors.append("")
    root = root_type.from_bytes(message.SerializeToString())
    root.authors = authors
    assert root.annotations[f"org.ommx.v1.{kind}.authors"] == ",".join(authors)
    assert list(message_type.FromString(root.to_bytes()).description.authors) == authors
    materialized = (
        root.as_parametric_instance().with_parameters({})
        if isinstance(root, Instance)
        else root.with_parameters({})
    )
    assert (
        list(InstanceMessage.FromString(materialized.to_bytes()).description.authors)
        == authors
    )
    root.annotations[f"org.ommx.v1.{kind}.authors"] = "Carol,Dan"
    assert list(message_type.FromString(root.to_bytes()).description.authors) == [
        "Carol",
        "Dan",
    ]


def test_model_operations_preserve_annotations():
    root = make_instance()
    root.title = "Annotated model"
    root.add_user_annotation("source", "original")
    partial = root.partial_evaluate({0: 0})
    assert partial.title == root.title
    assert partial.get_user_annotation("source") == "original"

    for parametric in (
        root.as_parametric_instance(),
        root.penalty_method(),
        root.uniform_penalty_method(),
    ):
        assert parametric.title == root.title
        assert parametric.get_user_annotation("source") == "original"
        assert "org.ommx.v1.instance.title" not in parametric.annotations
        restored = ParametricInstance.from_bytes(parametric.to_bytes())
        materialized = restored.with_parameters({})
        assert materialized.title == root.title
        assert materialized.get_user_annotation("source") == "original"
        assert "org.ommx.v1.parametric-instance.title" not in materialized.annotations

    partial.add_user_annotation("source", "changed")
    assert root.get_user_annotation("source") == "original"


def test_sample_projection_preserves_annotations():
    sample_set = make_instance().evaluate_samples([{0: 0}, {0: 1}])
    sample_set.instance = "sha256:instance"
    sample_set.solver = {"name": "sampler"}
    sample_set.add_user_annotation("source", "samples")
    for solution in (
        sample_set.get(0),
        sample_set.best_feasible,
        sample_set.best_feasible_relaxed,
        sample_set.best_feasible_unrelaxed,
    ):
        assert solution.instance == sample_set.instance
        assert solution.solver == sample_set.solver
        assert solution.get_user_annotation("source") == "samples"
        assert "org.ommx.v1.sample-set.instance" not in solution.annotations
        assert (
            Solution.from_bytes(solution.to_bytes()).annotations == solution.annotations
        )


@pytest.mark.parametrize("factory,root_type,message_type,kind", ROOTS)
def test_reserved_keys_in_extension_map_are_rejected(
    factory, root_type, message_type, kind
):
    message = message_type.FromString(factory().to_bytes())
    message.annotations["org.ommx.v1.custom"] = "invalid"
    error_type = ValueError if root_type is ParametricInstance else RuntimeError
    with pytest.raises(error_type, match="reserved"):
        root_type.from_bytes(message.SerializeToString())


def test_constructor_preserves_mutable_annotation_dictionary():
    annotations = {"com.example.owner": "Alice"}
    root = Instance(make_instance().raw, annotations)
    assert root.annotations is annotations
    annotations["com.example.owner"] = "Bob"
    assert Instance.from_bytes(root.to_bytes()).annotations == annotations
