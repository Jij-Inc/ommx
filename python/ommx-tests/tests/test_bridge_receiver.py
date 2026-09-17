"""V1 bridge factories return the existing SDK 2.x classes without promotion."""

from typing_extensions import assert_type
from importlib import import_module

import pytest
import ommx
from ommx import _ommx_rust, v1
from ommx.v1.instance_pb2 import Instance as RawInstance
from ommx.v1.parametric_instance_pb2 import ParametricInstance as RawParametricInstance
from ommx.v1.function_pb2 import Function as RawFunction


KINDS = {
    "function": ommx.Function,
    "constraint": ommx.Constraint,
    "decision_variable": ommx.DecisionVariable,
    "instance": ommx.Instance,
    "parametric_instance": ommx.ParametricInstance,
    "solution": ommx.Solution,
    "sample_set": ommx.SampleSet,
}


def receive(kind, data):
    return getattr(_ommx_rust, f"_bridge_protobuf_v1_{kind}_from_bytes")(data)


def example():
    x = ommx.DecisionVariable.binary(7, name="x", subscripts=[3])
    row = (x == 1).set_id(23)
    instance = ommx.Instance.from_components(
        decision_variables=[x],
        objective=x - 3,
        constraints=[row],
        sense=ommx.Instance.MINIMIZE,
        constraint_hints=v1.ConstraintHints(
            one_hot_constraints=[v1.OneHot(id=23, variables=[7])]
        ),
    )
    return x, row, instance


def test_registration_and_canonical_aliases():
    assert getattr(_ommx_rust, "_bridge_supported_protocols")() == [1]
    assert getattr(_ommx_rust, "BridgeError") is ommx.BridgeError
    for kind, cls in KINDS.items():
        assert cls is getattr(v1, cls.__name__)
        assert callable(getattr(_ommx_rust, f"_bridge_protobuf_v1_{kind}_from_bytes"))
        assert not hasattr(_ommx_rust, f"_bridge_protobuf_v2_{kind}_from_bytes")
    assert_type(ommx.Function(0), v1.Function)
    assert_type(ommx.DecisionVariable.binary(0), v1.DecisionVariable)
    assert_type(ommx.BridgeError(), ommx.BridgeError)


def test_all_types_use_canonical_wrappers_and_preserve_payloads():
    x, row, instance = example()
    parametric = ommx.ParametricInstance.from_components(
        decision_variables=[x],
        parameters=[],
        objective=x,
        constraints=[row],
        sense=ommx.Instance.MINIMIZE,
    )
    parametric.raw.constraint_hints.CopyFrom(
        RawInstance.FromString(instance.to_bytes()).constraint_hints
    )
    values = {
        "function": ommx.Function(x - 3),
        "constraint": row,
        "decision_variable": x,
        "instance": instance,
        "parametric_instance": parametric,
        "solution": instance.evaluate({7: 1}),
        "sample_set": instance.evaluate_samples([{7: 0}, {7: 1}]),
    }
    for kind, value in values.items():
        restored = receive(kind, value.to_bytes())
        assert type(restored) is KINDS[kind]
        module = "decision_variables" if kind == "decision_variable" else kind
        message = getattr(import_module(f"ommx.v1.{module}_pb2"), type(value).__name__)
        assert message.FromString(restored.to_bytes()) == message.FromString(
            value.to_bytes()
        )
    restored = receive("instance", instance.to_bytes())
    assert restored.constraint_hints.one_hot_constraints == [
        v1.OneHot(id=23, variables=[7])
    ]
    assert [c.id for c in restored.constraints] == [23]


def test_detached_constraint_preserves_identity_and_advances_allocator():
    _, row, _ = example()
    row.set_id(v1.Constraint._counter + 100)
    restored = receive("constraint", row.to_bytes())
    assert restored.id == row.id
    assert (
        v1.Constraint(function=0, equality=v1.Constraint.EQUAL_TO_ZERO).id > restored.id
    )


@pytest.mark.parametrize("kind", KINDS)
def test_malformed_payload_is_bridge_error_with_cause(kind):
    with pytest.raises(ommx.BridgeError) as error:
        receive(kind, b"\xff")
    assert error.value.__cause__ is not None


def test_unknown_function_variant_is_rejected():
    # An unrecognized length-delimited Function oneof arm.
    with pytest.raises(ommx.BridgeError):
        receive("function", b"\x2a\x00")


@pytest.mark.parametrize(
    "kind,raw_type",
    [("instance", RawInstance), ("parametric_instance", RawParametricInstance)],
)
@pytest.mark.parametrize(
    "location", ["objective", "constraint", "removed", "dependency", "named"]
)
def test_nested_unknown_functions_are_rejected(kind, raw_type, location):
    raw = raw_type(sense=RawInstance.SENSE_MINIMIZE, objective=RawFunction(constant=0))
    raw.decision_variables.add().CopyFrom(ommx.DecisionVariable.binary(7).to_protobuf())
    unsupported = RawFunction.FromString(b"\x2a\x00")
    if location == "objective":
        raw.objective.CopyFrom(unsupported)
    elif location == "constraint":
        raw.constraints.add(id=23, equality=1).function.CopyFrom(unsupported)
    elif location == "removed":
        row = raw.removed_constraints.add(removed_reason="test").constraint
        row.id, row.equality = 23, 1
        row.function.CopyFrom(unsupported)
    elif location == "dependency":
        raw.decision_variable_dependency[7].CopyFrom(unsupported)
    else:
        raw.named_functions.add(id=8).function.CopyFrom(unsupported)
    with pytest.raises(ommx.BridgeError):
        receive(kind, raw.SerializeToString())


def test_public_decoders_are_not_the_bridge_contract(monkeypatch):
    _, _, instance = example()
    monkeypatch.setattr(
        ommx.Instance, "from_bytes", lambda _: pytest.fail("public decoder called")
    )
    assert type(receive("instance", instance.to_bytes())) is ommx.Instance
