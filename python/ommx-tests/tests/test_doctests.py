import doctest
import importlib
import inspect
import pkgutil
from collections.abc import Iterator
from types import ModuleType

import ommx
import pytest


def iter_package_modules() -> Iterator[ModuleType]:
    yield ommx
    for module_info in pkgutil.walk_packages(ommx.__path__, prefix="ommx."):
        if module_info.name == "ommx._ommx_rust":
            continue
        if module_info.name == "ommx.v1" or module_info.name.startswith("ommx.v1."):
            continue
        yield importlib.import_module(module_info.name)


def iter_docstring_owners() -> Iterator[tuple[str, object, ModuleType]]:
    """Yield each package-owned or publicly re-exported docstring owner once."""
    seen: set[int] = set()

    def walk(
        owner_path: str, owner: object, module: ModuleType
    ) -> Iterator[tuple[str, object, ModuleType]]:
        if id(owner) in seen:
            return
        seen.add(id(owner))
        yield owner_path, owner, module

        if not inspect.isclass(owner):
            return

        for member_name, raw_member in vars(owner).items():
            if not (
                inspect.isroutine(raw_member)
                or inspect.isdatadescriptor(raw_member)
                or inspect.isclass(raw_member)
            ):
                continue
            if isinstance(raw_member, (classmethod, staticmethod)):
                member = raw_member.__func__
            else:
                member = raw_member
            yield from walk(f"{owner_path}.{member_name}", member, module)

    for module in iter_package_modules():
        module_name = module.__name__
        yield from walk(module_name, module, module)

        for export_name in getattr(module, "__all__", ()):
            yield from walk(
                f"{module_name}.{export_name}", getattr(module, export_name), module
            )

        for owner_name, owner in vars(module).items():
            if not (inspect.isclass(owner) or inspect.isroutine(owner)):
                continue
            if getattr(owner, "__module__", None) != module_name:
                continue
            yield from walk(f"{module_name}.{owner_name}", owner, module)


def parse_docstring(
    owner_path: str, owner: object, module: ModuleType
) -> doctest.DocTest | None:
    raw_doc = getattr(owner, "__doc__", None)
    if not isinstance(raw_doc, str):
        return None
    doc = inspect.cleandoc(raw_doc)
    test = doctest.DocTestParser().get_doctest(
        doc,
        dict(vars(module)),
        owner_path,
        f"<docstring {owner_path}>",
        0,
    )
    return test if test.examples else None


DOCTESTS = tuple(
    test
    for owner_path, owner, module in iter_docstring_owners()
    if (test := parse_docstring(owner_path, owner, module)) is not None
)


@pytest.mark.parametrize("test", DOCTESTS, ids=lambda test: test.name)
def test_doctest(test: doctest.DocTest) -> None:
    doctest.DebugRunner(optionflags=doctest.ELLIPSIS).run(test)
