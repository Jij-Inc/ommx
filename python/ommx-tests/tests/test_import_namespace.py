import importlib

import pytest

from ommx import DecisionVariable, Function, Instance


def test_domain_classes_are_importable_from_top_level() -> None:
    assert Instance.__name__ == "Instance"
    assert DecisionVariable.__name__ == "DecisionVariable"
    assert Function.__name__ == "Function"


def test_ommx_v1_import_raises_migration_message() -> None:
    with pytest.raises(ImportError, match="ommx\\.v1 no longer exports"):
        importlib.import_module("ommx.v1")
