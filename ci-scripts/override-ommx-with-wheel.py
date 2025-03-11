# /// script
# requires-python = ">=3.9"
# dependencies = [
#     "tomlkit",
# ]
# ///
#
# Used in test ci in Python Workflows
import tomlkit
from tomlkit.items import Table
import sys
import glob
from pathlib import Path

whl = list(Path("wheels").glob("ommx-*abi3*.whl"))
if len(whl) != 1:
    print(
        f"Expected to find exactly one wheel in the wheels directory, \
        but got: {whl}"
    )
    sys.exit(1)

whl = whl[0]
print(f"Found wheel: {whl}")

with open("pyproject.toml") as f:
    pyproject = tomlkit.parse(f.read())

tool = pyproject["tool"]
if not isinstance(tool, Table):
    raise KeyError("Expected tool table in pyproject.toml")

uv = tool["uv"]
if not isinstance(uv, Table):
    raise KeyError("Expected tool.uv table in pyproject.toml")

sources = uv["sources"]
if not isinstance(sources, Table):
    raise KeyError("Expected tool.uv.sources table in pyproject.toml")

sources["ommx"] = {"path": str(whl)}

workspace = uv["workspace"]
if not isinstance(workspace, Table):
    raise KeyError("Expected tool.uv.workspace table in pyproject.toml")

old_members = workspace["members"]

if not isinstance(old_members, list):
    raise KeyError("Expected tool.uv.workspace.members to be a list")

new_members = [
    targ for pat in old_members for targ in glob.glob(pat) if targ != "python/ommx"
]
workspace["members"] = new_members

with open("pyproject.toml", "w") as f:
    f.write(tomlkit.dumps(pyproject))
