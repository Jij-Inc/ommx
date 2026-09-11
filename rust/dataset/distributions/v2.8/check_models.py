"""Check generated QPLIB Artifacts against the official GAMS models.

Uses restricted arithmetic parsing, without executing GAMS or solving. Checks
0018 and 0681 at eight states each and verifies the variable domains of 3525.
"""

import argparse
import ast
import csv
import hashlib
import json
import math
from pathlib import Path
import re
import sys
import zipfile

from ommx.artifact import Artifact, get_image_dir, set_local_registry_root
from ommx.v1 import DecisionVariable, Instance

sys.setrecursionlimit(20000)


def add(a, b, scale=1.0):
    out = a.copy()
    for key, value in b.items():
        out[key] = out.get(key, 0.0) + scale * value
    return {key: value for key, value in out.items() if value != 0.0}


def multiply(a, b):
    out = {}
    for ka, va in a.items():
        for kb, vb in b.items():
            key = tuple(sorted(ka + kb))
            assert len(key) <= 2, "Oracle accepts polynomials of degree <= 2 only"
            out[key] = out.get(key, 0.0) + va * vb
    return out


def arithmetic(node):
    if isinstance(node, ast.Constant) and type(node.value) in (int, float):
        return {(): float(node.value)}
    if isinstance(node, ast.Name):
        return {(node.id,): 1.0}
    if isinstance(node, ast.UnaryOp):
        sign = {ast.UAdd: 1.0, ast.USub: -1.0}[type(node.op)]
        return {k: sign * v for k, v in arithmetic(node.operand).items()}
    if isinstance(node, ast.BinOp):
        a, b = arithmetic(node.left), arithmetic(node.right)
        if isinstance(node.op, ast.Add):
            return add(a, b)
        if isinstance(node.op, ast.Sub):
            return add(a, b, -1.0)
        if isinstance(node.op, ast.Mult):
            return multiply(a, b)
    if (
        isinstance(node, ast.Call)
        and isinstance(node.func, ast.Name)
        and node.func.id == "sqr"
        and len(node.args) == 1
        and not node.keywords
    ):
        a = arithmetic(node.args[0])
        return multiply(a, a)
    raise ValueError(f"Unsupported oracle expression: {ast.dump(node)}")


def expression(text):
    return arithmetic(ast.parse(" ".join(text.split()), mode="eval").body)


def gams_model(text):
    declaration = re.search(r"^Variables\s+(.*?);", text, re.M | re.S).group(1)
    names = re.findall(r"[A-Za-z]\w*", declaration)
    assert names.pop(0) == "objvar"
    # Verify the positional mapping instead of guessing it from sparse .sol rows.
    assert all(
        int(re.search(r"\d+$", name).group()) == i + 2 for i, name in enumerate(names)
    )
    equations = {}
    for num, left, relation, right in re.findall(
        r"^e(\d+)\.\.(.*?)=([EGL])=(.*?);", text, re.M | re.S
    ):
        equations[int(num)] = (add(expression(left), expression(right), -1.0), relation)
    objective, relation = equations.pop(1)
    assert relation == "E"
    obj_coefficient = objective.pop(("objvar",))
    assert obj_coefficient in (-1.0, 1.0)
    objective = {key: -value / obj_coefficient for key, value in objective.items()}
    assert all("objvar" not in key for key in objective)
    assert sorted(equations) == list(range(2, len(equations) + 2))
    return names, objective, [equations[i] for i in sorted(equations)]


def evaluate(poly, named_state, degree=None):
    return math.fsum(
        coeff * math.prod(named_state[name] for name in key)
        for key, coeff in poly.items()
        if degree is None or len(key) == degree
    )


def close(a, b, tol=1e-9):
    return (
        math.isfinite(a)
        and math.isfinite(b)
        and abs(a - b) <= tol * max(1.0, abs(a), abs(b))
    )


def digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--registry", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    set_local_registry_root(args.registry)
    with args.report.open() as f:
        rows = {r["name"]: r for r in csv.DictReader(f)}
    proofs = {}
    with zipfile.ZipFile(args.archive) as archive:
        members = {Path(name).name: name for name in archive.namelist()}

        def source(tag, suffix):
            return archive.read(members[f"QPLIB_{tag}.{suffix}"])

        def load(tag):
            row = rows[f"QPLIB_{tag}"]
            assert row["status"] in ("packaged", "published")
            artifact = Artifact.load_archive(get_image_dir(row["image"]))
            descriptor = artifact.get_layer_descriptor(row["instance_digest"])
            raw = artifact.get_blob(descriptor)
            assert digest(raw) == row["instance_digest"]
            instance = Instance.from_bytes(raw)
            proofs[tag] = {
                "instance_digest": row["instance_digest"],
                "source_qplib_digest": digest(source(tag, "qplib")),
                "source_gams_digest": digest(source(tag, "gms")),
            }
            return instance

        for tag in ("0018", "0681"):
            instance = load(tag)
            names, objective, constraints = gams_model(source(tag, "gms").decode())
            n, m = len(names), len(constraints)
            assert n == len(instance.decision_variables)
            sol_raw = source(tag, "sol")
            values = dict(
                line.split() for line in sol_raw.decode().splitlines() if line.strip()
            )
            assert set(values) <= set(names) | {"objvar"}
            published = {
                i: float(values.get(name, "0")) for i, name in enumerate(names)
            }
            states = [
                dict.fromkeys(range(n), 0.0),
                {i: float(i == 0) for i in range(n)},
                {i: -float(i == 0) for i in range(n)},
                {i: float(i == 1) for i in range(n)},
                {i: float(i in (0, 1)) for i in range(n)},
                {i: (((i * 7 + 3) % 19) - 9) / 8 for i in range(n)},
                {i: (((i * 11 + 5) % 23) - 11) / 7 for i in range(n)},
                published,
            ]
            expected_constraints = {}
            for i, (poly, relation) in enumerate(constraints):
                if relation in ("E", "L"):
                    expected_constraints[i] = poly
                if relation in ("E", "G"):
                    expected_constraints[m + i] = {
                        key: -value for key, value in poly.items()
                    }
            actual_constraints = {c.id: c for c in instance.constraints}
            assert set(actual_constraints) == set(expected_constraints)
            comparisons, max_error = 0, 0.0
            for state in states:
                named = {name: state[i] for i, name in enumerate(names)}
                checks = [
                    (instance.objective.evaluate(state), evaluate(objective, named))
                ]
                checks += [
                    (
                        actual_constraints[i].function.evaluate(state),
                        evaluate(poly, named),
                    )
                    for i, poly in expected_constraints.items()
                ]
                for actual, expected in checks:
                    assert close(actual, expected), (tag, actual, expected)
                    comparisons += 1
                    max_error = max(max_error, abs(actual - expected))
            solution = instance.evaluate(published, atol=1e-8)
            assert solution.feasible
            assert close(solution.objective, float(values["objvar"]))
            proofs[tag].update(
                {
                    "source_solution_digest": digest(sol_raw),
                    "states": len(states),
                    "comparisons": comparisons,
                    "max_absolute_error": max_error,
                    "published_objective": float(values["objvar"]),
                    "artifact_objective": solution.objective,
                    "artifact_feasible_at_1e8": solution.feasible,
                }
            )

        instance = load("3525")
        gams = source("3525", "gms").decode()
        integer_decl = re.search(r"^Integer Variables\s+(.*?);", gams, re.M | re.S)
        integer_names = set(re.findall(r"[A-Za-z]\w*", integer_decl.group(1)))
        assert len(integer_names) == 1662
        all_decl = re.search(r"^Variables\s+(.*?);", gams, re.M | re.S)
        names = re.findall(r"[A-Za-z]\w*", all_decl.group(1))
        assert names.pop(0) == "objvar"
        assert all(
            int(re.search(r"\d+$", name).group()) == i + 2
            for i, name in enumerate(names)
        )
        # GAMS integer variables default to [0, +inf]; free variables to [-inf, +inf].
        # https://www.gams.com/latest/docs/UG_Variables.html#UG_Variables_VariableTypes
        bounds = {
            name: [0.0 if name in integer_names else -math.inf, math.inf]
            for name in names
        }
        for name, side, value in re.findall(
            r"\b([xi]\d+)\.(lo|up|fx)\s*=\s*([^;]+);", gams
        ):
            value = float(value)
            if side == "fx":
                bounds[name] = [value, value]
            else:
                bounds[name][side == "up"] = value
        variables = {v.id: v for v in instance.decision_variables}
        assert len(variables) == len(names) == 1749
        binaries = 0
        for i, name in enumerate(names):
            v = variables[i]
            assert (v.kind in (DecisionVariable.BINARY, DecisionVariable.INTEGER)) == (
                name in integer_names
            )
            assert [v.bound.lower, v.bound.upper] == bounds[name], (
                name,
                v.bound,
                bounds[name],
            )
            if v.kind == DecisionVariable.BINARY:
                assert name in integer_names and bounds[name] == [0.0, 1.0]
                binaries += 1
        assert binaries == 831
        proofs["3525"].update(
            {
                "variables_compared": len(names),
                "source_integer_variables": len(integer_names),
                "integer_variables_with_binary_bounds": binaries,
            }
        )
    args.output.write_text(json.dumps(proofs, indent=2) + "\n")
    print(json.dumps(proofs, indent=2))


if __name__ == "__main__":
    main()
