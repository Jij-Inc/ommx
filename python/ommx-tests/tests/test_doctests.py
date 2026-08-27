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

# Number of hermetic setup examples before each owner reaches an external
# archive, remote registry, or the caller's persistent Local Registry. Every
# example from that boundary onward must carry an explicit doctest SKIP.
NON_HERMETIC_SETUP_EXAMPLES = {
    "ommx.artifact.Artifact": 0,
    "ommx.artifact.Artifact.import_archive": 0,
    "ommx.artifact.Artifact.inspect_archive": 0,
    "ommx.artifact.Artifact.load": 0,
    "ommx.artifact.ArtifactDraft": 0,
    "ommx.artifact.ArtifactDraft.new": 5,
    "ommx.artifact.ArtifactDraft.new_anonymous": 3,
    "ommx.artifact.ArtifactDraft.temp": 0,
    "ommx.artifact.ArtifactDraft.add_instance": 3,
    "ommx.artifact.ArtifactDraft.add_ndarray": 2,
    "ommx.artifact.ArtifactDraft.add_dataframe": 2,
    "ommx.artifact.ArtifactDraft.add_json": 1,
    "ommx.artifact.gc": 1,
    "ommx.artifact.prune_anonymous": 1,
    "ommx.dataset.miplib2017": 1,
    "ommx.dataset.qplib": 1,
    "ommx.experiment.Experiment": 5,
    "ommx.experiment.Experiment.fork": 0,
    "ommx.experiment.Experiment.run": 0,
}


@pytest.mark.parametrize("test", DOCTESTS, ids=lambda test: test.name)
def test_doctest(test: doctest.DocTest) -> None:
    doctest.DebugRunner(optionflags=doctest.ELLIPSIS).run(test)


def test_non_hermetic_examples_are_explicitly_skipped() -> None:
    tests_by_name = {test.name: test for test in DOCTESTS}
    skipped_owners = {
        test.name
        for test in DOCTESTS
        if any(example.options.get(doctest.SKIP, False) for example in test.examples)
    }
    assert skipped_owners == NON_HERMETIC_SETUP_EXAMPLES.keys()

    for name, setup_count in NON_HERMETIC_SETUP_EXAMPLES.items():
        examples = tests_by_name[name].examples
        assert all(
            not example.options.get(doctest.SKIP, False)
            for example in examples[:setup_count]
        )
        assert examples[setup_count:]
        assert all(
            example.options.get(doctest.SKIP, False)
            for example in examples[setup_count:]
        )
