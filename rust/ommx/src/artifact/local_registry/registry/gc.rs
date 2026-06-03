use super::super::RefRecord;
use super::{BlobRecord, LocalRegistry};
use anyhow::{Context, Result};
use oci_spec::image::{Descriptor, Digest, ImageManifest};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    time::{Duration, SystemTime},
};

const DEFAULT_GC_GRACE_PERIOD: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone)]
pub struct GcOptions {
    /// Extra digests to treat as GC roots. If a protected digest is an
    /// OCI Image Manifest, GC also walks its config, layers, and subject.
    pub protected_digests: Vec<Digest>,
    /// Unreachable blobs newer than this are deferred so active Run
    /// writes after the latest checkpoint are not deleted.
    pub grace_period: Duration,
}

impl GcOptions {
    pub fn parse_grace_period(input: &str) -> std::result::Result<Duration, String> {
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
        GcBlob::total_size(&self.reachable_blobs)
    }

    pub fn orphan_candidate_size(&self) -> u64 {
        GcBlob::total_size(&self.orphan_candidates)
    }

    pub fn deferred_size(&self) -> u64 {
        GcBlob::total_size(&self.deferred_blobs)
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
        GcBlob::total_size(&self.deleted_blobs)
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

struct GcTraversal<'reg> {
    registry: &'reg LocalRegistry,
    now: SystemTime,
    grace_period: Duration,
    roots: Vec<GcRoot>,
    all_blobs: Vec<BlobRecord>,
    all_blob_map: BTreeMap<String, BlobRecord>,
    reachable: BTreeMap<String, Digest>,
    parsed_manifests: BTreeSet<String>,
    to_visit: VecDeque<ManifestToVisit>,
    missing_blobs: Vec<GcMissingBlob>,
    invalid_manifests: Vec<GcInvalidManifest>,
}

impl LocalRegistry {
    pub fn gc_report(&self, options: &GcOptions) -> Result<GcReport> {
        GcTraversal::new(self, options)?.run()
    }

    pub fn gc(&self, options: &GcOptions) -> Result<GcDeleteReport> {
        let report = self.gc_report(options)?;
        let mut deleted_blobs = Vec::new();
        let mut skipped_blobs = Vec::new();
        for candidate in &report.orphan_candidates {
            let Some(record) = self.blob_record(&candidate.digest)? else {
                continue;
            };
            if !record.is_past_grace_period(SystemTime::now(), options.grace_period) {
                skipped_blobs.push(GcBlob::from(record));
                continue;
            }
            if self.delete_blob(&record.digest)? {
                deleted_blobs.push(GcBlob::from(record));
            }
        }
        Ok(GcDeleteReport {
            report,
            deleted_blobs,
            skipped_blobs,
        })
    }
}

impl<'reg> GcTraversal<'reg> {
    fn new(registry: &'reg LocalRegistry, options: &GcOptions) -> Result<Self> {
        let all_blobs = registry.list_blob_records()?;
        let all_blob_map = all_blobs
            .iter()
            .cloned()
            .map(|blob| (blob.digest.as_ref().to_string(), blob))
            .collect();
        let mut traversal = Self {
            registry,
            now: SystemTime::now(),
            grace_period: options.grace_period,
            roots: Vec::new(),
            all_blobs,
            all_blob_map,
            reachable: BTreeMap::new(),
            parsed_manifests: BTreeSet::new(),
            to_visit: VecDeque::new(),
            missing_blobs: Vec::new(),
            invalid_manifests: Vec::new(),
        };
        for ref_record in registry.index().list_refs(None)? {
            traversal.add_ref_root(ref_record);
        }
        for digest in &options.protected_digests {
            traversal.add_protected_root(digest.clone());
        }
        Ok(traversal)
    }

    fn run(mut self) -> Result<GcReport> {
        while let Some(item) = self.to_visit.pop_front() {
            self.visit_manifest(item);
        }
        Ok(self.into_report())
    }

    fn add_ref_root(&mut self, ref_record: RefRecord) {
        self.roots.push(GcRoot::Ref {
            name: ref_record.name.clone(),
            reference: ref_record.reference.clone(),
            digest: ref_record.descriptor.digest().clone(),
        });
        self.to_visit
            .push_back(ManifestToVisit::ref_manifest(&ref_record));
    }

