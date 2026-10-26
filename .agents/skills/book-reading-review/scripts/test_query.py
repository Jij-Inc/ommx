"""Regression tests for opt-in, revision-matched Book source queries."""

from __future__ import annotations

from contextlib import redirect_stderr, redirect_stdout
from copy import deepcopy
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from query import main, query


def manifest():
    return {
        "schema_version": 1,
        "baseline": {"source_revision": "example-revision"},
        "concepts": [
            {"id": "x", "label": "X", "owner": "A", "requires": [], "external": False},
            {"id": "y", "label": "Y", "owner": None, "requires": [], "external": True},
        ],
        "pages": [
            {
                "id": "A",
                "title": "Curated page title",
                "status": "revise",
                "source_paths": ["docs/ja/example.md"],
                "requires": [],
                "deferred": [],
                "sections": [
                    {
                        "id": "main",
                        "introduces": ["x"],
                        "uses": ["x"],
                        "mentions": ["y"],
                    }
                ],
            },
            {
                "id": "A.extra",
                "title": "Curated optional section",
                "status": "planned",
                "source_paths": ["docs/ja/example.md"],
                "parent_page": "A",
                "requires": ["x"],
                "deferred": ["y"],
                "sections": [
                    {"id": "extra", "introduces": [], "uses": ["x"], "mentions": ["y"]}
                ],
            },
        ],
        "routes": [],
    }


def evidence():
    return {
        "source_inventory.json": {
            "schema_version": "ommx-source-inventory-1",
            "source_commit": "example-revision",
            "pages": [
                {
                    "path": "docs/ja/example.md",
                    "source_url": "https://example.test/source",
                    "title": "Wrong title from a code comment",
                }
            ],
        },
        "observed_appearances.json": {
            "schema_version": "ommx-observed-appearances-1",
            "source_commit": "example-revision",
            "appearances": [
                {
                    "id": "OBS-1",
                    "path": "docs/ja/example.md",
                    "audit": "manual-audit",
                    "native_concept_id": "native-x",
                    "qualified_native_concept_id": "manual-audit:native-x",
                    "native_roles": ["mention"],
                    "native_surface": "prose",
                    "source_positions": [{"kind": "line", "start": 3, "end": 5}],
                    "note": "Only mentioned here",
                    "raw": {"custom_annotation": "preserve me"},
                }
            ],
        },
        "concept_crosswalk.json": {
            "schema_version": "ommx-concept-crosswalk-1",
            "source_commit": "example-revision",
            "entries": [
                {
                    "audit": "manual-audit",
                    "native_concept": "native-x",
                    "target_concepts": ["x"],
                    "relation": "related",
                    "note": "Not exact",
                }
            ],
        },
    }


class QueryTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)

    def write_json(self, filename, value):
        path = self.directory / filename
        path.write_text(json.dumps(value), encoding="utf-8")
        return path

    def write_evidence(self, documents=None):
        for filename, value in (
            documents if documents is not None else evidence()
        ).items():
            self.write_json(filename, value)
        return self.directory

    def invoke(self, *args):
        output, errors = io.StringIO(), io.StringIO()
        with redirect_stdout(output), redirect_stderr(errors):
            try:
                status = main([str(value) for value in args])
            except SystemExit as error:
                status = error.code
        return status, output.getvalue(), errors.getvalue()

    def test_default_uses_only_manifest_and_curated_titles(self):
        with patch(
            "query._load_evidence",
            side_effect=AssertionError("Unexpected evidence read"),
        ):
            result = query(manifest(), "concept", "x")
            self.assertEqual(result["actual_evidence"]["status"], "not_assessed")
            self.assertEqual(result["concept"]["owner"], "A")
            self.assertIn(
                {
                    "page": "A",
                    "title": "Curated page title",
                    "section": "main",
                    "role": "introduces",
                },
                result["target_contract_locations"],
            )
            page = query(manifest(), "page", "A.extra")
            self.assertEqual(
                page["target_page_contract"]["title"], "Curated optional section"
            )

    def test_default_does_not_require_evidence_baseline(self):
        document = manifest()
        del document["baseline"]
        self.assertEqual(
            query(document, "concept", "x")["actual_evidence"]["status"], "not_assessed"
        )

    def test_concept_roles_include_requires_deferred_and_mentions(self):
        result = query(manifest(), "concept", "y")
        self.assertEqual(
            {item["role"] for item in result["target_contract_locations"]},
            {"deferred", "mentions"},
        )

    def test_evidence_preserves_approximate_mapping_and_native_fields(self):
        original = manifest()
        documents = evidence()
        self.write_evidence(documents)
        before = deepcopy(original)
        result = query(original, "concept", "x", evidence_dir=self.directory)
        self.assertEqual(original, before)
        actual = result["actual_evidence"]
        self.assertEqual(actual["status"], "annotations_loaded")
        self.assertEqual(actual["source_revision"], "example-revision")
        observation = actual["observations"][0]
        self.assertEqual(observation["mapping_relation"], "related")
        self.assertEqual(observation["native_roles"], ["mention"])
        for field, value in documents["observed_appearances.json"]["appearances"][
            0
        ].items():
            self.assertEqual(observation[field], value)
        self.assertEqual(actual["prose_verification"], "not_performed")
        self.assertEqual(actual["runtime_verification"], "not_performed")

    def test_mapping_strength_is_never_promoted(self):
        for relation in ("exact", "narrower", "broader", "related"):
            with self.subTest(relation=relation):
                documents = evidence()
                documents["concept_crosswalk.json"]["entries"][0]["relation"] = relation
                self.write_evidence(documents)
                observation = query(
                    manifest(), "concept", "x", evidence_dir=self.directory
                )["actual_evidence"]["observations"][0]
                self.assertEqual(observation["mapping_relation"], relation)
                self.assertEqual(observation["native_roles"], ["mention"])

    def test_evidence_does_not_match_unrelated_concept(self):
        self.write_evidence()
        result = query(manifest(), "concept", "y", evidence_dir=self.directory)
        self.assertEqual(result["actual_evidence"]["observations"], [])

    def test_page_uses_source_without_inheriting_inventory_title(self):
        self.write_evidence()
        result = query(manifest(), "page", "A.extra", evidence_dir=self.directory)
        self.assertEqual(
            result["target_page_contract"]["title"], "Curated optional section"
        )
        self.assertEqual(len(result["actual_evidence"]["observations"]), 1)
        self.assertIn(
            "not automatically attributed", result["actual_evidence"]["attribution"]
        )
        self.assertNotIn("Wrong title", json.dumps(result))

    def test_each_evidence_revision_must_match_manifest(self):
        for filename in evidence():
            with self.subTest(filename=filename):
                documents = evidence()
                documents[filename]["source_commit"] = "another-revision"
                self.write_evidence(documents)
                with self.assertRaisesRegex(ValueError, "source_commit does not match"):
                    query(manifest(), "concept", "x", evidence_dir=self.directory)

    def test_evidence_requires_a_manifest_baseline(self):
        self.write_evidence()
        document = manifest()
        document.pop("baseline")
        with self.assertRaisesRegex(ValueError, "manifest.baseline"):
            query(document, "page", "A", evidence_dir=self.directory)

    def test_each_evidence_schema_is_checked(self):
        for filename in evidence():
            with self.subTest(filename=filename):
                documents = evidence()
                documents[filename]["schema_version"] = "future-schema"
                self.write_evidence(documents)
                with self.assertRaisesRegex(ValueError, "unsupported schema_version"):
                    query(manifest(), "concept", "x", evidence_dir=self.directory)

    def test_broken_evidence_joins_are_rejected(self):
        for filename, key in (
            ("source_inventory.json", "pages"),
            ("concept_crosswalk.json", "entries"),
        ):
            with self.subTest(filename=filename):
                documents = evidence()
                documents[filename][key] = []
                self.write_evidence(documents)
                with self.assertRaises(ValueError):
                    query(manifest(), "concept", "x", evidence_dir=self.directory)

    def test_invalid_evidence_shapes_and_mapping_targets_are_rejected(self):
        for invalid in (
            "duplicate",
            "unknown_target",
            "invalid_relation",
            "invalid_position",
            "null_roles",
        ):
            with self.subTest(invalid=invalid):
                documents = evidence()
                mappings = documents["concept_crosswalk.json"]["entries"]
                observation = documents["observed_appearances.json"]["appearances"][0]
                if invalid == "duplicate":
                    mappings.append(deepcopy(mappings[0]))
                elif invalid == "unknown_target":
                    mappings[0]["target_concepts"] = ["missing"]
                elif invalid == "invalid_relation":
                    mappings[0]["relation"] = "assume-exact"
                elif invalid == "invalid_position":
                    observation["source_positions"][0]["start"] = True
                else:
                    observation["native_roles"] = None
                self.write_evidence(documents)
                with self.assertRaises(ValueError):
                    query(manifest(), "concept", "x", evidence_dir=self.directory)

    def test_unmapped_evidence_is_not_invented_as_a_match(self):
        documents = evidence()
        documents["concept_crosswalk.json"]["entries"][0].update(
            target_concepts=[], relation="unmapped"
        )
        self.write_evidence(documents)
        self.assertEqual(
            query(manifest(), "concept", "x", evidence_dir=self.directory)[
                "actual_evidence"
            ]["observations"],
            [],
        )
        page = query(manifest(), "page", "A", evidence_dir=self.directory)
        self.assertEqual(
            page["actual_evidence"]["observations"][0]["mapping_relation"], "unmapped"
        )

    def test_query_rejects_semantically_inconsistent_manifest(self):
        document = manifest()
        document["pages"][0]["sections"][0]["introduces"] = []
        with self.assertRaisesRegex(ValueError, "inconsistent"):
            query(document, "page", "A")

    def test_cli_manifest_path_is_explicit_and_defaults_to_no_evidence(self):
        path = self.write_json("chosen-manifest.json", manifest())
        status, output, errors = self.invoke(path, "page", "A")
        self.assertEqual(status, 0, errors)
        self.assertEqual(
            json.loads(output)["actual_evidence"]["status"], "not_assessed"
        )

    def test_cli_unknown_ids_exit_2_without_traceback(self):
        path = self.write_json("manifest.json", manifest())
        for kind in ("concept", "page"):
            with self.subTest(kind=kind):
                status, output, errors = self.invoke(path, kind, "missing")
                self.assertEqual(status, 2)
                self.assertEqual(output, "")
                self.assertIn("Unknown", errors)
                self.assertNotIn("Traceback", errors)

    def test_cli_invalid_schema_missing_files_and_evidence_exit_2(self):
        valid_path = self.write_json("valid.json", manifest())
        broken_json = self.directory / "broken.json"
        broken_json.write_text("{", encoding="utf-8")
        bad_schema = manifest()
        bad_schema["schema_version"] = 999
        schema_path = self.write_json("schema.json", bad_schema)
        for args in (
            (self.directory / "missing.json", "concept", "x"),
            (broken_json, "concept", "x"),
            (schema_path, "concept", "x"),
            (
                valid_path,
                "concept",
                "x",
                "--evidence-dir",
                self.directory / "missing-evidence",
            ),
        ):
            with self.subTest(args=args):
                status, output, errors = self.invoke(*args)
                self.assertEqual(status, 2)
                self.assertEqual(output, "")
                self.assertTrue(errors)
                self.assertNotIn("Traceback", errors)

    def test_cli_malformed_evidence_exits_2_without_traceback(self):
        for invalid in (None, [], {}, True):
            with self.subTest(invalid=invalid):
                documents = evidence()
                documents["observed_appearances.json"]["appearances"][0][
                    "source_positions"
                ][0]["kind"] = invalid
                self.write_evidence(documents)
                path = self.write_json("manifest.json", manifest())
                status, output, errors = self.invoke(
                    path, "concept", "x", "--evidence-dir", self.directory
                )
                self.assertEqual(status, 2)
                self.assertEqual(output, "")
                self.assertNotIn("Traceback", errors)

    def test_cli_evidence_option_is_honored(self):
        self.write_evidence()
        path = self.write_json("manifest.json", manifest())
        status, output, errors = self.invoke(
            path, "concept", "x", "--evidence-dir", self.directory
        )
        self.assertEqual(status, 0, errors)
        self.assertEqual(
            json.loads(output)["actual_evidence"]["status"], "annotations_loaded"
        )


if __name__ == "__main__":
    unittest.main()
