//! Experiment and run scoped Attachment descriptor helpers.

use crate::artifact::{
    local_registry::StoredDescriptor,
    media_types::{self, RootPayloadVersion},
};
use crate::{Instance, ParametricInstance, SampleSet, Solution};
use anyhow::{ensure, Context, Result};
use oci_spec::image::MediaType;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

/// Fallback media type when file content cannot be identified.
pub const DEFAULT_FILE_MEDIA_TYPE: &str = "application/octet-stream";

const ZSTD_MEDIA_TYPE_SUFFIX: &str = "+zstd";

/// Compression applied to an Attachment's stored OCI layer.
///
/// Attachment readers remove the storage suffix, decompress the blob, and
/// expose the original media type and payload bytes. Compression is therefore
/// a storage detail rather than part of the attachment's logical type.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Compression {
    /// Store the attachment bytes unchanged.
    #[default]
    None,
    /// Store the attachment as a zstd stream.
    Zstd,
}

pub fn stored_media_type(compression: Compression, media_type: MediaType) -> MediaType {
    match compression {
        Compression::None => media_type,
        Compression::Zstd => MediaType::Other(format!("{media_type}{ZSTD_MEDIA_TYPE_SUFFIX}")),
    }
}

/// Name-indexed attachment bindings for one Experiment or Run namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AttachmentTable<D> {
    /// Attachment name to stored descriptor reference.
    entries: BTreeMap<String, D>,
    /// Optional export filename metadata for file attachments.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    filenames: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct RawAttachmentTable<D> {
    entries: BTreeMap<String, D>,
    #[serde(default)]
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

    pub fn from_entries<N>(entries: impl IntoIterator<Item = (N, D)>) -> Result<Self>
    where
        N: Into<String>,
    {
        let mut table = Self::new();
        for (name, descriptor) in entries {
            table.insert(name, descriptor, None)?;
        }
        Ok(table)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
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

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
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

    pub(crate) fn try_map<E>(
        &self,
        mut f: impl FnMut(&str, &D) -> Result<E>,
    ) -> Result<AttachmentTable<E>> {
        let entries = self
            .entries
            .iter()
            .map(|(name, descriptor)| Ok((name.clone(), f(name, descriptor)?)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        Ok(AttachmentTable::from_valid_parts(
            entries,
            self.filenames.clone(),
        ))
    }

    pub(crate) fn try_map_owned<E>(
        self,
        mut f: impl FnMut(D) -> Result<E>,
    ) -> Result<AttachmentTable<E>> {
        let entries = self
            .entries
            .into_iter()
            .map(|(name, descriptor)| Ok((name, f(descriptor)?)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        Ok(AttachmentTable::from_valid_parts(entries, self.filenames))
    }

    fn from_valid_parts(entries: BTreeMap<String, D>, filenames: BTreeMap<String, String>) -> Self {
        debug_assert!(validate_attachment_table_parts(&entries, &filenames).is_ok());
        Self { entries, filenames }
    }
}

impl<'de, D> Deserialize<'de> for AttachmentTable<D>
where
    D: Deserialize<'de>,
{
    fn deserialize<De>(deserializer: De) -> std::result::Result<Self, De::Error>
    where
        De: serde::Deserializer<'de>,
    {
        let raw = RawAttachmentTable::<D>::deserialize(deserializer)?;
        validate_attachment_table_parts(&raw.entries, &raw.filenames)
            .map_err(serde::de::Error::custom)?;
        Ok(Self {
            entries: raw.entries,
            filenames: raw.filenames,
        })
    }
}

impl<'reg> AttachmentTable<StoredDescriptor<'reg>> {
    fn attachment(&self, name: &str) -> Result<&StoredDescriptor<'reg>> {
        self.get(name)
            .ok_or_else(|| anyhow::anyhow!("Attachment `{name}` not found"))
    }

    pub(crate) fn media_type(&self, name: &str) -> Result<MediaType> {
        Ok(logical_media_type(self.attachment(name)?.media_type()).0)
    }

    pub(crate) fn blob(&self, name: &str) -> Result<Vec<u8>> {
        let descriptor = self.attachment(name)?;
        attachment_blob(descriptor)
    }

    pub(crate) fn instance(&self, name: &str) -> Result<Instance> {
        let descriptor = self.attachment(name)?;
        let (media_type, bytes) = attachment_payload(descriptor)?;
        let mut instance = match media_types::instance_payload_version(&media_type)? {
            RootPayloadVersion::V1 => Instance::from_v1_bytes(&bytes)?,
            RootPayloadVersion::V2 => Instance::from_v2_bytes(&bytes)?,
        };
        merge_descriptor_annotations(descriptor, &mut instance);
        Ok(instance)
    }

    pub(crate) fn parametric_instance(&self, name: &str) -> Result<ParametricInstance> {
        let descriptor = self.attachment(name)?;
        let (media_type, bytes) = attachment_payload(descriptor)?;
        let mut instance = match media_types::parametric_instance_payload_version(&media_type)? {
            RootPayloadVersion::V1 => ParametricInstance::from_v1_bytes(&bytes)?,
            RootPayloadVersion::V2 => ParametricInstance::from_v2_bytes(&bytes)?,
        };
        merge_descriptor_annotations(descriptor, &mut instance);
        Ok(instance)
    }

    pub(crate) fn solution(&self, name: &str) -> Result<Solution> {
        let descriptor = self.attachment(name)?;
        let (media_type, bytes) = attachment_payload(descriptor)?;
        let mut solution = match media_types::solution_payload_version(&media_type)? {
            RootPayloadVersion::V1 => Solution::from_v1_bytes(&bytes)?,
            RootPayloadVersion::V2 => Solution::from_v2_bytes(&bytes)?,
        };
        merge_descriptor_annotations(descriptor, &mut solution);
        Ok(solution)
    }

    pub(crate) fn sample_set(&self, name: &str) -> Result<SampleSet> {
        let descriptor = self.attachment(name)?;
        let (media_type, bytes) = attachment_payload(descriptor)?;
        let mut sample_set = match media_types::sample_set_payload_version(&media_type)? {
            RootPayloadVersion::V1 => SampleSet::from_v1_bytes(&bytes)?,
            RootPayloadVersion::V2 => SampleSet::from_v2_bytes(&bytes)?,
        };
        merge_descriptor_annotations(descriptor, &mut sample_set);
        Ok(sample_set)
    }

    pub(crate) fn write_attachment(
        &self,
        name: &str,
        path: impl AsRef<Path>,
        overwrite: bool,
    ) -> Result<PathBuf> {
        let descriptor = self
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Attachment `{name}` not found"))?;
        write_attachment_descriptor(descriptor, name, self.filename(name), path, overwrite)
    }
}

/// OCI layer media type for JSON attachment payloads.
const JSON_MEDIA_TYPE: &str = "application/json";

pub(crate) fn json_media_type() -> MediaType {
    MediaType::from(JSON_MEDIA_TYPE)
}

pub(crate) fn encode_json(name: &str, value: impl serde::Serialize) -> Result<Vec<u8>> {
    crate::artifact::stable_json_bytes(&value)
        .map_err(|e| crate::error!("Failed to encode JSON attachment `{name}`: {e}"))
}

fn logical_media_type(stored: &MediaType) -> (MediaType, Compression) {
    match stored.as_ref().strip_suffix(ZSTD_MEDIA_TYPE_SUFFIX) {
        Some(media_type) if !media_type.is_empty() => {
            (MediaType::from(media_type), Compression::Zstd)
        }
        _ => (stored.clone(), Compression::None),
    }
}

fn attachment_payload(descriptor: &StoredDescriptor<'_>) -> Result<(MediaType, Vec<u8>)> {
    let media_type = logical_media_type(descriptor.media_type()).0;
    let bytes = attachment_blob(descriptor)?;
    Ok((media_type, bytes))
}

fn attachment_blob(descriptor: &StoredDescriptor<'_>) -> Result<Vec<u8>> {
    let bytes = descriptor.registry().get_blob(descriptor)?;
    match logical_media_type(descriptor.media_type()).1 {
        Compression::None => Ok(bytes),
        Compression::Zstd => zstd::stream::decode_all(bytes.as_slice())
            .context("Failed to decompress zstd attachment"),
    }
}

fn merge_descriptor_annotations<T: crate::FlatAnnotations>(
    descriptor: &StoredDescriptor<'_>,
    value: &mut T,
) {
    let annotations = descriptor
        .annotations()
        .as_ref()
        .cloned()
        .unwrap_or_default();
    crate::FlatAnnotations::merge_annotations(value, &annotations);
}

/// Detect the media type of file contents using magic bytes.
pub fn detect_file_media_type(bytes: &[u8]) -> MediaType {
    infer::get(bytes)
        .map(|kind| MediaType::from(kind.mime_type()))
        .unwrap_or_else(|| MediaType::from(DEFAULT_FILE_MEDIA_TYPE))
}

pub fn open_file_attachment(
    path: impl AsRef<Path>,
    media_type: Option<MediaType>,
    filename: Option<&str>,
) -> Result<(MediaType, File, String)> {
    let path = path.as_ref();
    let mut file = File::open(path)
        .with_context(|| format!("Failed to open attachment file `{}`", path.display()))?;
    let metadata = file
        .metadata()
        .with_context(|| format!("Failed to inspect attachment file `{}`", path.display()))?;
    ensure!(
        metadata.is_file(),
        "Attachment path `{}` is not a regular file",
        path.display()
    );
    let media_type = match media_type {
        Some(media_type) => media_type,
        None => {
            let mut prefix = [0_u8; 8192];
            let read = file.read(&mut prefix).with_context(|| {
                format!("Failed to inspect attachment file `{}`", path.display())
            })?;
            file.seek(SeekFrom::Start(0)).with_context(|| {
                format!("Failed to rewind attachment file `{}`", path.display())
            })?;
            detect_file_media_type(&prefix[..read])
        }
    };
    let filename = file_attachment_filename(path, filename)?;
    Ok((media_type, file, filename))
}

/// Write an attachment blob to a filesystem path.
///
/// If `path` names an existing directory, the attachment filename metadata is
/// used inside that directory. Otherwise `path` is treated as the destination
/// file path.
fn write_attachment_descriptor(
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

    let blob = attachment_blob(descriptor)?;
    fs::write(&output_path, blob)
        .with_context(|| format!("Failed to write attachment to `{}`", output_path.display()))?;
    Ok(output_path)
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

fn validate_attachment_table_parts<D>(
    entries: &BTreeMap<String, D>,
    filenames: &BTreeMap<String, String>,
) -> Result<()> {
    for (name, filename) in filenames {
        ensure!(
            entries.contains_key(name),
            "Attachment filename table references missing attachment `{name}`"
        );
        validate_attachment_filename(filename)
            .with_context(|| format!("Invalid attachment filename for `{name}`"))?;
    }
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
