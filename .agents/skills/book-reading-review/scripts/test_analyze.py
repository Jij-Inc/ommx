"""Pure-stdlib regression tests for declared learning-flow contracts."""

from contextlib import redirect_stderr, redirect_stdout
import copy
import io
import json
from pathlib import Path
import tempfile
import unittest

from analyze import analyze, main


def concept(name, requires=()):
    return {
        "id": name,
        "label": name,
        "requires": list(requires),
        "owner": None,
        "external": True,
    }


def page(
    name,
    *,
    requires=(),
    introduces=(),
    uses=(),
    mentions=(),
    status="existing",
    **extra,
):
    return {
        "id": name,
        "title": name,
        "status": status,
        "path": name + ".md",
        "requires": list(requires),
        "sections": [
            {
                "id": "main",
                "introduces": list(introduces),
                "uses": list(uses),
                "mentions": list(mentions),
            }
        ],
        "deferred": [],
        **extra,
    }


def manifest(*, concepts, pages, routes=()):
    return {
        "schema_version": 1,
        "concepts": concepts,
        "pages": pages,
        "routes": list(routes),
    }


class AnalyzeTest(unittest.TestCase):
    # The original six learning-flow tests, with the required schema envelope.
    def test_mention_does_not_grant_knowledge(self):
        result = analyze(
            manifest(
                concepts=[concept("x")],
                pages=[
                    page("A", mentions=["x"]),
                    page("B", requires=["x"], uses=["x"]),
                ],
                routes=[{"id": "r", "initial": [], "steps": ["A", "B"]}],
            )
        )
        self.assertTrue(result["pages"][0]["contract_consistent"])
        self.assertEqual(result["pages"][0]["exported_concepts"], [])
        self.assertFalse(result["routes"][0]["contract_consistent"])
        self.assertIn(
            "route_prerequisite_missing", result["counts"]["findings_by_code"]
        )

    def test_forward_use_and_introduction_dependency(self):
        p = page("A")
        p["sections"] = [
            {"id": "early", "introduces": ["y"], "uses": ["x"], "mentions": []},
            {"id": "later", "introduces": ["x"], "uses": [], "mentions": []},
        ]
        result = analyze(
            manifest(concepts=[concept("x"), concept("y", ["x"])], pages=[p])
        )
        self.assertFalse(result["contract_consistent"])
        self.assertEqual(result["pages"][0]["exported_concepts"], ["x"])
        self.assertEqual(
            set(result["counts"]["findings_by_code"]),
            {"unavailable_use", "introduction_prerequisite_missing"},
        )

    def test_branch_join_is_intersection(self):
        result = analyze(
            manifest(
                concepts=[concept("common"), concept("left"), concept("right")],
                pages=[
                    page("A", introduces=["common", "left"]),
                    page("B", introduces=["common", "right"]),
                    page("C", requires=["common", "left"], uses=["left"]),
                ],
                routes=[
                    {
                        "id": "r",
                        "initial": [],
                        "steps": [{"alternatives": [["A"], ["B"]]}, "C"],
                    }
                ],
            )
        )
        route = result["routes"][0]
        self.assertEqual(route["trace"][0]["knowledge_after_join"], ["common"])
        self.assertFalse(route["contract_consistent"])
        self.assertEqual(route["trace"][1]["missing_entry_concepts"], ["left"])

    def test_unknown_references_and_cycle(self):
        result = analyze(
            manifest(
                concepts=[concept("a", ["b"]), concept("b", ["a"])],
                pages=[page("A", mentions=["missing"])],
                routes=[{"id": "r", "initial": [], "steps": ["missing-page"]}],
            )
        )
        self.assertFalse(result["catalog_consistent"])
        self.assertEqual(
            set(result["counts"]["findings_by_code"]),
            {"concept_cycle", "unknown_concept", "unknown_page"},
        )
        self.assertFalse(result["routes"][0]["contract_consistent"])

    def test_planned_route_is_not_implementation_verified(self):
        result = analyze(
            manifest(
                concepts=[concept("x"), concept("y", ["x"])],
                pages=[page("A", introduces=["x", "y"], uses=["y"], status="planned")],
                routes=[{"id": "r", "initial": [], "steps": ["A"]}],
            )
        )
        self.assertTrue(result["contract_consistent"])
        self.assertEqual(result["routes"][0]["final_knowledge"], ["x", "y"])
        self.assertFalse(result["routes"][0]["implementation_verified"])
        self.assertEqual(result["routes"][0]["planned_pages"], ["A"])

    def test_optional_children_do_not_export_and_deferred_use_fails(self):
        a = page("A", mentions=["x"])
        child = page("A.extra", introduces=["x"], parent_page="A")
        b = page("B", introduces=["x"], uses=["x"], deferred=["x"])
        result = analyze(
            manifest(
                concepts=[concept("x")],
                pages=[a, child, b],
                routes=[{"id": "r", "initial": [], "steps": ["A"]}],
            )
        )
        self.assertEqual(result["routes"][0]["final_knowledge"], [])
        self.assertIn("deferred_introduction", result["counts"]["findings_by_code"])
        self.assertIn("deferred_use", result["counts"]["findings_by_code"])

    def test_empty_or_missing_root_fields_raise_value_error(self):
        valid = manifest(concepts=[concept("x")], pages=[page("A")])
        with self.assertRaises(ValueError):
            analyze({})
        for field in ("schema_version", "concepts", "pages", "routes"):
            with self.subTest(field=field):
                incomplete = copy.deepcopy(valid)
                del incomplete[field]
                with self.assertRaises(ValueError):
                    analyze(incomplete)

    def test_schema_version_is_exactly_integer_one(self):
        for version in (None, True, False, 1.0, "1", 0, 2):
            with self.subTest(version=version):
                value = manifest(concepts=[concept("x")], pages=[page("A")])
                value["schema_version"] = version
                with self.assertRaisesRegex(ValueError, "schema_version"):
                    analyze(value)

    def test_root_arrays_are_required_and_concepts_pages_are_nonempty(self):
        for field in ("concepts", "pages", "routes"):
            for bad in (None, {}, "not-an-array"):
                with self.subTest(field=field, bad=bad):
                    value = manifest(concepts=[concept("x")], pages=[page("A")])
                    value[field] = bad
                    with self.assertRaises(ValueError):
                        analyze(value)
        for field in ("concepts", "pages"):
            with self.subTest(empty=field):
                value = manifest(concepts=[concept("x")], pages=[page("A")])
                value[field] = []
                with self.assertRaises(ValueError):
                    analyze(value)
        result = analyze(manifest(concepts=[concept("x")], pages=[page("A")]))
        self.assertTrue(result["contract_consistent"])
        self.assertEqual(result["routes"], [])

    def test_structural_errors_in_nested_values_raise_value_error(self):
        cases = [
            {"concepts": [{"id": 1}]},
            {"concepts": [{"id": "x", "requires": "x"}]},
            {"concepts": [{"id": "x", "requires": [False]}]},
            {"concepts": [{"id": "x", "owner": []}]},
            {"pages": [{"id": "A", "parent_page": []}]},
            {"pages": [{"id": "A", "sections": {}}]},
            {"pages": [{"id": "A", "sections": [{"id": 2}]}]},
            {"routes": [{"id": "r", "initial": [1]}]},
            {"routes": [{"id": "r", "steps": {}}]},
            {"routes": [{"id": "r", "steps": [{}]}]},
            {"routes": [{"id": "r", "steps": [{"alternatives": []}]}]},
            {"routes": [{"id": "r", "steps": [{"alternatives": ["A"]}]}]},
        ]
        for override in cases:
            with self.subTest(override=override):
                value = manifest(concepts=[concept("x")], pages=[page("A")])
                value.update(override)
                with self.assertRaises(ValueError):
                    analyze(value)

    def test_page_catalog_error_invalidates_route_that_visits_it(self):
        result = analyze(
            manifest(
                concepts=[concept("x")],
                pages=[page("A", uses=["unknown"])],
                routes=[{"id": "r", "initial": [], "steps": ["A"]}],
            )
        )
        self.assertFalse(result["pages"][0]["contract_consistent"])
        self.assertFalse(result["routes"][0]["contract_consistent"])
        self.assertIn("route_invalid_page", result["counts"]["findings_by_code"])

    def test_route_cannot_repair_an_invalid_standalone_page_contract(self):
        result = analyze(
            manifest(
                concepts=[concept("x")],
                pages=[page("A", uses=["x"])],
                routes=[{"id": "r", "initial": ["x"], "steps": ["A"]}],
            )
        )
        self.assertFalse(result["pages"][0]["contract_consistent"])
        self.assertFalse(result["routes"][0]["contract_consistent"])

    def test_invalid_optional_child_does_not_contaminate_parent(self):
        result = analyze(
            manifest(
                concepts=[concept("x")],
                pages=[
                    page("A"),
                    page("A.extra", parent_page="A", mentions=["unknown"]),
                ],
                routes=[{"id": "r", "initial": [], "steps": ["A"]}],
            )
        )
        self.assertFalse(result["contract_consistent"])
        self.assertTrue(result["pages"][0]["contract_consistent"])
        self.assertFalse(result["pages"][1]["contract_consistent"])
        self.assertTrue(result["routes"][0]["contract_consistent"])

    def test_failed_route_does_not_contaminate_another_route_or_page(self):
        result = analyze(
            manifest(
                concepts=[concept("x")],
                pages=[page("A", requires=["x"], uses=["x"])],
                routes=[
                    {"id": "r", "initial": [], "steps": ["A"]},
                    {"id": "r.extra", "initial": ["x"], "steps": ["A"]},
                ],
            )
        )
        self.assertTrue(result["pages"][0]["contract_consistent"])
        self.assertFalse(result["routes"][0]["contract_consistent"])
        self.assertTrue(result["routes"][1]["contract_consistent"])

    def test_unknown_concept_dependency_invalidates_dependents_pages_and_routes(self):
        result = analyze(
            manifest(
                concepts=[concept("x", ["unknown"]), concept("y", ["x"])],
                pages=[page("A", requires=["x"], introduces=["y"], uses=["x"])],
                routes=[{"id": "r", "initial": ["x"], "steps": ["A"]}],
            )
        )
        self.assertFalse(result["pages"][0]["contract_consistent"])
        self.assertEqual(result["pages"][0]["exported_concepts"], [])
        self.assertFalse(result["routes"][0]["contract_consistent"])
        self.assertIn("invalid_concept_reference", result["counts"]["findings_by_code"])

    def test_cycle_invalidates_transitive_dependents_not_just_back_edge(self):
        result = analyze(
            manifest(
                concepts=[
                    concept("x", ["y"]),
                    concept("y", ["x"]),
                    concept("z", ["x"]),
                ],
                pages=[page("A", requires=["x"], introduces=["z"])],
                routes=[{"id": "r", "initial": ["x"], "steps": ["A"]}],
            )
        )
        self.assertFalse(result["pages"][0]["contract_consistent"])
        self.assertEqual(result["pages"][0]["exported_concepts"], [])
        self.assertFalse(result["routes"][0]["contract_consistent"])
        self.assertIn("concept_cycle", result["counts"]["findings_by_code"])

    def test_invalid_initial_concept_invalidates_even_empty_route(self):
        result = analyze(
            manifest(
                concepts=[concept("x", ["x"])],
                pages=[page("A")],
                routes=[{"id": "r", "initial": ["x"], "steps": []}],
            )
        )
        self.assertFalse(result["routes"][0]["contract_consistent"])

    def test_duplicate_entity_ids_are_findings_and_taint_exact_entity(self):
        for field, duplicate in (
            ("concepts", concept("x")),
            ("pages", page("A")),
            ("routes", {"id": "r", "initial": [], "steps": ["A"]}),
        ):
            with self.subTest(field=field):
                value = manifest(
                    concepts=[concept("x")],
                    pages=[page("A", introduces=["x"])],
                    routes=[{"id": "r", "initial": [], "steps": ["A"]}],
                )
                value[field].append(duplicate)
                result = analyze(value)
                self.assertFalse(result["contract_consistent"])
                self.assertIn(
                    "duplicate_or_empty_id", result["counts"]["findings_by_code"]
                )
                self.assertFalse(result["routes"][0]["contract_consistent"])

    def test_duplicate_section_id_invalidates_only_its_page(self):
        a = page("A")
        a["sections"].append(
            {"id": "main", "introduces": ["x"], "uses": [], "mentions": []}
        )
        result = analyze(
            manifest(
                concepts=[concept("x")],
                pages=[a, page("A.extra")],
                routes=[
                    {"id": "r", "initial": [], "steps": ["A"]},
                    {"id": "s", "initial": [], "steps": ["A.extra"]},
                ],
            )
        )
        self.assertFalse(result["pages"][0]["contract_consistent"])
        self.assertTrue(result["pages"][1]["contract_consistent"])
        self.assertFalse(result["routes"][0]["contract_consistent"])
        self.assertTrue(result["routes"][1]["contract_consistent"])

    def test_optional_skip_branch_does_not_grant_knowledge(self):
        result = analyze(
            manifest(
                concepts=[concept("x")],
                pages=[
                    page("A", introduces=["x"]),
                    page("B", requires=["x"], uses=["x"]),
                ],
                routes=[
                    {
                        "id": "r",
                        "initial": [],
                        "steps": [{"alternatives": [["A"], []]}, "B"],
                    }
                ],
            )
        )
        self.assertEqual(result["routes"][0]["trace"][0]["knowledge_after_join"], [])
        self.assertFalse(result["routes"][0]["contract_consistent"])

    def test_no_implicit_prerequisite_closure(self):
        result = analyze(
            manifest(
                concepts=[concept("x"), concept("y", ["x"]), concept("z", ["x"])],
                pages=[page("A", requires=["y"], introduces=["z"])],
                routes=[{"id": "r", "initial": ["y"], "steps": ["A"]}],
            )
        )
        self.assertEqual(result["pages"][0]["exported_concepts"], [])
        self.assertIn(
            "introduction_prerequisite_missing", result["counts"]["findings_by_code"]
        )

    def test_main_exit_codes_distinguish_schema_contract_and_success(self):
        cases = [
            ({}, 2),
            (manifest(concepts=[concept("x")], pages=[page("A", uses=["unknown"])]), 1),
            (manifest(concepts=[concept("x")], pages=[page("A", introduces=["x"])]), 0),
        ]
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "manifest.json"
            for value, expected in cases:
                with self.subTest(expected=expected):
                    source.write_text(json.dumps(value), encoding="utf-8")
                    stdout, stderr = io.StringIO(), io.StringIO()
                    with redirect_stdout(stdout), redirect_stderr(stderr):
                        actual = main([str(source)])
                    self.assertEqual(actual, expected)
                    if expected == 2:
                        self.assertIn("schema_version", stderr.getvalue())
                        self.assertEqual(stdout.getvalue(), "")
                    else:
                        self.assertEqual(stderr.getvalue(), "")
                        self.assertEqual(
                            json.loads(stdout.getvalue())["contract_consistent"],
                            expected == 0,
                        )


if __name__ == "__main__":
    unittest.main()
