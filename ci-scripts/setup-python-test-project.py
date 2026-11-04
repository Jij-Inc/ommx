# Used in test ci in Python Workflows
# This do the following things:
#
# 1. Determine the wheel to be used from the argument
#   - Free-threaded Python 3.14 uses a version-specific wheel
#   - Python 3.15+ (GIL-enabled or free-threaded) uses the ABI3t wheel
#   - Older GIL-enabled Python uses the ABI3 wheel
# 2. If --use_local_ommx is NOT specified:
#   - Update the `ommx` source in the root `pyproject.toml` to point to the wheel
#   - Remove `ommx` from the `workspace.members` in the root `pyproject.toml`.
# 4. Filters out packages not supported by the Python version or test scope.
# 4. Write the updated `pyproject.toml` back to the file.
# 5. Tweaks python:test-ci Taskfile so that the CI doesn't run for the unsupported package.
#   + Free-threaded jobs run the core SDK suite; adapters used by SDK doctests
#     remain available as test dependencies.
import tomlkit
import sys
import glob
from pathlib import Path
from argparse import ArgumentParser
from packaging.specifiers import SpecifierSet
from packaging.utils import parse_wheel_filename
import re
from ruamel.yaml import YAML

SDK_PACKAGES = {"ommx", "ommx-tests"}
# SDK doctests cover Experiment solves and comparisons between these adapters.
SDK_TEST_DEPENDENCIES = {"ommx-highs-adapter", "ommx-pyscipopt-adapter"}


ap = ArgumentParser()
ap.add_argument("version", type=str, help="Python version")
ap.add_argument("--use_local_ommx", action="store_true")
ap.add_argument("--sdk-only", action="store_true", help="Test only the core SDK")
args = ap.parse_args()

use_local_ommx: bool = args.use_local_ommx
full_version: str = args.version
t = re.compile(r"t$")
free_thread = t.search(full_version) is not None
sdk_only = args.sdk_only or free_thread
version = t.sub("", full_version)
print(f"Version: {version}, Free Thread: {free_thread}")
if tuple(map(int, version.split("."))) >= (3, 15):
    abis = {"abi3", "abi3t"}
elif free_thread:
    short_ver = version.replace(".", "")
    abis = {f"cp{short_ver}t"}
else:
    abis = {"abi3"}


def check_version(version: str, dir: Path) -> bool:
    with open(dir / "pyproject.toml") as f:
        pyproject = tomlkit.parse(f.read())
        project = pyproject["project"]
        if not isinstance(project, dict):
            print(f"No project table: {project}", file=sys.stderr)
            return False
        req_py = project.get("requires-python")
        if not isinstance(req_py, str):
            print(f"No project.requires-python: {pyproject}", file=sys.stderr)
            return False
        spec = SpecifierSet(req_py)
        return version in spec


with open("pyproject.toml") as f:
    pyproject = tomlkit.parse(f.read())

tool = pyproject["tool"]
if not isinstance(tool, dict):
    raise KeyError("Expected tool table in pyproject.toml")

uv = tool["uv"]
if not isinstance(uv, dict):
    raise KeyError("Expected tool.uv table in pyproject.toml")

# This is a disposable CI workspace for one interpreter. In particular, a
# version-specific free-threaded wheel cannot resolve on other Python minors.
uv["environments"] = [f"python_version == '{version}'"]

if sdk_only:
    # Keep Sphinx for SDK rendering tests, but do not install dependencies used
    # only by executable Book tutorials (which are tested in separate jobs).
    groups = pyproject["dependency-groups"]
    del groups["tutorials"]
    groups["dev"] = [
        group for group in groups["dev"] if group != {"include-group": "tutorials"}
    ]

sources = uv["sources"]
if not isinstance(sources, dict):
    raise KeyError("Expected tool.uv.sources table in pyproject.toml")

if not use_local_ommx:
    whl = [
        wheel
        for wheel in Path("wheels").glob("ommx-*.whl")
        if {tag.abi for tag in parse_wheel_filename(wheel.name)[3]} == abis
    ]
    if len(whl) != 1:
        print(
            f"Expected to find exactly one wheel in the wheels directory, \
            but got: {whl}"
        )
        sys.exit(1)

    whl = whl[0]
    print(f"Found wheel: {whl}")
    sources["ommx"] = {"path": str(whl)}

workspace = uv["workspace"]
if not isinstance(workspace, dict):
    raise KeyError("Expected tool.uv.workspace table in pyproject.toml")

old_members = workspace["members"]

if not isinstance(old_members, list):
    raise KeyError("Expected tool.uv.workspace.members to be a list")

member_candidates = [
    targ
    for pat in old_members
    for targ in glob.glob(pat)
    if use_local_ommx or targ != "python/ommx"
]

new_members = []
excludeds = []
for member in member_candidates:
    member = Path(member)
    if sdk_only and member.name not in SDK_PACKAGES | SDK_TEST_DEPENDENCIES:
        excludeds.append(member.name)
        print(f"Excluding {member} from the SDK-only test scope")
        continue
    if not check_version(version, member):
        excludeds.append(member.name)
        print(
            f"Warning: {member} is not supported by Python {version}, and hence removed.",
            file=sys.stderr,
        )
        print(
            f"::warning file={member / 'pyproject.toml'}::{member} doesn't support Python {version}; Skipping CI."
        )
    else:
        new_members.append(str(member))

workspace["members"] = new_members

with open("pyproject.toml", "w") as f:
    f.write(tomlkit.dumps(pyproject))


# Rewriting Taskfile

taskfile = Path("python") / "Taskfile.yml"
with open(taskfile, "r") as f:
    yaml = YAML()
    dic = yaml.load(f)

tasks = dic["tasks"]["test-ci"]["cmds"]
new_cmds = []
for i in tasks:
    package = i["task"].split(":")[0]
    if package not in excludeds and (not sdk_only or package in SDK_PACKAGES):
        new_cmds.append(i)
    else:
        print(f"Excluding test {i['task']} from the selected test scope")
dic["tasks"]["test-ci"]["cmds"] = new_cmds


with open(taskfile, "w") as f:
    yaml.dump(dic, f)
