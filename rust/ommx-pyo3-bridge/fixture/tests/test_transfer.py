"""Negotiated transfers through a separately compiled PyO3 consumer."""

import sys
from types import ModuleType

import ommx
import ommx._ommx_rust as receiver
import ommx_pyo3_bridge_fixture as fixture
import pytest

from test_bridge import (
    assert_component_constraint,
    assert_component_decision_variable,
    assert_component_function,
    assert_component_parametric_instance,
    assert_component_sample_set,
    assert_component_solution,
)


INSTANCE = ommx.Instance
DECLARATION = "_bridge_supported_protocols"
V1_INSTANCE = "_bridge_protobuf_v1_instance_from_bytes"
V2_INSTANCE = "_bridge_protobuf_v2_instance_from_bytes"


def advertise(monkeypatch, ids):
    monkeypatch.setattr(receiver, DECLARATION, lambda: ids)


def spy_instance(monkeypatch):
    calls = []
    original_v1 = getattr(receiver, V1_INSTANCE)
    original_v2 = getattr(receiver, V2_INSTANCE)

    def v1(payload):
        calls.append((1, payload))
        return original_v1(payload)

    def v2(payload):
        calls.append((2, payload))
        return original_v2(payload)

    monkeypatch.setattr(receiver, V1_INSTANCE, v1)
    monkeypatch.setattr(receiver, V2_INSTANCE, v2)
    return calls


def test_python_sdk_explicitly_advertises_fixed_protocol_ids():
    assert getattr(receiver, DECLARATION)() == [1, 2]
    endpoints = [DECLARATION] + [
        f"_bridge_protobuf_v{version}_{kind}_from_bytes"
        for version in (1, 2)
        for kind in (
            "function",
            "constraint",
            "decision_variable",
            "instance",
            "parametric_instance",
            "solution",
            "sample_set",
        )
    ]
    for endpoint in endpoints:
        assert callable(getattr(receiver, endpoint))
        assert not hasattr(ommx, endpoint)


@pytest.mark.parametrize(
    ("advertised", "expected"),
    [([1, 2], 2), ([2, 1], 2), ([1], 1), ([2], 2), ([999, 1], 1), ([2, 2], 2)],
)
def test_sender_order_selects_protocol(monkeypatch, advertised, expected):
    advertise(monkeypatch, advertised)
    calls = spy_instance(monkeypatch)
    value = fixture.negotiated_instance()
    assert type(value) is INSTANCE
    assert value.objective.linear_terms == {7: 1.0}
    assert value.decision_variables[0].name == "instance_x"
    assert [protocol for protocol, _ in calls] == [expected]


def test_caller_can_prefer_v1_even_when_v2_is_available(monkeypatch):
    calls = spy_instance(monkeypatch)
    fixture.v1_first_instance()
    assert [protocol for protocol, _ in calls] == [1]


@pytest.mark.parametrize("ids", [[], [999]])
def test_no_common_protocol_fails_before_compilation(monkeypatch, ids):
    advertise(monkeypatch, ids)
    compiled = []
    with pytest.raises(ImportError, match="No supported OMMX transfer protocol"):
        fixture.negotiated_instance(lambda: compiled.append(True))
    assert compiled == []


def test_missing_declaration_is_not_inferred_from_existing_methods(monkeypatch):
    monkeypatch.delattr(receiver, DECLARATION)
    with pytest.raises(
        ImportError, match="must declare supported transfer protocols"
    ) as error:
        fixture.negotiated_instance()
    assert isinstance(error.value.__cause__, AttributeError)


@pytest.mark.parametrize("ids", [None, [-1], ["ProtobufV1"]])
def test_malformed_declaration_preserves_cause(monkeypatch, ids):
    advertise(monkeypatch, ids)
    with pytest.raises(
        ImportError, match="must declare supported transfer protocols"
    ) as error:
        fixture.negotiated_instance()
    assert error.value.__cause__ is not None


@pytest.mark.parametrize("missing", [True, False])
def test_broken_receiver_fails_at_transfer_without_falling_back(monkeypatch, missing):
    calls = []
    monkeypatch.setattr(receiver, V1_INSTANCE, lambda _: calls.append(1))
    if missing:
        monkeypatch.delattr(receiver, V2_INSTANCE)
    else:
        monkeypatch.setattr(receiver, V2_INSTANCE, None)
    with pytest.raises(ImportError, match="ProtobufV2"):
        fixture.negotiated_instance(lambda: calls.append("compiled"))
    # A protocol probe does not inspect type-specific receivers. The compiler
    # runs, then transfer reports the broken receiver instead of trying v1.
    assert calls == ["compiled"]