    fn add_protected_root(&mut self, digest: Digest) {
        self.roots.push(GcRoot::ProtectedDigest {
            digest: digest.clone(),
        });
        self.to_visit
            .push_back(ManifestToVisit::protected_digest(digest));
    }

    fn visit_manifest(&mut self, item: ManifestToVisit) {
        self.mark_digest(&item.digest, item.referenced_by.clone(), item.kind);
        let digest_key = item.digest.as_ref().to_string();
        if !self.parsed_manifests.insert(digest_key.clone()) {
            return;
        }
        if !self.all_blob_map.contains_key(&digest_key) {
            return;
        }

        let manifest = match self.read_manifest(&item) {
            Ok(Some(manifest)) => manifest,
            Ok(None) => return,
            Err(error) => {
                self.invalid_manifests.push(GcInvalidManifest {
                    digest: item.digest,
                    referenced_by: item.referenced_by,
                    kind: item.kind,
                    error: error.to_string(),
                });
                return;
            }
        };

        self.mark_descriptor(
            manifest.config(),
            Some(item.digest.clone()),
            GcReferenceKind::Config,
        );
        for layer in manifest.layers() {
            self.mark_descriptor(layer, Some(item.digest.clone()), GcReferenceKind::Layer);
        }
        if let Some(subject) = manifest.subject() {
            self.to_visit
                .push_back(ManifestToVisit::subject(subject, item.digest));
        }
    }

    fn read_manifest(&self, item: &ManifestToVisit) -> Result<Option<ImageManifest>> {
        let bytes = self
            .registry
            .read_blob(&item.digest)
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

    fn mark_descriptor(
        &mut self,
        descriptor: &Descriptor,
        referenced_by: Option<Digest>,
        kind: GcReferenceKind,
    ) {
        self.mark_digest(descriptor.digest(), referenced_by, kind);
    }

    fn mark_digest(
        &mut self,
        digest: &Digest,
        referenced_by: Option<Digest>,
        kind: GcReferenceKind,
    ) {
        let digest_key = digest.as_ref().to_string();
        if self
            .reachable
            .insert(digest_key.clone(), digest.clone())
            .is_some()
        {
            return;
        }
        if !self.all_blob_map.contains_key(&digest_key) {
            self.missing_blobs.push(GcMissingBlob {
                digest: digest.clone(),
                referenced_by,
                kind,
            });
        }
    }

    fn into_report(self) -> GcReport {
        let reachable_blobs = self
            .reachable
            .keys()
            .filter_map(|digest| self.all_blob_map.get(digest))
            .cloned()
            .map(GcBlob::from)
            .collect();

        let mut orphan_candidates = Vec::new();
        let mut deferred_blobs = Vec::new();
        for blob in self.all_blobs {
            if self.reachable.contains_key(blob.digest.as_ref()) {
                continue;
            }
            if blob.is_past_grace_period(self.now, self.grace_period) {
                orphan_candidates.push(GcBlob::from(blob));
            } else {
                deferred_blobs.push(GcBlob::from(blob));
            }
        }

        GcReport {
            roots: self.roots,
            reachable_blobs,
            orphan_candidates,
            deferred_blobs,
            missing_blobs: self.missing_blobs,
            invalid_manifests: self.invalid_manifests,
        }
    }
}

impl ManifestToVisit {
    fn ref_manifest(ref_record: &RefRecord) -> Self {
        Self {
            digest: ref_record.descriptor.digest().clone(),
            referenced_by: None,
            kind: GcReferenceKind::RefManifest,
            strict: true,
        }
    }

    fn protected_digest(digest: Digest) -> Self {
        Self {
            digest,
            referenced_by: None,
            kind: GcReferenceKind::ProtectedDigest,
            strict: false,
        }
    }

    fn subject(subject: &Descriptor, referenced_by: Digest) -> Self {
        Self {
            digest: subject.digest().clone(),
            referenced_by: Some(referenced_by),
            kind: GcReferenceKind::Subject,
            strict: true,
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

impl BlobRecord {
    fn is_past_grace_period(&self, now: SystemTime, grace_period: Duration) -> bool {
        let Some(modified) = self.modified else {
            return false;
        };
        let Ok(age) = now.duration_since(modified) else {
            return false;
        };
        age >= grace_period
    }
}

impl GcBlob {
    fn total_size(blobs: &[Self]) -> u64 {
        blobs.iter().map(|blob| blob.size).sum()
    }
}
