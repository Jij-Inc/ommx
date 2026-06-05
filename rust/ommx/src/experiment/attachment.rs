//! Experiment and run scoped Attachment descriptor helpers.

use crate::artifact::local_registry::{LocalRegistry, StoredDescriptor};
use crate::artifact::media_types;
use crate::{Instance, ParametricInstance, SampleSet, Solution};
use anyhow::{ensure, Context, Result};
use oci_spec::image::MediaType;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

/// Fallback media type when file content cannot be identified.
pub const DEFAULT_FILE_MEDIA_TYPE: &str = "application/octet-stream";

/// Name-indexed attachment bindings for one Experiment or Run namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentTable<D> {
    /// Attachment name to stored descriptor reference.
    entries: BTreeMap<String, D>,
    /// Optional export filename metadata for file attachments.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    filenames: BTreeMap<String, String>,
}

impl<D> Default for AttachmentTable<D> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
            filenames: BTreeMap::new(),
        }
    }
}

impl<D> AttachmentTable<D> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn contains_key(&self, name: &str) -> bool {
        self.entries.contains_key(name)
    }

    pub fn get(&self, name: &str) -> Option<&D> {
        self.entries.get(name)
    }

    pub fn filename(&self, name: &str) -> Option<&str> {
        self.filenames.get(name).map(String::as_str)
    }

    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.entries.keys()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &D)> {
        self.entries.iter()
    }

    pub fn values(&self) -> impl Iterator<Item = &D> {
        self.entries.values()
    }

    pub fn filenames(&self) -> impl Iterator<Item = (&String, &String)> {
        self.filenames.iter()
    }

    pub fn insert(
        &mut self,
        name: impl Into<String>,
        descriptor: D,
        filename: Option<String>,
    ) -> Result<()> {
        let name = name.into();
        ensure!(
            !self.entries.contains_key(&name),
            "Attachment `{name}` already exists"
        );
        if let Some(filename) = filename.as_deref() {
            validate_attachment_filename(filename)?;
        }

        self.entries.insert(name.clone(), descriptor);
        if let Some(filename) = filename {
            self.filenames.insert(name, filename);
        }
        Ok(())
    }

    pub fn validate(&self, context: &str) -> Result<()> {
        for (name, filename) in &self.filenames {
            ensure!(
                self.entries.contains_key(name),
                "Attachment filename table in {context} references missing attachment `{name}`"
            );
            validate_attachment_filename(filename).with_context(|| {
                format!("Invalid attachment filename for `{name}` in {context}")
            })?;
        }
        Ok(())
    }

    pub fn try_map<E>(&self, mut f: impl FnMut(&D) -> Result<E>) -> Result<AttachmentTable<E>> {
        let entries = self
            .entries
            .iter()
            .map(|(name, descriptor)| Ok((name.clone(), f(descriptor)?)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        Ok(AttachmentTable {
            entries,
            filenames: self.filenames.clone(),
        })
    }

    pub fn try_map_owned<E>(self, mut f: impl FnMut(D) -> Result<E>) -> Result<AttachmentTable<E>> {
        let entries = self
            .entries
            .into_iter()
            .map(|(name, descriptor)| Ok((name, f(descriptor)?)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        Ok(AttachmentTable {
            entries,
            filenames: self.filenames,
        })
    }

    pub fn into_parts(self) -> (BTreeMap<String, D>, BTreeMap<String, String>) {
        (self.entries, self.filenames)
    }

    /// Build a table from raw config maps; callers must validate before trusting it.
    pub(crate) fn from_parts_unchecked(
        entries: BTreeMap<String, D>,
        filenames: BTreeMap<String, String>,
    ) -> Self {
        Self { entries, filenames }
    }
}

impl<'reg> AttachmentTable<StoredDescriptor<'reg>> {
    fn attachment(&self, name: &str) -> Result<&StoredDescriptor<'reg>> {
        self.get(name)
            .ok_or_else(|| anyhow::anyhow!("Attachment `{name}` not found"))
    }

    pub fn media_type(&self, name: &str) -> Result<MediaType> {
        Ok(self.attachment(name)?.media_type().clone())
    }

    pub(crate) fn payload_annotations(&self, name: &str) -> Result<HashMap<String, String>> {
        Ok(self
            .attachment(name)?
            .annotations()
            .as_ref()
            .cloned()
            .unwrap_or_default())
    }

    pub fn ensure_media_type(&self, name: &str, expected: &MediaType) -> Result<()> {
        self.attachment(name)?.ensure_media_type(expected)
    }

    pub fn blob(&self, name: &str) -> Result<Vec<u8>> {
        let descriptor = self.attachment(name)?;
        descriptor.registry().get_blob(descriptor)
    }

    pub fn instance(&self, name: &str) -> Result<Instance> {
        self.ensure_media_type(name, &media_types::v1_instance())?;
        Instance::from_bytes(&self.blob(name)?)
    }

    pub fn parametric_instance(&self, name: &str) -> Result<ParametricInstance> {
        self.ensure_media_type(name, &media_types::v1_parametric_instance())?;
        ParametricInstance::from_bytes(&self.blob(name)?)
    }

    pub fn solution(&self, name: &str) -> Result<Solution> {
        self.ensure_media_type(name, &media_types::v1_solution())?;
        Solution::from_bytes(&self.blob(name)?)
    }

    pub fn sample_set(&self, name: &str) -> Result<SampleSet> {
        self.ensure_media_type(name, &media_types::v1_sample_set())?;
        SampleSet::from_bytes(&self.blob(name)?)
    }

    pub fn write_attachment(
        &self,
        name: &str,
        path: impl AsRef<Path>,
        overwrite: bool,
    ) -> Result<PathBuf> {
        let descriptor = self
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Attachment `{name}` not found"))?;
        write_attachment_descriptor(
            descriptor.registry(),
            descriptor,
            name,
            self.filename(name),
            path,
            overwrite,
        )
    }
}

/// OCI layer media type for JSON attachment payloads.
const JSON_MEDIA_TYPE: &str = "application/json";

/// A filesystem file prepared as an Experiment attachment payload.
#[derive(Debug, Clone)]
pub struct FileAttachment {
    media_type: MediaType,
    bytes: Vec<u8>,
    filename: String,
}

impl FileAttachment {
    /// Read a local file and prepare its bytes, media type, and export filename metadata.
    pub fn from_path(
        path: impl AsRef<Path>,
        media_type: Option<MediaType>,
        filename: Option<&str>,
    ) -> Result<Self> {
        let path = path.as_ref();
        let bytes = read_attachment_file(path)?;
        let media_type = media_type.unwrap_or_else(|| detect_file_media_type(&bytes));
        let filename = file_attachment_filename(path, filename)?;
        Ok(Self {
            media_type,
            bytes,
            filename,
        })
    }

    /// Consume this file attachment into parts accepted by [`AttachmentLogger`](super::AttachmentLogger).
    pub fn into_parts(self) -> (MediaType, Vec<u8>, String) {
        (self.media_type, self.bytes, self.filename)
    }
}

/// Write `bytes` to the registry and build the in-memory Attachment descriptor.
pub fn store_attachment_descriptor<'reg>(
    registry: &'reg LocalRegistry,
    media_type: MediaType,
    bytes: &[u8],
    payload_annotations: HashMap<String, String>,
) -> Result<StoredDescriptor<'reg>> {
    registry.store_layer_blob(media_type, bytes, payload_annotations)
}

