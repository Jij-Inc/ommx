#!/usr/bin/env python3
"""Query a validated Book manifest and optional revision-matched source evidence."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from analyze import analyze


_SCHEMAS = {
    "source_inventory.json": "ommx-source-inventory-1",
    "observed_appearances.json": "ommx-observed-appearances-1",
    "concept_crosswalk.json": "ommx-concept-crosswalk-1",
}
_RELATIONS = {"exact", "narrower", "broader", "related", "unmapped"}
_LIMITATION = (
    "Manifest consistency is not prose or runtime verification. Source occurrences "
    "and mappings do not establish learning, prerequisites, or implementation."
)


def _object(value, where):
    if not isinstance(value, dict):
        raise ValueError(f"{where}: expected an object")
    return value


def _array(value, where):
    if not isinstance(value, list):
        raise ValueError(f"{where}: expected an array")
    return value


def _string(value, where):
    if not isinstance(value, str) or not value:
        raise ValueError(f"{where}: expected a nonempty string")
    return value


def _strings(value, where):
    return [_string(item, where) for item in _array(value, where)]


def _read_json(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def _load_evidence(manifest, evidence_dir):
    baseline = _object(manifest.get("baseline"), "manifest.baseline")
    revision = _string(baseline.get("source_revision"), "baseline.source_revision")
    documents = {}
    for filename, schema in _SCHEMAS.items():
        document = _object(_read_json(Path(evidence_dir) / filename), filename)
        if document.get("schema_version") != schema:
            raise ValueError(f"{filename}: unsupported schema_version")
        if document.get("source_commit") != revision:
            raise ValueError(
                f"{filename}: source_commit does not match baseline.source_revision {revision!r}"
            )
        documents[filename] = document

    sources = {}
    for index, source in enumerate(
        _array(
            documents["source_inventory.json"].get("pages"), "source inventory.pages"
        )
    ):
        where = f"source inventory.pages[{index}]"
        source = _object(source, where)
        path = _string(source.get("path"), where + ".path")
        _string(source.get("source_url"), where + ".source_url")
        if path in sources:
            raise ValueError(f"{where}: duplicate source path {path!r}")
        sources[path] = source

    concept_ids = {concept["id"] for concept in manifest["concepts"]}
    mappings = {}
    for index, mapping in enumerate(
        _array(documents["concept_crosswalk.json"].get("entries"), "crosswalk.entries")
    ):
        where = f"crosswalk.entries[{index}]"
        mapping = _object(mapping, where)
        key = (
            _string(mapping.get("audit"), where + ".audit"),
            _string(mapping.get("native_concept"), where + ".native_concept"),
        )
        if key in mappings:
            raise ValueError(f"{where}: duplicate crosswalk key {key!r}")
        relation = _string(mapping.get("relation"), where + ".relation")
        if relation not in _RELATIONS:
            raise ValueError(f"{where}: unknown mapping relation {relation!r}")
        targets = _strings(mapping.get("target_concepts"), where + ".target_concepts")
        if (relation == "unmapped") != (not targets):
            raise ValueError(
                f"{where}: only unmapped entries may have no target concepts"
            )
        for target in targets:
            if target not in concept_ids:
                raise ValueError(f"{where}: unknown target concept {target!r}")
        if "note" in mapping and not isinstance(mapping["note"], str):
            raise ValueError(f"{where}.note: expected a string")
        mappings[key] = mapping

    observations, seen_ids = [], set()
    for index, appearance in enumerate(
        _array(documents["observed_appearances.json"].get("appearances"), "appearances")
    ):
        where = f"appearances[{index}]"
        appearance = _object(appearance, where)
        identifier = _string(appearance.get("id"), where + ".id")
        if identifier in seen_ids:
            raise ValueError(f"{where}: duplicate observation id {identifier!r}")
        seen_ids.add(identifier)
        path = _string(appearance.get("path"), where + ".path")
        if path not in sources:
            raise ValueError(f"{where}: source path missing from inventory: {path!r}")
        key = (
            _string(appearance.get("audit"), where + ".audit"),
            _string(appearance.get("native_concept_id"), where + ".native_concept_id"),
        )
        if key not in mappings:
            raise ValueError(f"{where}: missing crosswalk entry {key!r}")
        _string(
            appearance.get("qualified_native_concept_id"),
            where + ".qualified_native_concept_id",
        )
        _strings(appearance.get("native_roles"), where + ".native_roles")
        for position in _array(
            appearance.get("source_positions"), where + ".source_positions"
        ):
            position = _object(position, where + ".source_positions[]")
            kind = _string(position.get("kind"), where + ".source_positions[].kind")
            start, end = position.get("start"), position.get("end")
            if (
                kind not in {"line", "cell"}
                or type(start) is not int
                or type(end) is not int
                or not 1 <= start <= end
            ):
                raise ValueError(f"{where}: invalid line/cell source position")
        mapping = mappings[key]
        # Preserve native fields and mapping strength, not inventory title guesses.
        observations.append(
            {
                **appearance,
                "source_url": sources[path]["source_url"],
                "mapping_relation": mapping["relation"],
                "mapping_targets": mapping["target_concepts"],
                "mapping_note": mapping.get("note", ""),
            }
        )
    return revision, observations


def query(manifest, kind, identifier, *, evidence_dir=None):
    """Return a contract, with source annotations only when explicitly requested.

    Invalid manifests, unknown IDs, or incompatible evidence raise ValueError.
    The optional evidence directory is never resolved relative to this script.
    """
    result = analyze(manifest)
    if not result["contract_consistent"]:
        raise ValueError(
            "Manifest contracts are inconsistent; run analyze.py for diagnostics"
        )
    if kind not in {"concept", "page"}:
        raise ValueError(f"Unknown query kind: {kind!r}")
    pages = {page["id"]: page for page in manifest["pages"]}
    for page_id, page in pages.items():
        _string(page.get("title"), f"pages.{page_id}.title")

    if kind == "concept":
        concept = next(
            (item for item in manifest["concepts"] if item["id"] == identifier), None
        )
        if concept is None:
            raise ValueError(f"Unknown concept: {identifier}")
        locations = []
        for page in pages.values():
            for role in ("requires", "deferred"):
                if identifier in page.get(role, []):
                    locations.append(
                        {"page": page["id"], "title": page["title"], "role": role}
                    )
            for section in page.get("sections", []):
                for role in ("introduces", "uses", "mentions"):
                    if identifier in section.get(role, []):
                        locations.append(
                            {
                                "page": page["id"],
                                "title": page["title"],
                                "section": section["id"],
                                "role": role,
                            }
                        )
        response = {"concept": concept, "target_contract_locations": locations}
    else:
        if identifier not in pages:
            raise ValueError(f"Unknown page/section contract: {identifier}")
        page = pages[identifier]
        response = {"target_page_contract": page}

    actual_evidence = {
        "status": "not_assessed",
        "reason": "Source evidence is not read unless --evidence-dir is explicitly supplied.",
    }
    if evidence_dir is not None:
        revision, observations = _load_evidence(manifest, evidence_dir)
        if kind == "concept":
            matches = [
                item for item in observations if identifier in item["mapping_targets"]
            ]
            attribution = "Related source annotations; mapping strength and native roles are preserved."
        else:
            source_paths = _strings(
                page.get("source_paths", []), f"pages.{identifier}.source_paths"
            )
            matches = [item for item in observations if item["path"] in source_paths]
            attribution = "Same-physical-source annotations are not automatically attributed to a virtual subsection."
        actual_evidence = {
            "status": "annotations_loaded",
            "source_revision": revision,
            "observations": matches,
            "attribution": attribution,
            "prose_verification": "not_performed",
            "runtime_verification": "not_performed",
        }
    return {**response, "actual_evidence": actual_evidence, "limitation": _LIMITATION}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("kind", choices=("concept", "page"))
    parser.add_argument("identifier")
    parser.add_argument("--evidence-dir", type=Path)
    args = parser.parse_args(argv)
    try:
        response = query(
            _read_json(args.manifest),
            args.kind,
            args.identifier,
            evidence_dir=args.evidence_dir,
        )
    except (ValueError, OSError) as error:
        parser.exit(2, f"{error}\n")
    print(json.dumps(response, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
