//! Experiment and run scoped Attachment descriptor helpers.

use crate::artifact::local_registry::{LocalRegistry, StoredDescriptor};
use crate::artifact::media_types;
use crate::{Instance, ParametricInstance, SampleSet, Solution};
use crate::{Message, Parse};
use anyhow::{ensure, Context, Result};
use oci_spec::image::{Descriptor, MediaType};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

/// Fallback media type when file content cannot be identified.
pub const DEFAULT_FILE_MEDIA_TYPE: &str = "application/octet-stream";

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
        Ok(self.attachment(name)?.media_type().clone())
    }

    pub(crate) fn blob(&self, name: &str) -> Result<Vec<u8>> {
        let descriptor = self.attachment(name)?;
        descriptor.registry().get_blob(descriptor)
    }

    pub(crate) fn instance(&self, name: &str) -> Result<Instance> {
        let descriptor = self.attachment(name)?;
        descriptor.ensure_media_type(&media_types::v1_instance())?;
        let bytes = descriptor.registry().get_blob(descriptor)?;
        decode_instance_layer(&bytes, descriptor)
    }

    pub(crate) fn parametric_instance(&self, name: &str) -> Result<ParametricInstance> {
        let descriptor = self.attachment(name)?;
        descriptor.ensure_media_type(&media_types::v1_parametric_instance())?;
        let bytes = descriptor.registry().get_blob(descriptor)?;
        decode_parametric_instance_layer(&bytes, descriptor)
    }

    pub(crate) fn solution(&self, name: &str) -> Result<Solution> {
        let descriptor = self.attachment(name)?;
        descriptor.ensure_media_type(&media_types::v1_solution())?;
        let bytes = descriptor.registry().get_blob(descriptor)?;
        decode_solution_layer(&bytes, descriptor)
    }

    pub(crate) fn sample_set(&self, name: &str) -> Result<SampleSet> {
        let descriptor = self.attachment(name)?;
        descriptor.ensure_media_type(&media_types::v1_sample_set())?;
        let bytes = descriptor.registry().get_blob(descriptor)?;
        decode_sample_set_layer(&bytes, descriptor)
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

/// Write `bytes` to the registry and build the in-memory Attachment descriptor.
pub(crate) fn store_attachment_descriptor<'reg>(
    registry: &'reg LocalRegistry,
    media_type: MediaType,
    bytes: &[u8],
    annotations: HashMap<String, String>,
) -> Result<StoredDescriptor<'reg>> {
    registry.store_layer_blob(media_type, bytes, annotations)
}

pub(crate) fn json_media_type() -> MediaType {
    MediaType::from(JSON_MEDIA_TYPE)
}

pub(crate) fn encode_json(name: &str, value: impl serde::Serialize) -> Result<Vec<u8>> {
    crate::artifact::stable_json_bytes(&value)
        .map_err(|e| crate::error!("Failed to encode JSON attachment `{name}`: {e}"))
}

pub fn encode_instance_layer(instance: &Instance) -> (Vec<u8>, HashMap<String, String>) {
    let proto: crate::v1::Instance = instance.clone().into();
    let annotations = crate::artifact::instance_annotations(&proto);
    (proto.encode_to_vec(), annotations)
}

pub fn encode_parametric_instance_layer(
    instance: &ParametricInstance,
) -> (Vec<u8>, HashMap<String, String>) {
    let proto: crate::v1::ParametricInstance = instance.clone().into();
    let annotations = crate::artifact::parametric_instance_annotations(&proto);
    (proto.encode_to_vec(), annotations)
}

pub fn encode_solution_layer(solution: &Solution) -> (Vec<u8>, HashMap<String, String>) {
    let proto: crate::v1::Solution = solution.clone().into();
    let annotations = crate::artifact::solution_annotations(&proto);
    (proto.encode_to_vec(), annotations)
}

pub fn encode_sample_set_layer(sample_set: &SampleSet) -> (Vec<u8>, HashMap<String, String>) {
    let proto: crate::v1::SampleSet = sample_set.clone().into();
    let annotations = crate::artifact::sample_set_annotations(&proto);
    (proto.encode_to_vec(), annotations)
}

pub fn decode_instance_layer(bytes: &[u8], descriptor: &Descriptor) -> Result<Instance> {
    let mut instance = crate::v1::Instance::decode(bytes)?;
    crate::artifact::merge_instance_annotations(&mut instance, &descriptor_annotations(descriptor));
    Ok(instance.try_into()?)
}

pub fn decode_parametric_instance_layer(
    bytes: &[u8],
    descriptor: &Descriptor,
) -> Result<ParametricInstance> {
    let mut instance = crate::v1::ParametricInstance::decode(bytes)?;
    crate::artifact::merge_parametric_instance_annotations(
        &mut instance,
        &descriptor_annotations(descriptor),
    );
    Ok(instance.parse(&())?)
}

pub fn decode_solution_layer(bytes: &[u8], descriptor: &Descriptor) -> Result<Solution> {
    let mut solution = crate::v1::Solution::decode(bytes)?;
    crate::artifact::merge_solution_annotations(&mut solution, &descriptor_annotations(descriptor));
    Ok(solution.parse(&())?)
}

pub fn decode_sample_set_layer(bytes: &[u8], descriptor: &Descriptor) -> Result<SampleSet> {
    let mut sample_set = crate::v1::SampleSet::decode(bytes)?;
    crate::artifact::merge_sample_set_annotations(
        &mut sample_set,
        &descriptor_annotations(descriptor),
    );
    Ok(sample_set.parse(&())?)
}

fn descriptor_annotations(desc: &Descriptor) -> HashMap<String, String> {
    desc.annotations().as_ref().cloned().unwrap_or_default()
}

/// Detect the media type of file contents using magic bytes.
pub fn detect_file_media_type(bytes: &[u8]) -> MediaType {
    infer::get(bytes)
        .map(|kind| MediaType::from(kind.mime_type()))
        .unwrap_or_else(|| MediaType::from(DEFAULT_FILE_MEDIA_TYPE))
}

pub(crate) fn read_file_attachment(
    path: impl AsRef<Path>,
    media_type: Option<MediaType>,
    filename: Option<&str>,
) -> Result<(MediaType, Vec<u8>, String)> {
    let path = path.as_ref();
    let bytes = read_attachment_file(path)?;
    let media_type = media_type.unwrap_or_else(|| detect_file_media_type(&bytes));
    let filename = file_attachment_filename(path, filename)?;
    Ok((media_type, bytes, filename))
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
