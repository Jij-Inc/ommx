use anyhow::Result;
use ommx::artifact::Builder;
use std::{fs, path::Path};
use zip::ZipArchive;

pub fn package(path: &Path) -> Result<()> {
    let annotation_dict = ommx::dataset::miplib2017::instance_annotations();
    log::info!("Input Archive: {}", path.display());
    let f = fs::File::open(path)?;
    let mut ar = ZipArchive::new(f)?;

    for i in 0..ar.len() {
        let file = ar.by_index(i)?;
        let Some(name) = file.name().strip_suffix(".mps.gz").map(str::to_string) else {
            continue;
        };
        let Some(annotations) = annotation_dict.get(&name) else {
            log::warn!("Skip: No metadata found for '{name}'");
            continue;
        };
        log::info!("Loading: {name}");
        let instance = match ommx::mps::parse(file) {
            Ok(instance) => instance,
            Err(err) => {
                log::warn!("Skip: Failed to load '{name}' with error: {err}");
                continue;
            }
        };
        let mut builder = Builder::for_github("Jij-Inc", "ommx", "miplib2017", &name)?;
        builder.add_instance(instance.into(), annotations.clone())?;
        let mut artifact = builder.build()?;
        artifact.push()?;
    }
    Ok(())
}