@pytest.mark.parametrize("ids", [[1], [1, 2]])
def test_selected_protocol_and_sdk_module_are_retained(monkeypatch, ids):
    calls = spy_instance(monkeypatch)
    declarations = []

    def declaration():
        declarations.append(True)
        return ids

    monkeypatch.setattr(receiver, DECLARATION, declaration)

    def change_after_negotiation():
        advertise(monkeypatch, [])
        monkeypatch.setitem(sys.modules, "ommx", ModuleType("ommx"))

    result = fixture.negotiated_instance(change_after_negotiation)
    assert type(result) is INSTANCE
    # One probe for v2; only its absence leads to a second probe for v1.
    assert declarations == [True] * (1 if 2 in ids else 2)
    assert [protocol for protocol, _ in calls] == [ids[-1]]


def test_probe_errors_are_not_treated_as_unsupported_protocols(monkeypatch):
    calls = []
    original = RuntimeError("cannot read supported protocols")

    def declaration():
        calls.append(True)
        if len(calls) == 1:
            raise original
        return [1]

    monkeypatch.setattr(receiver, DECLARATION, declaration)
    with pytest.raises(ImportError, match="must declare") as error:
        fixture.negotiated_instance()
    assert error.value.__cause__ is original
    assert calls == [True]


def test_receiver_is_resolved_at_transfer(monkeypatch):
    calls = []
    original = getattr(receiver, V2_INSTANCE)

    def after_resolve():
        def receive(payload):
            calls.append(2)
            return original(payload)

        monkeypatch.setattr(receiver, V2_INSTANCE, receive)

    value = fixture.negotiated_instance(after_resolve)
    assert type(value) is INSTANCE
    assert calls == [2]


def test_one_target_transfers_multiple_types_without_reprobing(monkeypatch):
    calls = []

    def declaration():
        calls.append(True)
        return [2]

    monkeypatch.setattr(receiver, DECLARATION, declaration)
    function, constraint, variable = fixture.v2_components()
    assert_component_function(function)
    assert_component_constraint(constraint)
    assert_component_decision_variable(variable)
    assert calls == [True]


def test_completed_output_returns_the_received_python_object(monkeypatch):
    received = []
    original = getattr(receiver, V2_INSTANCE)

    def receive(payload):
        value = original(payload)
        received.append(value)
        return value

    monkeypatch.setattr(receiver, V2_INSTANCE, receive)

    def after_transfer():
        # Import is complete while the Rust function is still executing.
        assert len(received) == 1
        monkeypatch.delattr(receiver, V2_INSTANCE)
        monkeypatch.delattr(receiver, DECLARATION)

    value = fixture.completed_instance(after_transfer)
    assert type(value) is INSTANCE
    assert value is received[0]
    assert value.objective.linear_terms == {7: 1.0}
    assert len(received) == 1


def test_compilation_uses_selected_target_and_preserves_v1_hints(monkeypatch):
    value = fixture.compile_for_target()
    assert set(value.one_hot_constraints) == {23}
    assert value.constraints == {}

    advertise(monkeypatch, [1])
    calls = spy_instance(monkeypatch)
    legacy = fixture.compile_for_target()
    assert set(legacy.constraints) == {23}
    assert legacy.one_hot_constraints == {}
    assert len(calls) == 1
    assert calls[0][0] == 1
    # Check the complete wire message, before the v3 parser ignores advisory hints.
    assert fixture.has_legacy_hint(calls[0][1])
    for state in ({7: 0.0}, {7: 1.0}):
        assert legacy.evaluate(state).feasible == value.evaluate(state).feasible


def test_v1_export_error_does_not_retry_v2(monkeypatch):
    calls = spy_instance(monkeypatch)
    with pytest.raises(
        RuntimeError, match="ommx.Instance using ProtobufV1 during export"
    ) as error:
        fixture.v1_first_instance(special=True)
    assert error.value.__cause__ is not None
    assert "one-hot" in str(error.value.__cause__).lower()
    assert calls == []


def test_receiver_exception_is_preserved_as_cause_without_retry(monkeypatch):
    original = ValueError("receiver rejected the payload")
    calls = []

    def fail(_):
        calls.append(2)
        raise original

    monkeypatch.setattr(receiver, V2_INSTANCE, fail)
    monkeypatch.setattr(receiver, V1_INSTANCE, lambda _: calls.append(1))
    with pytest.raises(
        RuntimeError, match="ommx.Instance using ProtobufV2 during import"
    ) as error:
        fixture.negotiated_instance()
    assert error.value.__cause__ is original
    assert calls == [2]


def test_invalid_raw_root_is_rejected_by_the_receiver_parser():
    with pytest.raises(
        RuntimeError, match="ommx.Instance using ProtobufV1 during import"
    ) as error:
        fixture.invalid_instance()
    assert error.value.__cause__ is not None


