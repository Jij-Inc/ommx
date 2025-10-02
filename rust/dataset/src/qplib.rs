use anyhow::{Context, Result};
use ommx::artifact::Builder;
use std::{fs, path::Path};
use zip::ZipArchive;

pub fn package(path: &Path) -> Result<()> {
    log::info!("Input Archive: {}", path.display());
    let f = fs::File::open(path).with_context(|| format!("File not found: {path:?}"))?;
    let mut ar = ZipArchive::new(f).with_context(|| format!("Not a ZIP archive: {path:?}"))?;

    // Input archive is expected to contain `*.qplib` files.
    for i in 0..ar.len() {
        let file = ar.by_index(i)?;
        let file_name = file.name().to_string();
        let Some(name) = file_name.strip_suffix(".qplib").map(str::to_string) else {
            continue;
        };

        let Ok(mut builder) = Builder::for_github("Jij-Inc", "ommx", "qplib", &name) else {
            log::warn!("Skip: container already exists for '{name}'");
            continue;
        };

        log::info!("Loading: {name}");

        let instance = match ommx::qplib::parse(file) {
            Ok(instance) => instance,
            Err(err) => {
                log::error!("Skip: Failed to parse '{name}': {err}");
                continue;
            }
        };

        // Create basic annotations for QPLIB instances
        let mut annotations = ommx::artifact::InstanceAnnotations::default();
        annotations.set_title(name.clone());
        annotations.set_created_now();
        annotations.set_dataset("QPLIB".to_string());
        annotations.set_variables(instance.decision_variables().len());
        annotations.set_constraints(instance.constraints().len());

        builder.add_instance(instance.into(), annotations)?;
        let _artifact = builder.build()?;
        // Do not push here. Use `ommx push` command to upload the artifacts.
    }
    Ok(())
}
