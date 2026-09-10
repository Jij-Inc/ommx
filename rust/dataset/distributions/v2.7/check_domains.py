"""Compare MPS and Artifact domains where MIPLIB's variable classification differs.

Run with the v2 project environment and local Artifacts. No optimization or
presolve is performed: HiGHS only reads the original MPS variable domains.
"""

import argparse
import csv
import gzip
import hashlib
import json
from pathlib import Path
import tempfile
import zipfile

import highspy
from ommx.artifact import Artifact, get_image_dir, set_local_registry_root
from ommx.v1 import Instance, DecisionVariable

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--archive", type=Path, required=True)
parser.add_argument("--report", type=Path, required=True)
parser.add_argument("--registry", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
set_local_registry_root(args.registry)
repository = Path(__file__).resolve().parents[4]
fields = {
    "variables": "VariablesVari.",
    "binaries": "BinariesBina.",
    "integers": "IntegersInte.",
    "continuous": "ContinuousCont.",
}
with args.report.open() as f:
    rows = [r for r in csv.DictReader(f) if r["status"] in ("packaged", "published")]
with (repository / "rust/ommx/src/dataset/miplib2017.csv").open() as f:
    metadata = {r["InstanceInst."]: r for r in csv.DictReader(f)}
proof_path = args.output
proofs = {}
for row in rows:
    name = row["name"]
    source_counts = {
        key: int(float(metadata[name][field])) for key, field in fields.items()
    }
    parsed_counts = {key: int(row[key]) for key in fields}
    if source_counts == parsed_counts:
        continue
    assert parsed_counts["variables"] == source_counts["variables"]
    assert parsed_counts["continuous"] == source_counts["continuous"]
    assert (
        parsed_counts["binaries"] + parsed_counts["integers"]
        == source_counts["binaries"] + source_counts["integers"]
    )
    with zipfile.ZipFile(args.archive) as archive:
        raw_mps = archive.read(name + ".mps.gz")
    source_digest = "sha256:" + hashlib.sha256(raw_mps).hexdigest()
    image_name = f"ghcr.io/jij-inc/ommx/v2.7/miplib2017:{name}"
    artifact = Artifact.load_archive(get_image_dir(image_name))
    descriptor = artifact.get_layer_descriptor(row["instance_digest"])
    assert descriptor.media_type == "application/org.ommx.v1.instance"
    raw_instance = artifact.get_blob(descriptor)
    assert (
        "sha256:" + hashlib.sha256(raw_instance).hexdigest() == row["instance_digest"]
    )
    instance = Instance.from_bytes(raw_instance)
    variables = {variable.name: variable for variable in instance.decision_variables}
    highs = highspy.Highs()
    highs.setOptionValue("output_flag", False)
    with tempfile.TemporaryDirectory(prefix="source-domains-") as directory:
        source_path = Path(directory) / (name + ".mps")
        source_path.write_bytes(gzip.decompress(raw_mps))
        assert highs.readModel(str(source_path)) == highspy.HighsStatus.kOk
    model = highs.getLp()
    column_names = model.col_names_
    integrality = model.integrality_
    lower_bounds = model.col_lower_
    upper_bounds = model.col_upper_
    assert len(variables) == model.num_col_ == parsed_counts["variables"]
    assert set(variables) == set(column_names)
    unit_integers = 0
    for i, name_in_model in enumerate(column_names):
        variable = variables[name_in_model]
        assert variable.kind in (
            DecisionVariable.BINARY,
            DecisionVariable.INTEGER,
            DecisionVariable.CONTINUOUS,
        )
        source_integer = integrality[i] == highspy.HighsVarType.kInteger
        assert integrality[i] in (
            highspy.HighsVarType.kInteger,
            highspy.HighsVarType.kContinuous,
        )
        parsed_integer = variable.kind in (
            DecisionVariable.BINARY,
            DecisionVariable.INTEGER,
        )
        assert source_integer == parsed_integer, (name, name_in_model, "integrality")
        assert variable.bound.lower == lower_bounds[i], (
            name,
            name_in_model,
            "lower bound",
        )
        assert variable.bound.upper == upper_bounds[i], (
            name,
            name_in_model,
            "upper bound",
        )
        if variable.kind == DecisionVariable.BINARY:
            assert source_integer and 0 <= lower_bounds[i] <= upper_bounds[i] <= 1, (
                name,
                name_in_model,
                "binary domain",
            )
        unit_integers += (
            source_integer and lower_bounds[i] == 0 and upper_bounds[i] == 1
        )
    proofs[name] = {
        "domain_check_version": 1,
        "source_mps_digest": source_digest,
        "instance_digest": row["instance_digest"],
        "metadata_counts": source_counts,
        "parsed_counts": parsed_counts,
        "independent_reader": "HiGHS " + highs.version(),
        "variables_compared": len(variables),
        "source_integer_variables_with_bounds_0_1": unit_integers,
    }
    print(name, json.dumps(proofs[name]))
proof_path.write_text(json.dumps(proofs, indent=2) + "\n")
