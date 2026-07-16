"""Static return-type assertions for the downstream bridge consumer."""

import ommx
from ommx_pyo3_bridge_fixture import (
    constraint,
    decision_variable,
    function,
    instance,
)
from typing_extensions import assert_type


assert_type(function(), ommx.Function)
assert_type(constraint(), ommx.Constraint)
assert_type(decision_variable(), ommx.DecisionVariable)
assert_type(instance(), ommx.Instance)
