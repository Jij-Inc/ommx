import mip
import pytest

from ommx.testing import SingleFeasibleLPGenerator, DataType

import ommx_python_mip_adapter as adapter

from ommx_python_mip_adapter.exception import OMMXPythonMIPAdapterError


def test_error_not_optimized_model():
    model = mip.Model()

    with pytest.raises(OMMXPythonMIPAdapterError) as e:
        adapter.model_to_solution(model, b"")
    assert "`model.status` must be " in str(e.value)


def test_error_invalid_ommx_instance_bytes():
    generator = SingleFeasibleLPGenerator(10, DataType.INT)
    ommx_instance_bytes = generator.get_v1_instance()
    model = adapter.instance_to_model(ommx_instance_bytes)
    model.optimize()

    with pytest.raises(OMMXPythonMIPAdapterError) as e:
        adapter.model_to_solution(model, b"invalid")
    assert "Invalid `ommx_instance_bytes`" in str(e.value)
