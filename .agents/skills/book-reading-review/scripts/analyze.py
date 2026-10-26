#!/usr/bin/env python3
"""Check declared Book learning contracts, not the natural-language prose.

A section teaches its ordered ``introduces`` before evaluating its ``uses``.
Earlier use must therefore be represented by an earlier section. ``mentions``
never teaches; ``requires`` and route ``initial`` are exact known sets, without
implicit prerequisite closure. Optional child pages do not inherit knowledge.

Schema errors raise ValueError (CLI exit 2). Unknown or duplicate identifiers,
dependency cycles, and learning-contract violations are findings (CLI exit 1).
Only ``analyze(manifest)`` and ``main(argv=None)`` are public entry points.
"""

import argparse
from collections import Counter
import json
from pathlib import Path
import sys

__all__ = ["analyze", "main"]


def analyze(manifest):
    """Return JSON-serializable catalog, page-contract, and route diagnostics.

    The input must declare schema_version=1 and concepts/pages/routes arrays.
    Concepts and pages cannot be empty; routes=[] permits isolated-page checks.
    Unknown metadata fields are preserved by the caller but not interpreted.
    """
    if not isinstance(manifest, dict):
        raise ValueError("The manifest must be a JSON object.")
    if (
        type(manifest.get("schema_version")) is not int
        or manifest["schema_version"] != 1
    ):
        raise ValueError("schema_version must be the integer 1.")
    for name in ("concepts", "pages", "routes"):
        if name not in manifest or not isinstance(manifest[name], list):
            raise ValueError(f"{name}: Expected a required array.")
        if name != "routes" and not manifest[name]:
            raise ValueError(f"{name}: Expected a nonempty array.")

    findings = []
    invalid = {"concepts": set(), "pages": set(), "routes": set()}

    def report(code, location, message, *, entity=None, **details):
        findings.append(
            {
                "severity": "error",
                "code": code,
                "location": location,
                "message": message,
                **details,
            }
        )
        if entity is not None:
            kind, identifier = entity
            invalid[kind].add(identifier)

    def array(value, location):
        if not isinstance(value, list):
            raise ValueError(f"{location}: Expected an array.")
        return value

    def required_array(record, name, location):
        if name not in record:
            raise ValueError(f"{location}.{name}: Expected a required array.")
        return array(record[name], location + "." + name)

    def index(name):
        result = {}
        for n, item in enumerate(manifest[name]):
            location = f"{name}[{n}]"
            if not isinstance(item, dict) or not isinstance(item.get("id"), str):
                raise ValueError(f"{location}: Expected an object with a string id.")
            key = item["id"]
            if not key or key in result:
                report(
                    "duplicate_or_empty_id",
                    location,
                    "Entity ids must be nonempty and unique.",
                    entity=(name, key),
                    id=key,
                )
                continue
            result[key] = item
        return result

    concepts, pages, routes = index("concepts"), index("pages"), index("routes")

    def references(values, location, entity):
        known = []
        for n, value in enumerate(array(values, location)):
            here = f"{location}[{n}]"
            if not isinstance(value, str):
                raise ValueError(f"{here}: Expected a string concept id.")
            if value not in concepts:
                report(
                    "unknown_concept",
                    here,
                    "Unknown concept reference.",
                    entity=entity,
                    reference=value,
                )
            else:
                known.append(value)
        return known

    dependencies = {}
    for concept_id, concept in concepts.items():
        location = f"concepts.{concept_id}"
        dependencies[concept_id] = references(
            required_array(concept, "requires", location),
            location + ".requires",
            ("concepts", concept_id),
        )
        owner = concept.get("owner")
        if owner is not None and not isinstance(owner, str):
            raise ValueError(f"{location}.owner: Expected a string page id or null.")
        if owner is not None and owner not in pages:
            report(
                "unknown_owner_page",
                location + ".owner",
                "Unknown owner page reference.",
                entity=("concepts", concept_id),
                reference=owner,
            )

    # Concept-graph invalidity propagates to dependents, not to reader knowledge.
    color, stack = {}, []

    def visit(concept_id):
        color[concept_id] = 1
        stack.append(concept_id)
        for required in dependencies[concept_id]:
            if color.get(required) == 1:
                cycle = stack[stack.index(required) :] + [required]
                report(
                    "concept_cycle",
                    f"concepts.{concept_id}.requires",
                    "Concept prerequisites contain a cycle.",
                    entity=("concepts", concept_id),
                    cycle=cycle,
                )
                invalid["concepts"].update(cycle)
            elif not color.get(required):
                visit(required)
        stack.pop()
        color[concept_id] = 2

    for concept_id in concepts:
        if not color.get(concept_id):
            visit(concept_id)
    changed = True
    while changed:
        changed = False
        for concept_id, required in dependencies.items():
            if concept_id not in invalid["concepts"] and any(
                dependency in invalid["concepts"] for dependency in required
            ):
                invalid["concepts"].add(concept_id)
                changed = True

    def validate_graph_references(values, location, entity):
        for concept_id in values:
            if concept_id in invalid["concepts"]:
                report(
                    "invalid_concept_reference",
                    location,
                    "The referenced concept has an invalid catalog or prerequisite graph.",
                    entity=entity,
                    concept=concept_id,
                )

    normalized = {}
    for page_id, page in pages.items():
        location = f"pages.{page_id}"
        entity = ("pages", page_id)
        parent = page.get("parent_page")
        if parent is not None and not isinstance(parent, str):
            raise ValueError(
                f"{location}.parent_page: Expected a string page id or null."
            )
        if parent is not None and parent not in pages:
            report(
                "unknown_parent_page",
                location + ".parent_page",
                "Unknown parent page reference.",
                entity=entity,
                reference=parent,
            )
        requires = references(
            required_array(page, "requires", location), location + ".requires", entity
        )
        deferred = references(
            required_array(page, "deferred", location), location + ".deferred", entity
        )
        validate_graph_references(requires, location + ".requires", entity)
        validate_graph_references(deferred, location + ".deferred", entity)
        sections, section_ids = [], set()
        for n, section in enumerate(required_array(page, "sections", location)):
            section_location = f"{location}.sections[{n}]"
            if not isinstance(section, dict) or not isinstance(section.get("id"), str):
                raise ValueError(
                    f"{section_location}: Expected a section with a string id."
                )
            section_id = section["id"]
            if not section_id or section_id in section_ids:
                report(
                    "duplicate_or_empty_id",
                    section_location,
                    "Section ids must be nonempty and unique within a page.",
                    entity=entity,
                    id=section_id,
                )
            section_ids.add(section_id)
            normalized_section = {"id": section_id}
            for kind in ("introduces", "uses", "mentions"):
                values = references(
                    required_array(section, kind, section_location),
                    section_location + "." + kind,
                    entity,
                )
                validate_graph_references(values, section_location + "." + kind, entity)
                normalized_section[kind] = values
            sections.append(normalized_section)
        normalized[page_id] = {
            "requires": requires,
            "deferred": set(deferred),
            "sections": sections,
        }

    def validate_steps(raw_steps, location, route_id):
        steps = []
        for n, step in enumerate(array(raw_steps, location)):
            here = f"{location}[{n}]"
            if isinstance(step, str):
                if step not in pages:
                    report(
                        "unknown_page",
                        here,
                        "Unknown route page reference.",
                        entity=("routes", route_id),
                        reference=step,
                    )
                steps.append(step)
            elif isinstance(step, dict) and "alternatives" in step:
                alternatives = array(step["alternatives"], here + ".alternatives")
                if not alternatives:
                    raise ValueError(
                        f"{here}: A branch requires at least one alternative."
                    )
                steps.append(
                    {
                        "alternatives": [
                            validate_steps(
                                branch, f"{here}.alternatives[{i}]", route_id
                            )
                            for i, branch in enumerate(alternatives)
                        ]
                    }
                )
            else:
                raise ValueError(
                    f"{here}: A step must be a page id or an alternatives object."
                )
        return steps

    normalized_routes = {}
    for route_id, route in routes.items():
        location = f"routes.{route_id}"
        initial = references(
            required_array(route, "initial", location),
            location + ".initial",
            ("routes", route_id),
        )
        validate_graph_references(initial, location + ".initial", ("routes", route_id))
        normalized_routes[route_id] = {
            "initial": initial,
            "steps": validate_steps(
                required_array(route, "steps", location), location + ".steps", route_id
            ),
        }
    catalog_consistent = not findings

    def evaluate_page(page_id, initial, location, route_id=None):
        page, known, exports = normalized[page_id], set(initial), set()
        entity = ("pages", page_id) if route_id is None else ("routes", route_id)
        for section in page["sections"]:
            here = location + ".sections." + section["id"]
            for concept_id in section["introduces"]:
                if concept_id in page["deferred"]:
                    report(
                        "deferred_introduction",
                        here,
                        "A deferred concept is introduced on this page.",
                        entity=entity,
                        concept=concept_id,
                    )
                    continue
                if concept_id in invalid["concepts"]:
                    continue
                missing = sorted(set(dependencies[concept_id]) - known)
                if missing:
                    report(
                        "introduction_prerequisite_missing",
                        here,
                        "Introduction prerequisites are not known at this point.",
                        entity=entity,
                        concept=concept_id,
                        missing=missing,
                    )
                else:
                    known.add(concept_id)
                    exports.add(concept_id)
            for concept_id in section["uses"]:
                if concept_id in page["deferred"]:
                    report(
                        "deferred_use",
                        here,
                        "A deferred concept is used on this page.",
                        entity=entity,
                        concept=concept_id,
                    )
                if concept_id not in known:
                    report(
                        "unavailable_use",
                        here,
                        "The concept is used before it is known.",
                        entity=entity,
                        concept=concept_id,
                    )
        return known, exports

    page_results = []
    for page_id, page in normalized.items():
        known, exports = evaluate_page(page_id, page["requires"], "pages." + page_id)
        page_results.append(
            {
                "id": page_id,
                "status": pages[page_id].get("status"),
                "contract_consistent": page_id not in invalid["pages"],
                "requires": sorted(set(page["requires"])),
                "exported_concepts": sorted(exports),
                "knowledge_at_end": sorted(known),
                "body_verification": "not_performed",
            }
        )

    route_results = []
    for route_id, route in normalized_routes.items():
        planned = set()

        def walk(steps, initial, location):
            known, trace = set(initial), []
            for n, step in enumerate(steps):
                here = f"{location}[{n}]"
                if isinstance(step, str):
                    if step not in normalized:
                        trace.append({"page": step, "unknown": True})
                        continue
                    if step in invalid["pages"]:
                        report(
                            "route_invalid_page",
                            here,
                            "The route visits an invalid page contract.",
                            entity=("routes", route_id),
                            page=step,
                        )
                    if "planned" in str(pages[step].get("status", "")).lower():
                        planned.add(step)
                    missing = sorted(set(normalized[step]["requires"]) - known)
                    if missing:
                        report(
                            "route_prerequisite_missing",
                            here,
                            "The route does not supply this page's entry knowledge.",
                            entity=("routes", route_id),
                            page=step,
                            missing=missing,
                        )
                    before = set(known)
                    known, exports = evaluate_page(
                        step, known, here + ".page." + step, route_id
                    )
                    trace.append(
                        {
                            "page": step,
                            "missing_entry_concepts": missing,
                            "exported_concepts": sorted(exports),
                            "new_knowledge": sorted(known - before),
                        }
                    )
                else:
                    branch_results = [
                        walk(branch, known, f"{here}.alternatives[{i}]")
                        for i, branch in enumerate(step["alternatives"])
                    ]
                    if branch_results:
                        known = set.intersection(
                            *(result[0] for result in branch_results)
                        )
                    trace.append(
                        {
                            "alternatives": [result[1] for result in branch_results],
                            "knowledge_after_join": sorted(known),
                        }
                    )
            return known, trace

        final, trace = walk(
            route["steps"], route["initial"], f"routes.{route_id}.steps"
        )
        route_results.append(
            {
                "id": route_id,
                "contract_consistent": route_id not in invalid["routes"],
                "initial": sorted(set(route["initial"])),
                "final_knowledge": sorted(final),
                "trace": trace,
                "planned_pages": sorted(planned),
                "implementation_verified": False,
                "implementation_status": "contains_planned_pages"
                if planned
                else "not_assessed",
                "body_verification": "not_performed",
            }
        )

    return {
        "analysis_scope": {
            "subject": "declared_target_contracts",
            "body_verification": "not_performed",
            "runtime_verification": "not_performed",
            "section_order": "ordered introductions, then uses; mentions confer no knowledge",
            "knowledge_model": "exact declared sets; no implicit prerequisite or parent-page closure",
            "route_failure_note": "Final knowledge on an inconsistent route is diagnostic, not a guarantee.",
        },
        "contract_consistent": not findings,
        "catalog_consistent": catalog_consistent,
        "counts": {
            "concepts": len(concepts),
            "pages": len(pages),
            "routes": len(routes),
            "sections": sum(len(p["sections"]) for p in normalized.values()),
            "errors": len(findings),
            "warnings": 0,
            "consistent_pages": sum(p["contract_consistent"] for p in page_results),
            "consistent_routes": sum(r["contract_consistent"] for r in route_results),
            "findings_by_code": dict(
                sorted(Counter(f["code"] for f in findings).items())
            ),
        },
        "findings": findings,
        "pages": page_results,
        "routes": route_results,
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args(argv)
    try:
        result = analyze(json.loads(args.manifest.read_text(encoding="utf-8")))
        rendered = json.dumps(result, ensure_ascii=False, indent=2) + "\n"
        if args.output:
            args.output.write_text(rendered, encoding="utf-8")
        else:
            print(rendered, end="")
    except (OSError, ValueError) as error:
        print(f"analyze.py: {error}", file=sys.stderr)
        return 2
    return 0 if result["contract_consistent"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
