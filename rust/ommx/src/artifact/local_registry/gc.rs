use super::{BlobRecord, LocalRegistry, RefRecord};
use anyhow::{Context, Result};
use oci_spec::image::{Descriptor, Digest, ImageManifest};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    time::{Duration, SystemTime},
};

const DEFAULT_GC_GRACE_PERIOD: Duration = Duration::from_secs(24 * 60 * 60);

pub fn parse_gc_duration(input: &str) -> std::result::Result<Duration, String> {
    if input.is_empty() {
        return Err("duration must not be empty".to_string());
    }
    let (number, unit) = match input.as_bytes().last().copied() {
        Some(b's' | b'm' | b'h' | b'd') => (&input[..input.len() - 1], input.as_bytes().last()),
        Some(b'0'..=b'9') => (input, None),
        _ => {
            return Err(format!(
                "invalid duration suffix in {input:?}; use s, m, h, or d"
            ))
        }
    };
    let value = number
        .parse::<u64>()
        .map_err(|_| format!("invalid duration value in {input:?}"))?;
    let seconds = match unit.copied() {
        Some(b's') | None => value,
        Some(b'm') => value
            .checked_mul(60)
            .ok_or_else(|| format!("duration is too large: {input}"))?,
        Some(b'h') => value
            .checked_mul(60 * 60)
            .ok_or_else(|| format!("duration is too large: {input}"))?,
        Some(b'd') => value
            .checked_mul(24 * 60 * 60)
            .ok_or_else(|| format!("duration is too large: {input}"))?,
        _ => unreachable!("duration unit was filtered above"),
    };
    Ok(Duration::from_secs(seconds))
}

#[derive(Debug, Clone)]
pub struct GcOptions {
    /// Extra digests to treat as GC roots. If a protected digest is an
    /// OCI Image Manifest, GC also walks its config, layers, and subject.
    pub protected_digests: Vec<Digest>,
    /// Unreachable blobs newer than this are deferred so active Run
    /// writes after the latest checkpoint are not deleted.
    pub grace_period: Duration,
}

