//! Experiment and run scoped Attachment descriptor helpers.

use crate::artifact::local_registry::{LocalRegistry, StoredDescriptor};
use crate::artifact::{
    media_types, InstanceAnnotations, ParametricInstanceAnnotations, SampleSetAnnotations,
    SolutionAnnotations,
};
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
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn from_entries<N>(entries: impl IntoIterator<Item = (N, D)>) -> Result<Self>
    where
        N: Into<String>,
    {
        let mut table = Self::new();
        for (name, descriptor) in entries {
            table.insert(name, descriptor, None)?;
        }
        Ok(table)
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn contains_key(&self, name: &str) -> bool {
        self.entries.contains_key(name)
    }

    pub(crate) fn get(&self, name: &str) -> Option<&D> {
        self.entries.get(name)
    }

    pub(crate) fn filename(&self, name: &str) -> Option<&str> {
        self.filenames.get(name).map(String::as_str)
    }

    pub(crate) fn names(&self) -> impl Iterator<Item = &String> {
        self.entries.keys()
    }

    pub(crate) fn insert(
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

    pub(crate) fn validate(&self, context: &str) -> Result<()> {
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

    pub(crate) fn try_map<E>(
        &self,
        mut f: impl FnMut(&str, &D) -> Result<E>,
    ) -> Result<AttachmentTable<E>> {
        let mut table = AttachmentTable::from_entries(
            self.entries
                .iter()
                .map(|(name, descriptor)| Ok((name.clone(), f(name, descriptor)?)))
                .collect::<Result<Vec<_>>>()?,
        )?;
        table.filenames = self.filenames.clone();
        Ok(table)
    }

    pub(crate) fn try_map_owned<E>(
        self,
        mut f: impl FnMut(D) -> Result<E>,
    ) -> Result<AttachmentTable<E>> {
        let mut table = AttachmentTable::from_entries(
            self.entries
                .into_iter()
                .map(|(name, descriptor)| Ok((name, f(descriptor)?)))
                .collect::<Result<Vec<_>>>()?,
        )?;
        table.filenames = self.filenames;
        Ok(table)
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

    pub(crate) fn instance(&self, name: &str) -> Result<(Instance, InstanceAnnotations)> {
        let descriptor = self.attachment(name)?;
        descriptor.ensure_media_type(&media_types::v1_instance())?;
        let bytes = descriptor.registry().get_blob(descriptor)?;
        Ok((
            Instance::from_bytes(&bytes)?,
            InstanceAnnotations::from_descriptor(descriptor),
        ))
    }

    pub(crate) fn parametric_instance(
        &self,
        name: &str,
    ) -> Result<(ParametricInstance, ParametricInstanceAnnotations)> {
        let descriptor = self.attachment(name)?;
        descriptor.ensure_media_type(&media_types::v1_parametric_instance())?;
        let bytes = descriptor.registry().get_blob(descriptor)?;
        Ok((
            ParametricInstance::from_bytes(&bytes)?,
            ParametricInstanceAnnotations::from_descriptor(descriptor),
        ))
    }

    pub(crate) fn solution(&self, name: &str) -> Result<(Solution, SolutionAnnotations)> {
        let descriptor = self.attachment(name)?;
        descriptor.ensure_media_type(&media_types::v1_solution())?;
        let bytes = descriptor.registry().get_blob(descriptor)?;
        Ok((
            Solution::from_bytes(&bytes)?,
            SolutionAnnotations::from_descriptor(descriptor),
        ))
    }

    pub(crate) fn sample_set(&self, name: &str) -> Result<(SampleSet, SampleSetAnnotations)> {
        let descriptor = self.attachment(name)?;
        descriptor.ensure_media_type(&media_types::v1_sample_set())?;
        let bytes = descriptor.registry().get_blob(descriptor)?;
        Ok((
            SampleSet::from_bytes(&bytes)?,
            SampleSetAnnotations::from_descriptor(descriptor),
        ))
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