def test_v1_components_reconstruct_canonical_types():
    assert_component_function(fixture.v1_function())
    assert_component_decision_variable(fixture.v1_decision_variable())
    constraint = fixture.v1_constraint()
    assert type(constraint) is ommx.Constraint
    assert constraint.name == "choice"
    assert constraint.function.linear_terms == {7: 1.0}
    assert constraint.function.constant_term == -1.0
    assert constraint.equality == ommx.Equality.EqualToZero


def test_fixed_value_stays_with_its_root_owner():
    with pytest.raises(
        RuntimeError, match="ommx.DecisionVariable using ProtobufV1 during import"
    ) as error:
        fixture.v1_decision_variable(fixed=True)
    assert "transfer its Instance" in str(error.value.__cause__)


def test_v2_components_reconstruct_canonical_types():
    function, constraint, variable = fixture.v2_components()
    assert_component_function(function)
    assert_component_constraint(constraint)
    assert_component_decision_variable(variable)


@pytest.mark.parametrize("ids", [[1], [2]])
def test_all_root_types_support_both_protocols(monkeypatch, ids):
    advertise(monkeypatch, ids)
    assert_component_parametric_instance(fixture.negotiated_parametric_instance())
    assert_component_solution(fixture.negotiated_solution())
    assert_component_sample_set(fixture.negotiated_sample_set())


@pytest.mark.parametrize("ids", [[1], [2]])
def test_root_transfer_does_not_look_up_public_classes_or_decoders(monkeypatch, ids):
    advertise(monkeypatch, ids)
    roots = [
        ("Instance", fixture.negotiated_instance),
        ("ParametricInstance", fixture.negotiated_parametric_instance),
        ("Solution", fixture.negotiated_solution),
        ("SampleSet", fixture.negotiated_sample_set),
    ]
    classes = {name: getattr(ommx, name) for name, _ in roots}
    for name, _ in roots:
        monkeypatch.delattr(ommx, name)
        monkeypatch.delattr(receiver, name)
    for name, transfer in roots:
        value = transfer()
        assert type(value) is classes[name]
        # Use the preserved class only to compare the complete reconstructed root.
        restored = classes[name].from_v2_bytes(value.to_v2_bytes())
        assert restored.to_v2_bytes() == value.to_v2_bytes()


@pytest.mark.parametrize("version", [1, 2])
@pytest.mark.parametrize(
    "kind", ["instance", "parametric_instance", "solution", "sample_set"]
)
def test_malformed_root_is_a_bridge_runtime_error(version, kind):
    receive = getattr(receiver, f"_bridge_protobuf_v{version}_{kind}_from_bytes")
    with pytest.raises(
        RuntimeError, match=f"invalid OMMX ProtobufV{version} bridge payload"
    ):
        receive(b"\xff")


@pytest.mark.parametrize("kind", ["function", "constraint", "decision_variable"])
def test_malformed_v1_component_is_a_bridge_runtime_error(kind):
    receive = getattr(receiver, f"_bridge_protobuf_v1_{kind}_from_bytes")
    with pytest.raises(RuntimeError, match="invalid OMMX ProtobufV1 bridge payload"):
        receive(b"\xff")


@pytest.mark.parametrize(
    ("endpoint", "kwargs"),
    [
        ("_bridge_protobuf_v2_function_from_bytes", {"bytes": b"\xff"}),
        (
            "_bridge_protobuf_v2_constraint_from_bytes",
            {"constraint": b"\xff", "context": b""},
        ),
        (
            "_bridge_protobuf_v2_decision_variable_from_bytes",
            {"id": 7, "decision_variable": b"\xff", "label": b""},
        ),
        ("_bridge_protobuf_v1_function_from_bytes", {"bytes": b"\xff"}),
        ("_bridge_protobuf_v1_constraint_from_bytes", {"bytes": b"\xff"}),
        ("_bridge_protobuf_v1_decision_variable_from_bytes", {"bytes": b"\xff"}),
        *[
            (f"_bridge_protobuf_v{version}_{kind}_from_bytes", {"bytes": b"\xff"})
            for version in (1, 2)
            for kind in ("instance", "parametric_instance", "solution", "sample_set")
        ],
    ],
)
def test_configured_receivers_preserve_keyword_arguments(endpoint, kwargs):
    # Argument binding succeeds and the malformed bytes reach the parser.
    with pytest.raises(RuntimeError, match="invalid OMMX"):
        getattr(receiver, endpoint)(**kwargs)


def test_receiver_implementation_stays_private():
    for name in (
        "Receiver",
        "ProtocolDeclaration",
        "ReceiverConfig",
        "ProtobufV1ReceiverConfig",
        "ProtobufV2ReceiverConfig",
    ):
        assert not hasattr(receiver, name)
        assert not hasattr(ommx, name)
