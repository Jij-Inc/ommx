import uuid

from ommx.artifact import ArtifactDraft, gc, prune_anonymous


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
