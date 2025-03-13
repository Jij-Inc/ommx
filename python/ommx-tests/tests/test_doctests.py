import doctest
import ommx
import pkgutil
import importlib


def test_doctest():
    result = doctest.testmod(ommx, optionflags=doctest.ELLIPSIS)
    assert result.failed == 0
    # type: ignore
    for mod in pkgutil.iter_modules(ommx.__path__):
        mod = importlib.import_module(f"ommx.{mod.name}")
        result = doctest.testmod(mod, optionflags=doctest.ELLIPSIS)
        assert result.failed == 0