pub fn json_media_type() -> MediaType {
    MediaType::from(JSON_MEDIA_TYPE)
}

pub fn encode_json(name: &str, value: impl serde::Serialize) -> Result<Vec<u8>> {
    crate::artifact::stable_json_bytes(&value)
        .map_err(|e| crate::error!("Failed to encode JSON attachment `{name}`: {e}"))
}

/// Detect the media type of file contents using magic bytes.
pub fn detect_file_media_type(bytes: &[u8]) -> MediaType {
    infer::get(bytes)
        .map(|kind| MediaType::from(kind.mime_type()))
        .unwrap_or_else(|| MediaType::from(DEFAULT_FILE_MEDIA_TYPE))
}

/// Write an attachment blob to a filesystem path.
///
/// If `path` names an existing directory, the attachment filename metadata is
/// used inside that directory. Otherwise `path` is treated as the destination
/// file path.
fn write_attachment_descriptor(
    registry: &LocalRegistry,
    descriptor: &StoredDescriptor<'_>,
    name: &str,
    filename: Option<&str>,
    path: impl AsRef<Path>,
    overwrite: bool,
) -> Result<PathBuf> {
    let output_path = attachment_output_path(name, filename, path.as_ref());
    if output_path.exists() && !overwrite {
        crate::bail!(
            "Attachment destination `{}` already exists",
            output_path.display()
        );
    }

    let blob = registry.get_blob(descriptor)?;
    fs::write(&output_path, blob)
        .with_context(|| format!("Failed to write attachment to `{}`", output_path.display()))?;
    Ok(output_path)
}

fn read_attachment_file(path: &Path) -> Result<Vec<u8>> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("Failed to inspect attachment file `{}`", path.display()))?;
    ensure!(
        metadata.is_file(),
        "Attachment path `{}` is not a regular file",
        path.display()
    );
    fs::read(path).with_context(|| format!("Failed to read attachment file `{}`", path.display()))
}

fn file_attachment_filename(path: &Path, filename: Option<&str>) -> Result<String> {
    let filename = match filename {
        Some(filename) => filename.to_string(),
        None => path
            .file_name()
            .and_then(|filename| filename.to_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Attachment file `{}` does not have a valid UTF-8 filename",
                    path.display()
                )
            })?
            .to_string(),
    };
    validate_attachment_filename(&filename)?;
    Ok(filename)
}

fn validate_attachment_filename(filename: &str) -> Result<()> {
    ensure!(
        !filename.is_empty(),
        "Attachment filename must not be empty"
    );
    ensure!(
        !filename.contains('/') && !filename.contains('\\'),
        "Attachment filename must be a basename, not a path"
    );
    ensure!(
        filename != "." && filename != "..",
        "Attachment filename must not be `.` or `..`"
    );
    Ok(())
}

fn attachment_output_path(name: &str, filename: Option<&str>, path: &Path) -> PathBuf {
    if path.is_dir() {
        path.join(attachment_export_filename(name, filename))
    } else {
        path.to_path_buf()
    }
}

fn attachment_export_filename(name: &str, filename: Option<&str>) -> String {
    filename
        .and_then(safe_attachment_filename)
        .or_else(|| safe_attachment_filename(name))
        .unwrap_or_else(|| "attachment".to_string())
}

fn safe_attachment_filename(filename: &str) -> Option<String> {
    let candidate = filename.rsplit('/').next().unwrap_or(filename);
    let candidate = candidate.rsplit('\\').next().unwrap_or(candidate);
    if candidate.is_empty() || candidate == "." || candidate == ".." {
        None
    } else {
        Some(candidate.to_string())
    }
}
