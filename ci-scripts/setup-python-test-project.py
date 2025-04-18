# /// script
# requires-python = ">=3.9"
# dependencies = [
#     "tomlkit",
#     "packaging",
#     "ruamel.yaml",
# ]
# ///
#
# Used in test ci in Python Workflows
# This do the following things:
#
# 1. Determine the wheel to be used from the argument
#   - If the version is suffixed with `t', non-ABI, version-specific wheel is used
#   - Otherwise, ABI3 wheel is used
# 2. If --use_local_ommx is NOT specified:
#   - Update the `ommx` source in the root `pyproject.toml` to point to the wheel
#   - Remove `ommx` from the `workspace.members` in the root `pyproject.toml`.
# 4. Filters out package not supported by the python version and warns about it.
# 4. Write the updated `pyproject.toml` back to the file.
# 5. Tweaks python:test-ci Taskfile so that the CI doesn't run for the unsupported package.
#   + NOTE: with free-threaded pythons, it currently tests OMMX only, as the
#     adapter dependencies tend to provide ABI3 wheels only, which is unsupported with free-threaded pythons.
import tomlkit
import sys
import glob
from pathlib import Path
from argparse import ArgumentParser
from packaging.specifiers import SpecifierSet
import re
from ruamel.yaml import YAML

FREE_THREAD_PACKAGES = {"ommx", "ommx-tests"}


ap = ArgumentParser()
ap.add_argument("version", type=str, help="Python version")
ap.add_argument("--use_local_ommx", action="store_true")
args = ap.parse_args()

use_local_ommx: bool = args.use_local_ommx
full_version: str = args.version
t = re.compile(r"t$")
free_thread = t.search(full_version) is not None
version = t.sub("", full_version)
print(f"Version: {version}, Free Thread: {free_thread}")
if args.version[-1] == "t":
    short_ver = version.replace(".", "")
    pat = f"ommx-*cp{short_ver}-cp{short_ver}t*.whl"
else:
    pat = "ommx-*abi3*.whl"


def check_version(version: str, free_thread: bool, dir: Path) -> bool:
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
        if free_thread and dir.name not in FREE_THREAD_PACKAGES:
            print(f"::warning::Excluding {dir} for free-threaded python")
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

sources = uv["sources"]
if not isinstance(sources, dict):
    raise KeyError("Expected tool.uv.sources table in pyproject.toml")

if not use_local_ommx:
    whl = list(Path("wheels").glob(pat))
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
    if not check_version(version, free_thread, member):
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
    if free_thread:
        # When free-threaded python is used, only ommx is tested.
        if i["task"].split(":")[0] in FREE_THREAD_PACKAGES:
            new_cmds.append(i)
        else:
            print(
                f"::warning file={taskfile}::Excluding test {i['task']} for free-threaded python",
            )
    else:
        if i["task"].split(":")[0] not in excludeds:
            new_cmds.append(i)
dic["tasks"]["test-ci"]["cmds"] = new_cmds


with open(taskfile, "w") as f:
    yaml.dump(dic, f)
