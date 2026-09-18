"""The installed producer stub must expose the receiving SDK's concrete class."""

from typing import assert_type

import bridge_test_modeling as modeling
import ommx
from ommx import v1


def accept_legacy_instance(instance: v1.Instance) -> v1.Solution:
    return instance.evaluate({0: 1})


one_hot = modeling.compile_one_hot([1.0])
assert_type(one_hot, ommx.Instance)
assert_type(one_hot, v1.Instance)
assert_type(accept_legacy_instance(one_hot), v1.Solution)
assert_type(modeling.compile_sos1([1.0], [(0.0, 2.0)]), ommx.Instance)
assert_type(modeling.compile_absolute_objective([1.0]), ommx.Instance)