impl Default for GcOptions {
    fn default() -> Self {
        Self {
            protected_digests: Vec::new(),
            grace_period: DEFAULT_GC_GRACE_PERIOD,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GcReport {
    pub roots: Vec<GcRoot>,
    pub reachable_blobs: Vec<GcBlob>,
    pub orphan_candidates: Vec<GcBlob>,
    pub deferred_blobs: Vec<GcBlob>,
    pub missing_blobs: Vec<GcMissingBlob>,
    pub invalid_manifests: Vec<GcInvalidManifest>,
}

impl GcReport {
    pub fn reachable_size(&self) -> u64 {
        total_size(&self.reachable_blobs)
    }

    pub fn orphan_candidate_size(&self) -> u64 {
        total_size(&self.orphan_candidates)
    }

    pub fn deferred_size(&self) -> u64 {
        total_size(&self.deferred_blobs)
    }
}

#[derive(Debug, Clone)]
pub struct GcDeleteReport {
    pub report: GcReport,
    pub deleted_blobs: Vec<GcBlob>,
    /// Blobs that were candidates during mark but became too new to
    /// delete when rechecked immediately before unlink.
    pub skipped_blobs: Vec<GcBlob>,
}

impl GcDeleteReport {
    pub fn deleted_size(&self) -> u64 {
        total_size(&self.deleted_blobs)
    }
}

#[derive(Debug, Clone)]
pub enum GcRoot {
    Ref {
        name: String,
        reference: String,
        digest: Digest,
    },
    ProtectedDigest {
        digest: Digest,
    },
}

#[derive(Debug, Clone)]
pub struct GcBlob {
    pub digest: Digest,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcReferenceKind {
    RefManifest,
    ProtectedDigest,
    Config,
    Layer,
    Subject,
}

#[derive(Debug, Clone)]
pub struct GcMissingBlob {
    pub digest: Digest,
    pub referenced_by: Option<Digest>,
    pub kind: GcReferenceKind,
}

#[derive(Debug, Clone)]
pub struct GcInvalidManifest {
    pub digest: Digest,
    pub referenced_by: Option<Digest>,
    pub kind: GcReferenceKind,
    pub error: String,
}

#[derive(Debug, Clone)]
struct ManifestToVisit {
    digest: Digest,
    referenced_by: Option<Digest>,
    kind: GcReferenceKind,
    strict: bool,
}

impl LocalRegistry {
    pub fn gc_report(&self, options: &GcOptions) -> Result<GcReport> {
        let now = SystemTime::now();
        let refs = self.index().list_refs(None)?;
        let all_blobs = self.blobs().list_blobs()?;
        let all_blob_map: BTreeMap<String, BlobRecord> = all_blobs
            .iter()
            .cloned()
            .map(|blob| (blob.digest.as_ref().to_string(), blob))
            .collect();

        let mut roots = Vec::new();
        let mut reachable = BTreeMap::<String, Digest>::new();
        let mut parsed_manifests = BTreeSet::<String>::new();
        let mut to_visit = VecDeque::<ManifestToVisit>::new();
        let mut missing_blobs = Vec::new();
        let mut invalid_manifests = Vec::new();

        for ref_record in refs {
            roots.push(GcRoot::Ref {
                name: ref_record.name.clone(),
                reference: ref_record.reference.clone(),
                digest: ref_record.descriptor.digest().clone(),
            });
            enqueue_ref_manifest(&mut to_visit, ref_record);
        }

        for digest in &options.protected_digests {
            roots.push(GcRoot::ProtectedDigest {
                digest: digest.clone(),
            });
            to_visit.push_back(ManifestToVisit {
                digest: digest.clone(),
                referenced_by: None,
                kind: GcReferenceKind::ProtectedDigest,
                strict: false,
            });
        }

        while let Some(item) = to_visit.pop_front() {
            mark_digest(
                &mut reachable,
                &all_blob_map,
                &mut missing_blobs,
                &item.digest,
                item.referenced_by.clone(),
                item.kind,
            );
            let digest_key = item.digest.as_ref().to_string();
            if !parsed_manifests.insert(digest_key.clone()) {
                continue;
            }
            if !all_blob_map.contains_key(&digest_key) {
                continue;
            }

            let manifest = match self.read_gc_manifest(&item) {
                Ok(Some(manifest)) => manifest,
                Ok(None) => continue,
                Err(error) => {
                    invalid_manifests.push(GcInvalidManifest {
                        digest: item.digest,
                        referenced_by: item.referenced_by,
                        kind: item.kind,
                        error: error.to_string(),
                    });
                    continue;
                }
            };

            mark_descriptor(
                &mut reachable,
                &all_blob_map,
                &mut missing_blobs,
                manifest.config(),
                Some(item.digest.clone()),
                GcReferenceKind::Config,
            );
            for layer in manifest.layers() {
                mark_descriptor(
                    &mut reachable,
                    &all_blob_map,
                    &mut missing_blobs,
                    layer,
                    Some(item.digest.clone()),
                    GcReferenceKind::Layer,
                );
            }
            if let Some(subject) = manifest.subject() {
                to_visit.push_back(ManifestToVisit {
                    digest: subject.digest().clone(),
                    referenced_by: Some(item.digest),
                    kind: GcReferenceKind::Subject,
                    strict: true,
                });
            }
        }

        let reachable_blobs = reachable
            .keys()
            .filter_map(|digest| all_blob_map.get(digest))
            .cloned()
            .map(GcBlob::from)
            .collect();

        let mut orphan_candidates = Vec::new();
        let mut deferred_blobs = Vec::new();
        for blob in all_blobs {
            if reachable.contains_key(blob.digest.as_ref()) {
                continue;
            }
            if is_past_grace_period(&blob, now, options.grace_period) {
                orphan_candidates.push(GcBlob::from(blob));
            } else {
                deferred_blobs.push(GcBlob::from(blob));
            }
        }

        Ok(GcReport {
            roots,
            reachable_blobs,
            orphan_candidates,
            deferred_blobs,
            missing_blobs,
            invalid_manifests,
        })
    }

    pub fn gc(&self, _options: &GcOptions) -> Result<GcDeleteReport> {
        unimplemented!(
            "Local Registry GC delete is not implemented yet; run gc_report or `ommx gc` without --delete"
        )
    }

    fn read_gc_manifest(&self, item: &ManifestToVisit) -> Result<Option<ImageManifest>> {
        let bytes = self
            .blobs()
            .read_bytes(&item.digest)
            .with_context(|| format!("Failed to read manifest blob {}", item.digest))?;
        match serde_json::from_slice::<ImageManifest>(&bytes) {
            Ok(manifest) => Ok(Some(manifest)),
            Err(_error) if !item.strict => {
                tracing::debug!(
                    "Protected digest {} is not an OCI Image Manifest; keeping only the blob",
                    item.digest
                );
                Ok(None)
            }
            Err(error) => Err(error)
                .with_context(|| format!("Failed to parse OCI image manifest {}", item.digest)),
        }
    }
}

impl From<BlobRecord> for GcBlob {
    fn from(value: BlobRecord) -> Self {
        Self {
            digest: value.digest,
            size: value.size,
            modified: value.modified,
        }
    }
}

fn enqueue_ref_manifest(to_visit: &mut VecDeque<ManifestToVisit>, ref_record: RefRecord) {
    to_visit.push_back(ManifestToVisit {
        digest: ref_record.descriptor.digest().clone(),
        referenced_by: None,
        kind: GcReferenceKind::RefManifest,
        strict: true,
    });
}

fn mark_descriptor(
    reachable: &mut BTreeMap<String, Digest>,
    all_blobs: &BTreeMap<String, BlobRecord>,
    missing_blobs: &mut Vec<GcMissingBlob>,
    descriptor: &Descriptor,
    referenced_by: Option<Digest>,
    kind: GcReferenceKind,
) {
    mark_digest(
        reachable,
        all_blobs,
        missing_blobs,
        descriptor.digest(),
        referenced_by,
        kind,
    );
}

fn mark_digest(
    reachable: &mut BTreeMap<String, Digest>,
    all_blobs: &BTreeMap<String, BlobRecord>,
    missing_blobs: &mut Vec<GcMissingBlob>,
    digest: &Digest,
    referenced_by: Option<Digest>,
    kind: GcReferenceKind,
) {
    let digest_key = digest.as_ref().to_string();
    if reachable
        .insert(digest_key.clone(), digest.clone())
        .is_some()
    {
        return;
    }
    if !all_blobs.contains_key(&digest_key) {
        missing_blobs.push(GcMissingBlob {
            digest: digest.clone(),
            referenced_by,
            kind,
        });
    }
}

fn is_past_grace_period(blob: &BlobRecord, now: SystemTime, grace_period: Duration) -> bool {
    let Some(modified) = blob.modified else {
        return false;
    };
    let Ok(age) = now.duration_since(modified) else {
        return false;
    };
    age >= grace_period
}

fn total_size(blobs: &[GcBlob]) -> u64 {
    blobs.iter().map(|blob| blob.size).sum()
}
