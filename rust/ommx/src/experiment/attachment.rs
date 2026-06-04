//! Experiment and run scoped Attachment descriptor helpers.

use super::{ANN_ATTACHMENT_NAME, ANN_RUN_ID, ANN_SPACE};
use crate::artifact::local_registry::{LocalRegistry, StoredDescriptor};
use anyhow::{ensure, Context, Result};
use oci_spec::image::MediaType;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

/// Descriptor annotation key storing the human attachment name.
pub const ATTACHMENT_NAME_ANNOTATION: &str = ANN_ATTACHMENT_NAME;

/// Descriptor annotation key storing the filename used when exporting a file attachment.
pub const ATTACHMENT_FILENAME_ANNOTATION: &str = "org.ommx.attachment.filename";

/// Fallback media type when file content cannot be identified.
pub const DEFAULT_FILE_MEDIA_TYPE: &str = "application/octet-stream";

/// The storage space an Attachment descriptor belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentSpace {
    /// Shared by the whole experiment (dataset, source problem, ...).
    Experiment,
    /// Owned by a single run.
    Run(u64),
}

impl AttachmentSpace {
    fn as_str(self) -> &'static str {
        match self {
            AttachmentSpace::Experiment => "experiment",
            AttachmentSpace::Run(_) => "run",
        }
    }

    fn run_id(self) -> Option<u64> {
        match self {
            AttachmentSpace::Experiment => None,
            AttachmentSpace::Run(run_id) => Some(run_id),
        }
    }

    fn descriptor_annotations(
        self,
        name: &str,
        extra_annotations: HashMap<String, String>,
    ) -> Result<HashMap<String, String>> {
        let mut annotations = HashMap::new();
        annotations.insert(ANN_SPACE.to_string(), self.as_str().to_string());
        if let Some(run_id) = self.run_id() {
            annotations.insert(ANN_RUN_ID.to_string(), run_id.to_string());
        }
        annotations.insert(ANN_ATTACHMENT_NAME.to_string(), name.to_string());
        for (key, value) in extra_annotations {
            ensure!(
                key != ANN_SPACE && key != ANN_RUN_ID && key != ANN_ATTACHMENT_NAME,
                "Attachment annotation `{key}` is reserved"
            );
            annotations.insert(key, value);
        }
        Ok(annotations)
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
    pub fn into_parts(self) -> (MediaType, Vec<u8>, HashMap<String, String>) {
        let mut annotations = HashMap::new();
        annotations.insert(ATTACHMENT_FILENAME_ANNOTATION.to_string(), self.filename);
        (self.media_type, self.bytes, annotations)
    }
}

/// Write `bytes` to the registry and build the in-memory Attachment descriptor.
pub fn store_attachment_descriptor<'reg>(
    registry: &'reg LocalRegistry,
    space: AttachmentSpace,
    name: &str,
    media_type: MediaType,
    bytes: &[u8],
    extra_annotations: HashMap<String, String>,
) -> Result<StoredDescriptor<'reg>> {
    let annotations = space.descriptor_annotations(name, extra_annotations)?;
    registry.store_layer_blob(media_type, bytes, annotations)
}

pub fn json_media_type() -> MediaType {
    MediaType::from(JSON_MEDIA_TYPE)
}

pub fn encode_json(name: &str, value: impl serde::Serialize) -> Result<Vec<u8>> {
    crate::artifact::stable_json_bytes(&value)
        .map_err(|e| crate::error!("Failed to encode JSON attachment `{name}`: {e}"))
}

pub fn attachment_name(descriptor: &oci_spec::image::Descriptor) -> Option<&str> {
    descriptor
        .annotations()
        .as_ref()
        .and_then(|annotations| annotations.get(ANN_ATTACHMENT_NAME))
        .map(String::as_str)
}

/// Return the export filename recorded for a file attachment.
pub fn attachment_filename(descriptor: &oci_spec::image::Descriptor) -> Option<&str> {
    descriptor
        .annotations()
        .as_ref()
        .and_then(|annotations| annotations.get(ATTACHMENT_FILENAME_ANNOTATION))
        .map(String::as_str)
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
pub fn write_attachment_descriptor(
    registry: &LocalRegistry,
    descriptor: &StoredDescriptor<'_>,
    name: &str,
    path: impl AsRef<Path>,
    overwrite: bool,
) -> Result<PathBuf> {
    let output_path = attachment_output_path(descriptor, name, path.as_ref());
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

fn attachment_output_path(
    descriptor: &oci_spec::image::Descriptor,
    name: &str,
    path: &Path,
) -> PathBuf {
    if path.is_dir() {
        path.join(attachment_export_filename(descriptor, name))
    } else {
        path.to_path_buf()
    }
}

fn attachment_export_filename(descriptor: &oci_spec::image::Descriptor, name: &str) -> String {
    attachment_filename(descriptor)
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
