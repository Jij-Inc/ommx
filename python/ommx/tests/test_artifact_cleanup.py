import uuid

import pytest

from ommx.artifact import (
    ArtifactDraft,
    gc,
    list_artifacts,
    prune_anonymous,
    remove_image,
    restore_image,
)
from ommx.experiment import Experiment


def test_cleanup_accepts_explicit_empty_root(tmp_path):
    prune_report = prune_anonymous(root=tmp_path)
    assert prune_report.root == tmp_path
    assert prune_report.delete_applied is False
    assert prune_report.count == 0
    assert prune_report.refs == []

    gc_report = gc(root=tmp_path)
    assert gc_report.root == tmp_path
    assert gc_report.delete_applied is False
    assert gc_report.roots == []
    assert gc_report.reachable_blobs == []
    assert gc_report.orphan_candidates == []
    assert gc_report.deferred_blobs == []
    assert gc_report.deleted_blobs == []


def test_prune_anonymous_reports_and_deletes_refs():
    artifact = ArtifactDraft.new_anonymous().commit()
    image_name = artifact.image_name

    dry_run = prune_anonymous()
    assert dry_run.delete_applied is False
    assert image_name in {ref.image_name for ref in dry_run.refs}

    deleted = prune_anonymous(delete=True)
    assert deleted.delete_applied is True
    assert image_name in {ref.image_name for ref in deleted.refs}

    after = prune_anonymous()
    assert image_name not in {ref.image_name for ref in after.refs}


def test_gc_reports_and_deletes_unreachable_blobs_after_prune():
    payload = f"gc-target-{uuid.uuid4()}".encode()
    draft = ArtifactDraft.new_anonymous()
    descriptor = draft.add_layer("application/octet-stream", payload, {})
    artifact = draft.commit()

    deleted_refs = prune_anonymous(delete=True)
    assert artifact.image_name in {ref.image_name for ref in deleted_refs.refs}

    dry_run = gc(grace_period="0s")
    assert dry_run.delete_applied is False
    assert descriptor.digest in {blob.digest for blob in dry_run.orphan_candidates}
    assert dry_run.deleted_blobs == []

    deleted = gc(delete=True, grace_period="0s")
    assert deleted.delete_applied is True
    assert descriptor.digest in {blob.digest for blob in deleted.deleted_blobs}
    assert deleted.deleted_size >= descriptor.size

    after = gc(grace_period="0s")
    assert descriptor.digest not in {blob.digest for blob in after.orphan_candidates}


def test_remove_image_deletes_named_ref_but_leaves_blobs_for_gc():
    image_name = f"example.com/ommx-tests/remove-{uuid.uuid4().hex}:latest"
    draft = ArtifactDraft.new(image_name)
    descriptor = draft.add_layer("application/octet-stream", b"remove-me", {})
    draft.commit()
    manifest_digest = list_artifacts(image_name)[0].manifest_digest

    assert remove_image(image_name) == manifest_digest
    assert remove_image(image_name) is None
    assert list_artifacts(image_name) == []
    assert restore_image(image_name, manifest_digest) is True
    assert restore_image(image_name, manifest_digest) is False
    assert [record.manifest_digest for record in list_artifacts(image_name)] == [
        manifest_digest
    ]

    assert remove_image(image_name) == manifest_digest
    assert descriptor.digest in {
        blob.digest for blob in gc(grace_period="0s").orphan_candidates
    }


def test_restore_image_does_not_replace_a_new_target():
    image_name = f"example.com/ommx-tests/restore-{uuid.uuid4().hex}:latest"
    original = ArtifactDraft.new(image_name)
    original.add_layer("application/octet-stream", b"original", {})
    original.commit()
    original_digest = list_artifacts(image_name)[0].manifest_digest
    assert remove_image(image_name) == original_digest

    replacement = ArtifactDraft.new(image_name)
    replacement.add_layer("application/octet-stream", b"replacement", {})
    replacement.commit()
    replacement_digest = list_artifacts(image_name)[0].manifest_digest

    with pytest.raises(RuntimeError, match="ref currently points to"):
        restore_image(image_name, original_digest)
    assert [record.manifest_digest for record in list_artifacts(image_name)] == [
        replacement_digest
    ]


def test_prune_anonymous_can_include_experiments_and_filter_by_age():
    with Experiment() as experiment:
        experiment.log_json("kind", "anonymous-experiment")
    image_name = experiment.image_name

    artifacts_only = prune_anonymous()
    assert image_name not in {ref.image_name for ref in artifacts_only.refs}

    recent_only = prune_anonymous(experiments=True, older_than="365d")
    assert image_name not in {ref.image_name for ref in recent_only.refs}

    candidates = prune_anonymous(experiments=True, older_than="0s")
    matching = [ref for ref in candidates.refs if ref.image_name == image_name]
    assert len(matching) == 1
    assert matching[0].kind == "experiment"

    deleted = prune_anonymous(
        delete=True,
        experiments=True,
        older_than="0s",
    )
    assert image_name in {ref.image_name for ref in deleted.refs}


def test_prune_anonymous_rejects_invalid_retention_duration():
    with pytest.raises(ValueError, match="invalid duration suffix"):
        prune_anonymous(older_than="last-week")


def test_gc_rejects_invalid_grace_period():
    with pytest.raises(ValueError, match="invalid duration suffix"):
        gc(grace_period="last-week")
