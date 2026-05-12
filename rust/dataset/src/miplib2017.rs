use anyhow::{Context, Result};
use ommx::artifact::{ImageRef, LocalArtifact, LocalArtifactBuilder};
use std::{fs, path::Path};
use url::Url;
use zip::ZipArchive;

pub fn package(path: &Path) -> Result<()> {
    let annotation_dict = ommx::dataset::miplib2017::instance_annotations();
    tracing::info!("Input Archive: {}", path.display());
    let f = fs::File::open(path).with_context(|| format!("File not found: {path:?}"))?;
    let mut ar = ZipArchive::new(f).with_context(|| format!("Not a ZIP archive: {path:?}"))?;

    let source_url = Url::parse("https://github.com/Jij-Inc/ommx")?;

    // Input archive is expected to contain `*.mps.gz` files on the root level.
    for i in 0..ar.len() {
        let file = ar.by_index(i)?;
        let Some(name) = file.name().strip_suffix(".mps.gz").map(str::to_string) else {
            continue;
        };
        let Some(annotations) = annotation_dict.get(&name) else {
            tracing::warn!("Skip: No metadata found for '{name}'");
            continue;
        };

        let image_name =
            match ImageRef::parse(&format!("ghcr.io/jij-inc/ommx/v3/miplib2017:{name}")) {
                Ok(name) => name,
                Err(err) => {
                    tracing::warn!("Skip: invalid image name for '{name}': {err}");
                    continue;
                }
            };
        if LocalArtifact::try_open(image_name.clone())?.is_some() {
            tracing::info!("Skip: {image_name} already in the v3 local registry");
            continue;
        }

        tracing::info!("Loading: {name}");
        let instance = match ommx::mps::parse(file) {
            Ok(instance) => instance,
            Err(err) => {
                tracing::error!("Skip: Failed to load '{name}' with error: {err}");
                continue;
            }
        };
        let expected_count = annotations.variables()?;
        let actual_count = instance.decision_variables().len();
        if actual_count != expected_count {
            tracing::error!(
                "Skip: Variable count mismatch for '{name}': expected {expected_count}, found {actual_count}"
            );
            continue;
        }

        let mut annotations = annotations.clone();
        annotations.set_created_now();

        let mut builder = LocalArtifactBuilder::new(image_name);
        builder.add_source(&source_url);
        builder.add_instance(instance.into(), annotations)?;
        let _artifact = builder.build()?;
        // Do not push here. Use `ommx push` command to upload the artifacts.
    }
    Ok(())
}
